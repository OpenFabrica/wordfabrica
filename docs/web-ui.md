# 网页界面：HTML 外围 + egui 文档区（WEB.2）

## 结构

网页版分两层：

- **外围界面**用 HTML/CSS。标题栏、选项卡、功能区、下拉菜单、侧栏、对话框、文件页、状态栏都在这一层。
  代码是 Rust，用 [Leptos](https://leptos.dev)（CSR 模式）写，在 `crates/ui-web`。
- **文档区**仍是 egui canvas（`#wordcraft_canvas`）。排版、绘制、光标、选区、输入法、标尺、批注气球、右键菜单都留在这里。

桌面版不变，仍全部由 egui 绘制。

```text
index.html
┌──────────────── #wc-title   标题栏 ────────────────┐
├──────────────── #wc-tabs    选项卡 ────────────────┤
├──────────────── #wc-ribbon  功能区 ────────────────┤
│ #wc-left │        #wc-doc（egui canvas）  │ #wc-right │
│ 导航窗格 │                                │ 样式/批注 │
├──────────────── #wc-status  状态栏 ────────────────┤
└──────────────────────────────────────────────────────┘
#wc-overlay：对话框（<dialog>）、文件页、下拉菜单
```

## 只有一个编辑状态

文档、选区、撤销记录、对话框里的数据，都只在 `WordApp`/`Session` 里。HTML 层只保存纯界面状态，例如哪个菜单展开、样式库翻到第几页。

数据只按一个方向流动：

```text
HTML 事件 ──Action──▶ 队列 ──(下一帧前)──▶ WordApp::apply ──▶ Session
   ▲                                                         │
   └──── Leptos signal ◀── ChromeState 快照 ◀──(每帧后)──────┘
```

- `crates/ui-egui/src/host.rs`
  - `ChromeState`：每帧后生成的只读快照。包括标题、选项卡、撤销/重做、选区格式、段落、开关状态、命令可用表、状态栏、当前对话框、侧栏数据（只在侧栏打开时生成）、文件页数据。
  - `Action`：HTML 能请求的全部操作。`Run { id, params }` 走 `WordApp::run`，和功能区、快捷键、CLI、MCP 同一条路。
  - `HostedChrome`：哪些部分由 HTML 绘制。设为 `ALL` 后，egui 只画文档区。
- `crates/ui-web/src/bridge.rs`
  - HTML 事件只把 `Action` 放进队列，再请求重绘；不会直接借用 `WordApp`。
  - `apply_actions` 在 `app.logic` 前执行队列。`publish` 在帧末生成快照；快照变了才写入 signal。
  - 各视图用 `Bridge::select` 取快照的一部分（`Memo`），只有这部分变了才更新 DOM。
- `crates/ui-web/src/model.rs`：功能区布局写成数据（选项卡 → 组 → 控件 → 菜单项）。测试检查每个命令 id 都已注册。

## 对话框

- `dialogs::press(app, &mut Dialog, &DialogButton)` 是对话框按钮的唯一实现。egui 和 HTML 都调用它，所以两边执行的命令相同。
- HTML 对话框（`<dialog>`，`showModal`）只保存表单的本地副本。按钮发 `Action::Dialog { dialog, button }`，由 `press` 执行；需要关闭时 `app.dialog` 变为 `None`，快照随之关闭对话框。

## 图标和预览图

- 图标仍是代码画的原创图形（`crates/ui-egui/src/icons.rs`）。同一份绘制代码输出 egui 图形或 SVG（`icons::svg`）。没有新的图片资源。
- 字体菜单、样式库、样式集、表格样式、模板缩略图由引擎渲染（`crates/ui-egui/src/previews.rs`），转成 PNG data URL 后显示在 `<img>` 中。浏览器里没有文档字体，所以不能用 CSS 字体代替。每帧最多渲染 6 张，按 key 缓存。

## 焦点和输入

- 功能区按钮在 `mousedown` 时 `preventDefault`。点击按钮不会拿走文档焦点，选区和输入法状态保持不变。
- 按钮执行命令后发 `Action::FocusDocument`，焦点回到 canvas。焦点已在文档区（canvas 或 egui 的输入框）时不再移动，避免打断输入法组合。
- 字体框、字号框、搜索框、对话框里的输入框拥有焦点时，egui 没有焦点，所以正文快捷键不会触发。Enter 提交并回到文档；Esc 只回到文档。
- 焦点在 HTML 按钮上（不是文本框）时，`Ctrl/⌘` 快捷键和 F 键通过 `registry.by_shortcut` 转发给命令。
- F6：在文档和功能区之间切换。Esc：关闭菜单，或回到文档。
- 中文输入法规则不变，见 [中文编辑](chinese-editing.md)。

## 回退

在地址后加 `?egui-chrome`，页面隐藏 HTML 外围，egui 画全部界面（和 WEB.1 相同）。例如 `http://127.0.0.1:8771/?egui-chrome`。

## 开发

```sh
export PATH="$HOME/.cargo/bin:$PATH"   # wasm 检查需要 rustup 的 cargo
cd apps/wordcraft-web && trunk build --release && cd ../..
cargo run -p wordcraft-server          # 打开 http://127.0.0.1:8771/
```

- 样式在 `apps/wordcraft-web/style/*.css`。颜色只用 `--wc-*` 变量；变量由 `theme::Tokens` 写到 `<html>`，深色模式自动切换。
- 新功能先加命令（`CommandSpec`），再在 `model.rs` 里放按钮。不要在 HTML 层保存文档数据。
- `cargo xtask layers` 把 `ui-web` 放在 L6，它可以依赖 `ui-egui`，反过来不行。

## 体积

Release 构建（`trunk build --release`，2026-10-10）：

| | wasm | gzip -9 |
|---|---|---|
| WEB.1（全 egui） | 48.48 MB | 27.25 MB |
| WEB.2（HTML 外围） | 50.59 MB | 27.95 MB |

增加约 2.1 MB（gzip 后 0.7 MB），主要是 Leptos 和 HTML 视图代码。egui 外围代码仍保留，供桌面版和 `?egui-chrome` 使用。

## 文档区是否替换

本阶段不替换。egui 文档区已经支持排版、选区、输入法、批注气球和标尺，继续保留。以后只有在 HTML 文档区有明确收益时（例如无障碍阅读或浏览器原生拼写检查）再评估。
