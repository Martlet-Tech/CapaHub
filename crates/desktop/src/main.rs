mod bootstrap;
mod hook_manager;
mod icon_loader;
mod log_window;
mod plugin_manager_window;
mod tray;

use bootstrap::App;
use std::sync::Arc;
use windows_sys::Win32::UI::WindowsAndMessaging::*;
use plugin_api::{AppStarted, PluginDisabled, PluginEnabled, Event};
use crate::hook_manager::{set_eventbus_ptr, set_logger_ptr};

fn main() {
    let app = App::bootstrap();

    {
        let hk = app.hook_manager.clone();
        let pm = app.plugin_manager.clone();
        app.eventbus.subscribe("plugin.enabled", Arc::new(move |ev: Arc<dyn Event>| {
            if let Some(pe) = ev.as_any().downcast_ref::<PluginEnabled>() {
                if pe.needs_hook && !hk.has_mouse_hook() {
                    let _ = hk.register_mouse_hook();
                    pm.logger().info("core", "Mouse hook registered (runtime)");
                }
            }
        }));
    }
    {
        let hk = app.hook_manager.clone();
        let pm = app.plugin_manager.clone();
        app.eventbus.subscribe("plugin.disabled", Arc::new(move |ev: Arc<dyn Event>| {
            if let Some(_pd) = ev.as_any().downcast_ref::<PluginDisabled>() {
                if hk.has_mouse_hook() && !pm.any_hook_requested() {
                    hk.unregister_mouse_hook();
                    pm.logger().info("core", "Mouse hook unregistered (no plugin needs it)");
                }
            }
        }));
    }

    set_eventbus_ptr(Arc::into_raw(app.eventbus.clone()) as *mut std::ffi::c_void);
    set_logger_ptr(Arc::into_raw(app.logger.clone()) as *mut std::ffi::c_void);
    plugin_manager_window::set_pm(app.plugin_manager.clone());

    let started = Arc::new(AppStarted);
    app.eventbus.publish(started);

    app.logger.info("core", "CapaHub started — entering message loop");
    run_message_loop();

    app.logger.info("core", "Shutting down...");
    app.plugin_manager.unload_all();
    app.hook_manager.unregister_mouse_hook();
    app.logger.info("core", "CapaHub exited");
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
