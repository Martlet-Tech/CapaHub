# CapaHub

**CapaHub** 是 Windows 桌面能力平台——一个托盘，一个宿主，无限插件。<br>
CapaHub is a Windows desktop capability platform — one tray, one host, unlimited plugins.

剪贴板、截图、鼠标手势——都是平台上的**插件**。平台提供基础设施（EventBus、PluginManager、Hook、Overlay），插件实现业务逻辑。<br>
Clipboard, screenshot, gesture — all are **plugins**. The platform provides infrastructure; plugins provide business logic.

---

## 插件 Plugins

| 插件 Plugin | 功能 Description | 触发 Trigger |
|---|---|---|
| **Clipboard** | 剪贴板历史记录、搜索、粘贴 Clipboard history, search, paste | `Ctrl+Shift+V` 打开选择器 |
| **Gesture** | 鼠标手势：画形状触发操作 Mouse gesture recognition | 按住右键画方向/形状 |
| **Screenshot** | 选区截图 Region screenshot | `F5` 开始选择，拖拽选区域 |
| **Counter** | 截图计数器 Screenshot counter | `F6` 查看统计 |

插件是 JS 写的（`main.js` + `plugin.toml`），通过 `ctx` 对象调用平台能力（overlay、input、storage、clipboard 等）。<br>
Plugins are written in JavaScript (`main.js` + `plugin.toml`), calling platform capabilities via the `ctx` object.

---

## 架构 Architecture

```
plugins/ (JS + plugin.toml)
    ↓  ctx API
plugin_api (trait re-exports for JS runtime)
    ↓
core (EventBus, Logger, Config, PluginManager, Storage trait)
    ↓
desktop (Win32 host: tray, hooks, overlay, webview)
```

- **核心单向依赖**: `core` 不知道 `plugin_api` 存在，`plugin` 不直接调 Win32。<br>
  **One-way dependency**: `core` does not know `plugin_api`, plugins cannot call Win32 directly.
- **插件间通信**: 全部走 EventBus，禁止直接调用。<br>
  **Inter-plugin communication**: exclusively via EventBus.
- **插件加载**: 自动扫描 `plugins_dir/*/plugin.toml`，支持 `.js` 和 `.dll` 两种插件。<br>
  **Plugin loading**: auto-scans for `plugin.toml`, supports both JS and native DLL plugins.

### 核心 Crate

| Crate | 职责 Responsibility |
|---|---|
| `core` | EventBus、Logger、Config、PluginManager、`StorageProvider` trait |
| `plugin_api` | 重新导出 core 的 trait，供插件使用 Re-exports core traits for plugins |
| `desktop` | Win32 宿主：托盘、钩子管理、overlay、WebView2、clipboard 监听 |

---

## 快速开始 Quick Start

```bash
# 构建全部 Build all
cargo build --workspace

# 运行 Run
cargo run -p desktop

# 日志窗口：托盘 → Show Log
# 插件管理器：托盘 → Plugin Manager
```

### 插件目录结构 Plugin Directory Layout

```
%LOCALAPPDATA%/capahub/plugins/
├── clipboard/
│   ├── plugin.toml
│   ├── main.js
│   └── index.html
├── gesture/
│   ├── plugin.toml
│   ├── main.js
│   └── index.html
└── screenshot/
    ├── plugin.toml
    ├── main.js
    └── index.html
```

---

## 创建 JS 插件 Writing a JS Plugin

### 最小结构 Minimal Structure

```
my-plugin/
├── plugin.toml
└── main.js
```

### plugin.toml

```toml
[plugin]
name = "my-plugin"
version = "0.1.0"
description = "Does something useful"

[hooks]
mouse = true           # 是否需要鼠标钩子
keyboard = false       # 是否需要键盘钩子

[capabilities]
clipboard = false      # 是否需要剪贴板能力
input = false          # 是否需要模拟输入 (sendKeys)
overlay = false        # 是否需要覆盖层
screen = false         # 是否需要屏幕截图

[events]
subscribes = ["mouse.down", "mouse.move", "mouse.up"]
publishes = []
```

### main.js

```javascript
ctx.log("info", "my-plugin started");

function on_mouse_down(e) {
    ctx.log("debug", "mouse down: " + e.button + " at " + e.x + "," + e.y);
}

function on_mouse_move(e) {
    // only called if mouse hook is active
}

function on_mouse_up(e) {
    ctx.log("debug", "mouse up");
}
```

### ctx API

| 方法 API | 说明 Description |
|---|---|
| `ctx.log(level, msg)` | 日志：debug/info/warn/error |
| `ctx.storage.get(key)` | 读取持久化键值 Read persisted KV |
| `ctx.storage.set(key, val)` | 写入持久化键值 Write persisted KV |
| `ctx.commit_intent(type, json)` | 打开一个窗口（如选择器）Open a window (e.g. picker) |
| `ctx.close_intent()` | 关闭当前窗口 Close current window |
| `ctx.clipboard.paste(text)` | 模拟粘贴 Simulate paste |
| `ctx.clipboard.readText()` | 读取剪贴板文本 Read clipboard text |
| `ctx.input.sendKeys(str)` | 模拟按键 Simulate keystrokes |
| `ctx.overlay.cmd(json)` | 覆盖层命令（创建/绘制/销毁）Overlay commands |
| `ctx.screen.capture(x,y,w,h)` | 截取屏幕区域 Capture screen region |

事件处理函数按 `on_<event_type>()` 命名，例如订阅 `"mouse.down"` 则实现 `on_mouse_down(e)`。<br>
Event handlers are named `on_<event_type>()`, e.g. subscribe `"mouse.down"` → implement `on_mouse_down(e)`.

---

## 事件 Events

| 事件 Event | 负载 Payload | 来源 Source |
|---|---|---|
| `mouse.down` | `{ button, x, y, timestamp }` | 鼠标钩子 |
| `mouse.move` | `{ button, x, y, timestamp }` | 鼠标钩子（3:1 节流） |
| `mouse.up` | `{ button, x, y, timestamp }` | 鼠标钩子 |
| `keyboard.down` | `{ vk, timestamp }` | 键盘钩子 |
| `keyboard.up` | `{ vk, timestamp }` | 键盘钩子 |
| `app.started` | — | 启动时 |
| `app.shutdown` | — | 退出时 |
| `plugin.loaded` | `{ name }` | 插件加载时 |
| `plugin.enabled` | `{ name, needs_hook }` | 插件启用时 |
| `plugin.disabled` | `{ name }` | 插件禁用时 |
| `plugin.activate` | `{ name }` | 激活某插件 |
| `plugin.action` | `{ plugin, action, payload }` | 插件间动作调用 |
| `clipboard.changed` | 文本 | 剪贴板内容变化 |
| `screenshot.capture` | — | 触发截图 |
| `hotkey.show_clipboard` | — | 显示剪贴板选择器 |
| `hotkey.f6` | — | F6 热键 |
| `render.intent` | `RenderIntent` | 窗口/覆盖层渲染请求 |

---

## 开发命令 Development

```bash
cargo build --workspace        # 构建全部 Build all
cargo run -p desktop            # 运行 Run
cargo build --release -p core   # 构建 release
```

---

## 协议 License

AGPL-3.0
