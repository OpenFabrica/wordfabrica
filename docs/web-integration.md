# 第三方集成（WEB.4）

## 背景和边界

第三方系统把 DOCX 交给 WordCraft，用户在浏览器里编辑，每次显式保存都会导出 DOCX 并回调第三方。
思路与 [ONLYOFFICE 的保存回调](https://api.onlyoffice.com/docs/docs-api/get-started/how-it-works/saving-file) 相同，
但仍是同一个 Rust 进程：回调队列在 SQLite 中，文件在本地目录或 MinIO，不需要独立转换或消息服务。
基础编辑器和存储见 [web-storage.md](web-storage.md)。

- 不启用时（未设置 `WORDCRAFT_INTEGRATION_SECRET`）所有 `/api/integration/*` 路由返回 404，行为与之前完全一致。
- 没有按用户的权限，也没有只读模式：拿到编辑链接的人都能编辑，信任模型与基础编辑器相同。共享部署请放在带身份认证的反向代理之后。
- 只处理 DOCX。`.docm` 保存后丢失宏，`.dotx` 保存后成为普通文档。没有版本历史界面。

## 流程

1. 第三方 `POST /api/integration/documents`，给出 DOCX（服务器下载 URL，或 multipart 上传）、`callback_url` 和自己的 `key`。
2. 服务器用 `wordcraft_docx` 解析；原件存为 `originals/{id}.docx`；同一个 SQLite 事务里创建 revision 1 的编辑器文档和集成记录。
3. 返回 `{id, revision, edit_url}`。`edit_url` 为 `{WORDCRAFT_PUBLIC_URL}/?doc={id}`，在浏览器打开即进入现有的 `?doc=` 流程。
4. 自动保存只写 JSON 草稿，不产生回调。
5. 用户显式保存（Ctrl/Cmd+S 或保存按钮）时，请求带 `commit: true`：服务器导出 DOCX，存为 `saved/{id}/{revision}.docx`，在 SQLite outbox 排队，后台 worker 向 `callback_url` 发送签名回调。

```mermaid
sequenceDiagram
    participant T as 第三方
    participant S as WordCraft 服务
    participant F as 文件存储
    participant D as SQLite
    participant U as 用户浏览器
    T->>S: POST /api/integration/documents (签名)
    S->>T: 下载 url（仅 JSON 模式）
    S->>F: originals/{id}.docx
    S->>D: 事务：文档 revision 1 + 集成记录
    S-->>T: {id, revision, edit_url}
    T-->>U: 打开 edit_url
    U->>S: PUT 自动保存（只存草稿）
    U->>S: PUT commit:true（Ctrl/Cmd+S）
    S->>F: saved/{id}/{revision}.docx
    S->>D: outbox 入队
    S-->>U: Metadata
    S->>T: POST callback_url (签名, 含下载链接)
    T-->>S: 2xx
    T->>F: 用链接下载 DOCX
```

`commit` 的处理幂等：导出或存储失败时用户看到 503，重试同一份快照会被确认并再次提交。没有集成记录的普通文档忽略 `commit`。

## 配置

| 环境变量 | 默认值 | 职责 |
|---|---|---|
| `WORDCRAFT_INTEGRATION_SECRET` | 未设置 = 关闭 | 与第三方共享的密钥，至少 32 字节 |
| `WORDCRAFT_PUBLIC_URL` | 无 | 启用时必填。用户和第三方访问本服务的地址，如 `https://docs.example.com`，用于 `edit_url` 和下载链接 |
| `WORDCRAFT_STORAGE` | `local` | `local` 或 `minio` |
| `WORDCRAFT_STORAGE_DIR` | `<数据目录>/files` | `local` 时的文件目录 |
| `WORDCRAFT_S3_ENDPOINT` | 无 | `minio` 必填，如 `http://127.0.0.1:9000` |
| `WORDCRAFT_S3_BUCKET` | 无 | `minio` 必填；桶必须事先存在 |
| `WORDCRAFT_S3_ACCESS_KEY` / `WORDCRAFT_S3_SECRET_KEY` | 无 | `minio` 必填 |
| `WORDCRAFT_S3_REGION` | `us-east-1` | S3 区域 |
| `WORDCRAFT_DOWNLOAD_TTL` | `3600` | 下载链接有效秒数，范围 60..=604800 |
| `WORDCRAFT_ALLOW_HOSTS` | 空 = 不限制 | 逗号分隔的主机名；同时约束文档 URL（含每次重定向）和 `callback_url`。生产环境建议设置，防止 SSRF |

配置错误（密钥过短、缺少必填项、TTL 越界）时服务拒绝启动。MinIO 使用 path-style 地址，允许 `http://`。

本地 MinIO 示例：

```sh
docker run -p 9000:9000 -p 9001:9001 \
  -e MINIO_ROOT_USER=minio -e MINIO_ROOT_PASSWORD=minio12345 \
  minio/minio server /data --console-address :9001
mc alias set local http://127.0.0.1:9000 minio minio12345
mc mb local/wordcraft

export WORDCRAFT_INTEGRATION_SECRET="$(openssl rand -hex 32)"
export WORDCRAFT_PUBLIC_URL=http://127.0.0.1:8771
export WORDCRAFT_STORAGE=minio
export WORDCRAFT_S3_ENDPOINT=http://127.0.0.1:9000
export WORDCRAFT_S3_BUCKET=wordcraft
export WORDCRAFT_S3_ACCESS_KEY=minio
export WORDCRAFT_S3_SECRET_KEY=minio12345
cargo run -p wordcraft-server
```

MinIO 模式下下载链接是 S3 预签名 URL；本地模式下是服务器自己签名的 `/api/integration/files/...` 链接。

## 请求签名

第三方发给 WordCraft 的每个 `/api/integration/documents*` 请求带两个头：

- `X-WordCraft-Timestamp`：Unix 秒，与服务器时间相差不超过 ±300 秒。
- `X-WordCraft-Signature`：小写十六进制 `HMAC-SHA256(secret, "{timestamp}.{METHOD}.{path}.{raw body bytes}")`。

`path` 是 WordCraft 收到的 URL 路径，不含查询串（反向代理若改写路径，要按改写后的路径签名）。
`METHOD` 为大写。`GET` 没有请求体，签名串以 `.` 结尾。`body` 必须与实际发送的字节完全一致。

### curl（JSON 模式）

```sh
SECRET=...                       # 与服务器相同
BASE=http://127.0.0.1:8771
P=/api/integration/documents
BODY='{"url":"https://files.example.com/a.docx","file_name":"a.docx","callback_url":"https://app.example.com/wordcraft/callback","key":"doc-42"}'
TS=$(date +%s)
SIG=$(printf '%s.POST.%s.%s' "$TS" "$P" "$BODY" | openssl dgst -sha256 -hmac "$SECRET" -hex | awk '{print $NF}')
curl -sS "$BASE$P" -H 'Content-Type: application/json' \
  -H "X-WordCraft-Timestamp: $TS" -H "X-WordCraft-Signature: $SIG" \
  --data-binary "$BODY"
# {"id":"…","revision":1,"edit_url":"http://127.0.0.1:8771/?doc=…"}

# 查询状态（GET，空请求体）
ID=...
P=/api/integration/documents/$ID
TS=$(date +%s)
SIG=$(printf '%s.GET.%s.' "$TS" "$P" | openssl dgst -sha256 -hmac "$SECRET" -hex | awk '{print $NF}')
curl -sS "$BASE$P" -H "X-WordCraft-Timestamp: $TS" -H "X-WordCraft-Signature: $SIG"
```

### Python（multipart 上传）

multipart 的边界和各部分头都属于请求体，所以先自己拼好字节，再对这些字节签名并原样发送。

```python
import hashlib, hmac, json, time, urllib.request

SECRET, BASE = b"...", "http://127.0.0.1:8771"
PATH = "/api/integration/documents"
meta = {"file_name": "a.docx", "callback_url": "https://app.example.com/wordcraft/callback", "key": "doc-42"}
boundary = "wc-boundary-7f3a9"
body = b"".join([
    f'--{boundary}\r\nContent-Disposition: form-data; name="meta"\r\n'
    f'Content-Type: application/json\r\n\r\n{json.dumps(meta)}\r\n'.encode(),
    f'--{boundary}\r\nContent-Disposition: form-data; name="file"; filename="a.docx"\r\n'
    f'Content-Type: application/octet-stream\r\n\r\n'.encode(),
    open("a.docx", "rb").read(),
    f"\r\n--{boundary}--\r\n".encode(),
])
ts = str(int(time.time()))
sig = hmac.new(SECRET, ts.encode() + b".POST." + PATH.encode() + b"." + body, hashlib.sha256).hexdigest()
request = urllib.request.Request(BASE + PATH, data=body, method="POST", headers={
    "Content-Type": f"multipart/form-data; boundary={boundary}",
    "X-WordCraft-Timestamp": ts,
    "X-WordCraft-Signature": sig,
})
print(urllib.request.urlopen(request).read().decode())
```

## 接口

| 请求 | 说明 |
|---|---|
| `POST /api/integration/documents` | 签名。注册文档，返回 `{id, revision, edit_url}` |
| `GET /api/integration/documents/{id}` | 签名。返回 `{id, key, revision, saved_revision, callback_state, url, url_expires_at}` |
| `GET /api/integration/files/{id}/{revision}?expires=&sig=` | 本地存储的下载链接，由服务器签名并过期；无需请求签名 |

创建请求有两种形式：

- `application/json`：`{url, file_name, callback_url, key, title?}`，服务器下载 `url`。
- `multipart/form-data`：`meta` 部分是同样的 JSON（不含 `url`），`file` 部分是 DOCX 字节。

| 字段 | 规则 |
|---|---|
| `file_name` | 1–255 字符，不含斜杠和控制字符；扩展名只能是 `.docx`、`.docm`、`.dotx`。保存结果始终是 DOCX，文件名为 `<stem>.docx` |
| `key` | 1–200 字符，第三方自己的文档标识，原样出现在回调里 |
| `title` | 可选，最多 200 字符；缺省为文件名主干 |
| `callback_url` | http(s)；配置了 `WORDCRAFT_ALLOW_HOSTS` 时主机必须在名单内 |
| `url` | 仅 JSON 模式。仅 http/https；15 秒超时；最多 5 次重定向且每一跳都按允许名单检查；最多 16 MiB |

请求体最大 16 MiB。状态里的 `callback_state` 为 `pending`、`done` 或 `failed`；还没有显式保存过时 `saved_revision`、`callback_state`、`url` 为 `null`。
每次查询状态都会生成新的下载链接。下载链接也允许从其他网站通过顶层导航打开（浏览器地址栏跳转），其余 API 仍然只限同源。

| 状态码 | 含义 |
|---|---|
| 400 | 输入无效，正文为 `{"error": "..."}` |
| 401 | 签名缺失、错误或时间戳过期 |
| 403 | 下载链接错误或已过期 |
| 404 | 文档不存在，或集成未启用 |
| 413 | 超过 16 MiB |
| 502 | 无法下载 `url` |
| 503 | 文件存储或导出失败 |

## 保存回调

服务器向 `callback_url` 发送 `POST`，`Content-Type: application/json`，带同样的两个头。
签名是 `HMAC-SHA256(secret, "{timestamp}.{raw body}")`，没有 METHOD 和 path。

```json
{"event":"document.saved","id":"…","key":"doc-42","revision":3,"title":"a","file_name":"a.docx",
 "url":"https://docs.example.com/api/integration/files/…","url_expires_at":1760000000,"saved_at":1759996400000}
```

`saved_at` 是 Unix 毫秒，`url_expires_at` 是 Unix 秒。第三方用 `url` 下载 DOCX（链接过期前有效）。

验证示例（Python）：

```python
def verify(secret: bytes, headers, raw_body: bytes) -> bool:
    ts = headers["X-WordCraft-Timestamp"]
    if abs(time.time() - int(ts)) > 300:
        return False
    expected = hmac.new(secret, ts.encode() + b"." + raw_body, hashlib.sha256).hexdigest()
    return hmac.compare_digest(expected, headers["X-WordCraft-Signature"])
```

投递规则：

- 任何 2xx 都算送达。不跟随重定向，签名正文只会发到登记的地址；每次发送前会重新检查 `WORDCRAFT_ALLOW_HOSTS`。
- 失败后指数退避重试：`5 秒 × 2^(n-1)`，上限 1 小时，最多 10 次，之后状态为 `failed`。
- 同一文档较新的保存会取代较旧、未送达的回调；第三方应比较 `revision`，只接受不小于已处理版本的回调，并容忍重复。
- 队列在 SQLite 中，服务重启后继续投递。每次尝试都会生成新的下载链接。

## 实现

- `apps/wordcraft-server/src/integration.rs`：配置、路由、请求签名中间件、创建/状态/下载。
- `apps/wordcraft-server/src/callbacks.rs`：outbox worker、退避、签名回调。
- `apps/wordcraft-server/src/signing.rs`：HMAC-SHA256 签名与常量时间校验。
- `apps/wordcraft-server/src/storage.rs`：本地目录或 MinIO（`object_store`）文件存储。
- `apps/wordcraft-server/src/store.rs`：集成记录和 outbox 表。
- `crates/remote`：`WriteRequest.commit`（可选布尔，默认 `false`）。
- `apps/wordcraft-web/src/remote.rs`：手动保存带 `commit: true`，自动保存不带。
