use crate::eventbus::EventBus;
use crate::logger::Logger;
use crate::render_intent::{RenderIntent, RenderIntentEvent};
use crate::storage::Storage;
use std::os::windows::ffi::OsStrExt;
use std::path::PathBuf;
use std::sync::Arc;

pub struct PluginContext {
    pub logger: Arc<Logger>,
    pub eventbus: Arc<EventBus>,
    pub storage: Arc<Storage>,
    pub config_path: PathBuf,
    pub plugin_name: String,
}

impl PluginContext {
    pub fn commit_intent(&self, intent: RenderIntent) {
        self.eventbus.publish(Arc::new(RenderIntentEvent(intent)));
    }

    pub fn close_intent(&self) {}

    pub fn to_ffi(&self) -> crate::plugin_context_ffi::PluginContextFFI {
        let config_path_wide: Vec<u16> = self.config_path.as_os_str().encode_wide().collect();
        let config_path_ptr = config_path_wide.as_ptr();
        std::mem::forget(config_path_wide);

        crate::plugin_context_ffi::PluginContextFFI {
            logger: Arc::as_ptr(&self.logger) as *mut std::ffi::c_void,
            eventbus: Arc::as_ptr(&self.eventbus) as *mut std::ffi::c_void,
            config_path: config_path_ptr,
            plugin_name: self.plugin_name.as_ptr(),
        }
    }

    pub unsafe fn from_ffi(ffi: &crate::plugin_context_ffi::PluginContextFFI) -> Self {
        let logger = Arc::from_raw(ffi.logger as *const Logger);
        let eventbus = Arc::from_raw(ffi.eventbus as *const EventBus);
        let plugin_name = {
            let mut len = 0;
            while *ffi.plugin_name.add(len) != 0 { len += 1; }
            String::from_utf8_unchecked(std::slice::from_raw_parts(ffi.plugin_name, len).to_vec())
        };
        let config_path = {
            let mut len = 0;
            while *ffi.config_path.add(len) != 0 { len += 1; }
            String::from_utf16_lossy(std::slice::from_raw_parts(ffi.config_path, len))
        };
        PluginContext {
            logger,
            eventbus,
            storage: Arc::new(Storage::new_in_memory().unwrap()),
            config_path: PathBuf::from(config_path),
            plugin_name,
        }
    }
}
