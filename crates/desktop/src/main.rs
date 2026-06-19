mod bootstrap;
mod hook_manager;
mod log_window;
mod tray;

use bootstrap::App;
use std::sync::Arc;
use windows_sys::Win32::UI::WindowsAndMessaging::*;
use plugin_api::AppStarted;
use crate::hook_manager::{set_eventbus_ptr, set_logger_ptr};

fn main() {
    let mut app = App::bootstrap();

    set_eventbus_ptr(Arc::into_raw(app.eventbus.clone()) as *mut std::ffi::c_void);
    set_logger_ptr(Arc::into_raw(app.logger.clone()) as *mut std::ffi::c_void);

    let started = Arc::new(AppStarted);
    app.eventbus.publish(started);

    app.logger.info("core", "CapaHub 已启动 — 进入消息循环");

    run_message_loop();

    app.logger.info("core", "正在关闭...");
    if let Some(pm) = Arc::get_mut(&mut app.plugin_manager) {
        pm.unload_all();
    }
    app.hook_manager.unregister_mouse_hook();
    app.logger.info("core", "CapaHub 已退出");
}

fn run_message_loop() {
    let mut msg = unsafe { std::mem::zeroed::<MSG>() };
    loop {
        let result = unsafe { GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) };
        match result {
            0 => break,
            -1 => break,
            _ => {
                unsafe {
                    TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }
        }
    }
}
