# CapaHub

**CapaHub** 是 Windows 桌面能力平台，不是工具——是承载插件的平台。<br>
CapaHub is a Windows desktop capability platform — not a tool, but a platform that hosts plugins.

## 设计初衷 Motivation

电脑自启动的东西越来越多：剪贴板（Dittxxx）、鼠标手势（Strokexxx）、截图（Snipaxxx）、代理（Claxxx）……每个都占一个托盘图标，各自维护一套热键、配置、更新逻辑。<br>
I had too many autostart utilities — clipboard manager, mouse gestures, screenshot, proxy — each with its own tray icon, hotkeys, config, and update mechanism.

这些工具共享同一套基础设施：全局钩子、热键、配置、日志、数据库、覆盖层。为什么不能统一成一个平台？<br>
They all share the same infrastructure: global hooks, hotkeys, config, logging, storage, overlay. Why not unify them into one platform?

**CapaHub 的答案：** 一个托盘，一个宿主，无限插件。<br>
**CapaHub's answer:** One tray, one host, unlimited plugins.

剪贴板、截图、鼠标手势、OCR、AI——都是平台上的**插件**。平台提供基础设施（EventBus、PluginManager、Config、Logger、Input Hook），插件实现业务逻辑。<br>
Clipboard, screenshot, gesture, OCR, AI — all are **plugins**. The platform provides infrastructure; plugins provide business logic.

---

## 欢迎参与 Welcome

本项目作者完全不懂 Rust，全靠 AI 智能体编程。代码质量、架构设计肯定有很多不成熟的地方，欢迎各路大佬指点、提 issue、甚至直接 PR。
The author doesn't know Rust at all — every line of code is written by AI agents. The code quality and architecture are far from perfect. Pull requests, issues, and advice are all warmly welcomed.

有能力的朋友，欢迎帮忙实现核心插件（剪贴板、截图、鼠标手势），或者给平台本身提改进建议。
If you can help implement the core plugins (clipboard, screenshot, gesture) or improve the platform itself, please jump in.

---

## 架构 Architecture

```
CapaHub
├── Core      事件总线/日志/配置/插件管理/事件类型
│             EventBus, Logger, Config, PluginManager, types
├── Plugin API  插件 SDK：Plugin trait、PluginContext、FFI 边界
│               Plugin trait, PluginContext, FFI boundary
├── Desktop   Win32 宿主：托盘/日志窗口/钩子管理/插件管理器
│             Win32 host: tray, log window, hook manager, manager UI
└── Plugins   运行时加载的 cdylib
              cdylib .dll loaded at runtime
```

**依赖方向（单向）**<br>
**Dependency direction (one-way):**
```
plugins/* → plugin_api → core → desktop
```

- core 不知道 plugin_api 存在。<br>
  core does not know plugin_api exists.
- 插件不能调 Win32 或其他插件。<br>
  Plugins must not import Win32 or other plugins.
- 插件间通信全部走 EventBus。<br>
  All inter-plugin communication goes through EventBus.

---

## 快速开始 Quick Start

```bash
# 构建并运行 Build and run
cargo run -p desktop

# 打包模板插件为 .dap 安装包
# Package template plugin as installable .dap
scripts\pack-plugin.bat template

# 启动 CapaHub 后：托盘 → 插件管理器 → Install DAP → 选择 .dap 文件
# After launch: Tray → Plugin Manager → Install DAP → select .dap file
```

---

## 插件生命周期 Plugin Lifecycle

```
Loaded → Enabled ↔ Disabled → Unloaded
```

| 状态 State | 含义 Meaning |
|------------|-------------|
| `Loaded` | DLL 已加载，on_load 已调用，**未**订阅事件。<br>DLL loaded, on_load called, NOT subscribed. |
| `Enabled` | on_enable 已调用，事件订阅中。<br>on_enable called, subscribed to events. |
| `Disabled` | 取消事件订阅，on_disable 已调用，DLL 不卸载。<br>Unsubscribed, on_disable called, DLL stays loaded. |
| `Unloaded` | on_unload 已调用，DLL 释放。<br>on_unload called, DLL freed. |

通过**插件管理器窗口**（托盘 → Plugin Manager）或代码控制状态。<br>
Manage states via **Plugin Manager** window (Tray → Plugin Manager) or programmatically.

---

## 创建插件 Creating a Plugin

### 项目结构 Project Structure

```
my-plugin/
├── Cargo.toml
├── plugin.toml
└── src/
    └── lib.rs
```

### Cargo.toml

```toml
[package]
name = "my-plugin"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
plugin_api = { path = "../../crates/plugin_api" }
```

### plugin.toml

```toml
[plugin]
name = "my-plugin"
version = "0.1.0"
description = "Does something useful"

[hooks]
mouse = true

[events]
subscribes = ["mouse.down", "mouse.move", "mouse.up"]
publishes = []
```

### 插件代码 Plugin Code

```rust
use plugin_api::{Event, MouseButton, Plugin, PluginContext, PluginContextFFI};
use std::ffi::c_void;
use std::sync::Arc;

struct MyPlugin { ctx: PluginContext }

impl Plugin for MyPlugin {
    fn on_load(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.ctx.logger.info("my-plugin", "loaded");
        Ok(())
    }
    fn on_enable(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.ctx.logger.info("my-plugin", "enabled");
        Ok(())
    }
    fn on_disable(&mut self) {
        self.ctx.logger.info("my-plugin", "disabled");
    }
    fn on_unload(&mut self) {
        self.ctx.logger.info("my-plugin", "unloaded");
    }
    fn on_event(&mut self, event: Arc<dyn Event>) {
        if let Some(me) = event.mouse_event() {
            self.ctx.logger.debug("my-plugin",
                &format!("mouse at {},{}", me.x, me.y));
        }
    }
}

#[no_mangle]
pub extern "C" fn plugin_create(ctx_ptr: *const PluginContextFFI) -> *mut c_void {
    let ctx = unsafe { (*ctx_ptr).to_plugin_context() };
    Box::into_raw(Box::new(Box::new(MyPlugin { ctx }) as Box<dyn Plugin>)) as *mut c_void
}

#[no_mangle]
pub extern "C" fn plugin_destroy(ptr: *mut c_void) {
    if !ptr.is_null() {
        unsafe { drop(Box::from_raw(*Box::from_raw(ptr as *mut *mut dyn Plugin))); }
    }
}
```

### 打包为 .dap  Package as .dap

```
my-plugin-0.1.0.dap
├── plugin.toml
├── my_plugin.dll
├── index.html          可选，托盘菜单点插件名时用浏览器打开
│                       optional, opened from tray menu
└── assets/
    └── icon.png        可选 optional
```

构建后 `cargo build --release`，用 zip 打包后改后缀为 `.dap`。<br>
Build with `cargo build --release`, zip with `.dap` extension.

---

## 事件表 Events

| 事件 Event | 负载 Data | 来源 Source |
|-----------|-----------|------------|
| `mouse.down` | `MouseEvent` | 鼠标钩子 HookManager |
| `mouse.move` | `MouseEvent` | 鼠标钩子 HookManager |
| `mouse.up` | `MouseEvent` | 鼠标钩子 HookManager |
| `app.started` | — | 启动 Bootstrap |
| `app.shutdown` | — | 关闭 Shutdown |
| `plugin.loaded` | `PluginLoaded { name }` | 加载 Load |
| `plugin.enabled` | `PluginEnabled { name, needs_hook }` | 启用 Enable |
| `plugin.disabled` | `PluginDisabled { name }` | 禁用 Disable |

---

## 开发命令 Development

```bash
# 构建全部 Build all
cargo build --workspace

# 运行 Run
cargo run -p desktop

# 构建并部署模板插件 Build & deploy template
scripts\deploy-plugin.bat template

# 打包 .dap 安装包 Package .dap installer
scripts\pack-plugin.bat template

# 启动脚本（自动构建 desktop）Run script
scripts\run.bat
```
