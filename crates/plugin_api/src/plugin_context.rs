use std::ffi::c_void;

#[repr(C)]
pub struct PluginContextFFI {
    pub logger: *mut c_void,
    pub eventbus: *mut c_void,
    pub config_path: *const u16,
    pub plugin_name: *const u8,
}
