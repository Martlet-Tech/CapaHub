pub mod event;
pub mod plugin;
pub mod plugin_context;

pub use event::{
    AppShutdown, AppStarted, ClipboardChanged, ClipboardItemDeleted, ClipboardItemSelected, Event, MouseButton,
    MouseDown, MouseEvent, MouseMove, MouseUp, PluginActivate, PluginAction, PluginDisabled,
    PluginEnabled, PluginLoaded, ShowClipboard,
};
pub use plugin::Plugin;
pub use plugin_context::PluginContextFFI;
