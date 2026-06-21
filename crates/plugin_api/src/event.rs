use std::any::Any;

pub trait Event: Send + Sync {
    fn event_type(&self) -> &'static str;
    fn as_any(&self) -> &dyn Any;
    fn js_repr(&self) -> String { String::new() }
}

/// Event with dynamically-configured type and representation.
/// Used by js_runtime overlay callbacks, so core doesn't need to import
/// concrete event types from plugins.
pub struct DynamicEvent {
    pub event_type: &'static str,
    pub repr: String,
}

impl Event for DynamicEvent {
    fn event_type(&self) -> &'static str { self.event_type }
    fn as_any(&self) -> &dyn Any { self }
    fn js_repr(&self) -> String { self.repr.clone() }
}

#[derive(Debug, Clone, PartialEq)]
pub enum MouseButton {
    Left, Right, Middle, X1, X2, None,
}

#[derive(Debug, Clone)]
pub struct MouseEvent {
    pub button: MouseButton,
    pub x: i32,
    pub y: i32,
    pub timestamp: u64,
}

impl Event for MouseEvent {
    fn event_type(&self) -> &'static str { "mouse.event" }
    fn as_any(&self) -> &dyn Any { self }
}

pub struct MouseDown(pub MouseEvent);
impl Event for MouseDown {
    fn event_type(&self) -> &'static str { "mouse.down" }
    fn as_any(&self) -> &dyn Any { self }
    fn js_repr(&self) -> String {
        mouse_js_repr(&self.0)
    }
}

pub struct MouseMove(pub MouseEvent);
impl Event for MouseMove {
    fn event_type(&self) -> &'static str { "mouse.move" }
    fn as_any(&self) -> &dyn Any { self }
    fn js_repr(&self) -> String {
        mouse_js_repr(&self.0)
    }
}

pub struct MouseUp(pub MouseEvent);
impl Event for MouseUp {
    fn event_type(&self) -> &'static str { "mouse.up" }
    fn as_any(&self) -> &dyn Any { self }
    fn js_repr(&self) -> String {
        mouse_js_repr(&self.0)
    }
}

// ── Keyboard events ──────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct KeyEvent {
    pub vk_code: u32,
    pub pressed: bool,
    pub timestamp: u64,
}

pub struct KeyDown(pub KeyEvent);
impl Event for KeyDown {
    fn event_type(&self) -> &'static str { "keyboard.down" }
    fn as_any(&self) -> &dyn Any { self }
    fn js_repr(&self) -> String {
        format!("{{ vk: {}, ts: {} }}", self.0.vk_code, self.0.timestamp)
    }
}

pub struct KeyUp(pub KeyEvent);
impl Event for KeyUp {
    fn event_type(&self) -> &'static str { "keyboard.up" }
    fn as_any(&self) -> &dyn Any { self }
    fn js_repr(&self) -> String {
        format!("{{ vk: {}, ts: {} }}", self.0.vk_code, self.0.timestamp)
    }
}

fn mouse_js_repr(me: &MouseEvent) -> String {
    let btn = match me.button {
        MouseButton::Left => "Left",
        MouseButton::Right => "Right",
        MouseButton::Middle => "Middle",
        _ => "None",
    };
    format!("{{ button: \"{}\", x: {}, y: {}, timestamp: {} }}", btn, me.x, me.y, me.timestamp)
}

pub struct AppStarted;
impl Event for AppStarted {
    fn event_type(&self) -> &'static str { "app.started" }
    fn as_any(&self) -> &dyn Any { self }
}

pub struct AppShutdown;
impl Event for AppShutdown {
    fn event_type(&self) -> &'static str { "app.shutdown" }
    fn as_any(&self) -> &dyn Any { self }
}

pub struct PluginLoaded {
    pub name: String,
}
impl Event for PluginLoaded {
    fn event_type(&self) -> &'static str { "plugin.loaded" }
    fn as_any(&self) -> &dyn Any { self }
}

pub struct PluginEnabled {
    pub name: String,
    pub needs_hook: bool,
}
impl Event for PluginEnabled {
    fn event_type(&self) -> &'static str { "plugin.enabled" }
    fn as_any(&self) -> &dyn Any { self }
}

pub struct PluginDisabled {
    pub name: String,
}
impl Event for PluginDisabled {
    fn event_type(&self) -> &'static str { "plugin.disabled" }
    fn as_any(&self) -> &dyn Any { self }
}

pub struct PluginActivate {
    pub name: String,
}
impl Event for PluginActivate {
    fn event_type(&self) -> &'static str { "plugin.activate" }
    fn as_any(&self) -> &dyn Any { self }
    fn js_repr(&self) -> String {
        format!("{{ name: \"{}\" }}", self.name)
    }
}

pub struct PluginAction {
    pub plugin: String,
    pub action: String,
    pub payload: String,
}
impl Event for PluginAction {
    fn event_type(&self) -> &'static str { "plugin.action" }
    fn as_any(&self) -> &dyn Any { self }
    fn js_repr(&self) -> String {
        format!(
            "{{ plugin: \"{}\", action: \"{}\", payload: \"{}\" }}",
            self.plugin, self.action, self.payload
        )
    }
}
