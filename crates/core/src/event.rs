use std::any::Any;

pub trait Event: Send + Sync {
    fn event_type(&self) -> &'static str;
    fn as_any(&self) -> &dyn Any;

    fn mouse_event(&self) -> Option<&MouseEvent> { None }
}

#[derive(Debug, Clone, PartialEq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    X1,
    X2,
    None,
}

#[derive(Debug, Clone)]
pub struct MouseEvent {
    pub button: MouseButton,
    pub x: i32,
    pub y: i32,
    pub timestamp: u64,
}

impl Event for MouseEvent {
    fn event_type(&self) -> &'static str {
        "mouse.event"
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn mouse_event(&self) -> Option<&MouseEvent> {
        Some(self)
    }
}

pub struct MouseDown(pub MouseEvent);
impl Event for MouseDown {
    fn event_type(&self) -> &'static str {
        "mouse.down"
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn mouse_event(&self) -> Option<&MouseEvent> {
        Some(&self.0)
    }
}

pub struct MouseMove(pub MouseEvent);
impl Event for MouseMove {
    fn event_type(&self) -> &'static str {
        "mouse.move"
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn mouse_event(&self) -> Option<&MouseEvent> {
        Some(&self.0)
    }
}

pub struct MouseUp(pub MouseEvent);
impl Event for MouseUp {
    fn event_type(&self) -> &'static str {
        "mouse.up"
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn mouse_event(&self) -> Option<&MouseEvent> {
        Some(&self.0)
    }
}

pub struct AppStarted;
impl Event for AppStarted {
    fn event_type(&self) -> &'static str {
        "app.started"
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

pub struct AppShutdown;
impl Event for AppShutdown {
    fn event_type(&self) -> &'static str {
        "app.shutdown"
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

pub struct PluginLoaded {
    pub name: String,
}
impl Event for PluginLoaded {
    fn event_type(&self) -> &'static str {
        "plugin.loaded"
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}
