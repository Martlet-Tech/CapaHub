use plugin_api::Event;
use std::any::Any;

pub struct ClipboardChanged {
    pub text: String,
}
impl Event for ClipboardChanged {
    fn event_type(&self) -> &'static str { "clipboard.changed" }
    fn as_any(&self) -> &dyn Any { self }
    fn js_repr(&self) -> String {
        let escaped = self.text
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
            .replace('\r', "");
        format!("\"{}\"", escaped)
    }
}

pub struct ShowClipboard;
impl Event for ShowClipboard {
    fn event_type(&self) -> &'static str { "hotkey.show_clipboard" }
    fn as_any(&self) -> &dyn Any { self }
}
