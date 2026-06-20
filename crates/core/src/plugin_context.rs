use crate::eventbus::EventBus;
use crate::logger::Logger;
use crate::render_intent::{RenderIntent, RenderIntentEvent};
use std::ffi::c_void;
use std::os::windows::ffi::OsStrExt;
use std::path::PathBuf;
use std::sync::Arc;

pub struct PluginContext {
    pub logger: Arc<Logger>,
    pub eventbus: Arc<EventBus>,
    pub config_path: PathBuf,
    pub plugin_name: String,
}

impl PluginContext {
    pub fn commit_intent(&self, intent: RenderIntent) {
        self.eventbus.publish(Arc::new(RenderIntentEvent(intent)));
    }

    pub fn close_intent(&self) {
        // Will be handled via event in the future
    }
}

#[repr(C)]
pub struct PluginContextFFI {
    pub logger: *mut c_void,
    pub eventbus: *mut c_void,
    pub config_path: *const u16,
    pub plugin_name: *const u8,
}

impl PluginContextFFI {
    pub fn from(ctx: &PluginContext) -> Self {
        let config_path_wide: Vec<u16> = ctx.config_path.as_os_str().encode_wide().collect();
        let config_path_ptr = config_path_wide.as_ptr();
        std::mem::forget(config_path_wide);

        PluginContextFFI {
            logger: Arc::as_ptr(&ctx.logger) as *mut c_void,
            eventbus: Arc::as_ptr(&ctx.eventbus) as *mut c_void,
            config_path: config_path_ptr,
            plugin_name: ctx.plugin_name.as_ptr(),
        }
    }

    pub unsafe fn to_plugin_context(&self) -> PluginContext {
        let logger = Arc::from_raw(self.logger as *const Logger);
        let eventbus = Arc::from_raw(self.eventbus as *const EventBus);
        let plugin_name = {
            let mut len = 0;
            while *self.plugin_name.add(len) != 0 { len += 1; }
            String::from_utf8_unchecked(std::slice::from_raw_parts(self.plugin_name, len).to_vec())
        };
        let config_path = {
            let mut len = 0;
            while *self.config_path.add(len) != 0 { len += 1; }
            String::from_utf16_lossy(std::slice::from_raw_parts(self.config_path, len))
        };
        PluginContext {
            logger,
            eventbus,
            config_path: PathBuf::from(config_path),
            plugin_name,
        }
    }
}
