# 网页版插件（WEB.3）

插件给网页版加功能。插件是 **JSON 数据文件**，不含代码。所以插件不能运行脚本，也不能读写文档以外的东西。

现在支持一种插件：**模版标签**（`"kind": "tags"`）。

## 用户怎么用

1. 选项卡右侧（“批注”按钮左边）有插件按钮，例如“模版标签”。
2. 点击按钮，右侧打开标签窗格。再点一次，或点 ✕，关闭窗格。
3. 窗格里可以：
   - 在搜索框输入文字。标签名、插入文字、说明、分类名都能匹配，不分大小写。
   - 点分类按钮（“全部”、“客户信息”…）只看一个分类。
4. 点一个标签，它的文字插入到文档光标处。有选中文字时，替换选中文字（和打字一样）。插入可以撤销。

## 管理员怎么配置

把插件文件放进插件目录，然后重启服务。

- 插件目录：环境变量 `WORDCRAFT_PLUGINS_DIR`。没设置时用启动目录下的 `plugins/`；这个目录不存在就不加载插件。
- 只读目录里直接放的 `*.json` 文件，按文件名排序。子目录、符号链接、其他文件都忽略。
- 启动时检查每个文件。有一个文件不合格，服务就不启动，并打印哪个文件、什么问题。
- 仓库里的 `plugins/template-tags.json` 是示例。

```sh
WORDCRAFT_PLUGINS_DIR=/srv/wordcraft-plugins cargo run -p wordcraft-server
```

只有连着服务端的网页版有插件。`?local` 模式、`?egui-chrome` 模式和桌面版没有插件。

## 文件格式

```json
{
  "id": "template-tags",
  "name": "模版标签",
  "kind": "tags",
  "categories": [
    {
      "name": "客户信息",
      "tags": [
        { "label": "客户名称", "text": "{{客户名称}}", "note": "甲方单位全称" },
        { "label": "{{联系人}}" }
      ]
    }
  ]
}
```

| 字段 | 必填 | 说明 |
|---|---|---|
| `id` | 是 | 1–64 个字符，只能用 `a-z 0-9 - _ .`。所有插件里不能重复。 |
| `name` | 是 | 按钮和窗格标题，一行，最多 100 字。 |
| `kind` | 是 | 现在只有 `"tags"`。 |
| `categories` | 是 | 1–500 个分类。分类 `name` 一行，最多 100 字。 |
| `tags[].label` | 是 | 窗格里显示的名字，一行，最多 200 字。 |
| `tags[].text` | 否 | 插入的文字，最多 2000 字。不能有换行等控制字符（可以有 Tab）。不写就插入 `label`。 |
| `tags[].note` | 否 | 鼠标悬停时显示的说明，最多 500 字。 |

上限：最多 32 个插件；每个文件最多 1 MiB、5000 个标签。

## 代码位置

- `crates/remote`：共享格式 `Plugin`、`PluginKind`、`Tag`，以及 `Plugin::validate`（服务端和浏览器都用它检查）。
- `apps/wordcraft-server/src/plugins.rs`：启动时读目录、检查，提供 `GET /api/plugins`。
- `apps/wordcraft-web/src/plugins.rs`：编辑器启动后读取插件（10 秒超时）。失败只记日志，编辑器照常用。
- `crates/ui-web/src/plugins.rs`：分类和搜索的过滤逻辑（可本地测试）。
- `crates/ui-web/src/view/plugins.rs`：插件按钮和标签窗格。

## 以后加新类型

在 `PluginKind` 里加一个变体，在 `Plugin::validate` 里加检查，再在 `view/plugins.rs` 的 `pane` 里加它的窗格。插件做的事都要走已有命令（`Bridge::run`），不要在网页层保存文档数据。
