mod act;
mod bootstrap;
mod clipboard;
mod hook_manager;
mod hotkey;
mod icon_loader;
mod log_window;
mod overlay;
mod plugin_manager_window;
mod tray;
mod webview_host;
mod window_manager;

use bootstrap::App;
use std::sync::Arc;
use windows_sys::Win32::UI::WindowsAndMessaging::*;
use plugin_api::{AppStarted, Event, PluginDisabled, PluginEnabled};
use crate::hook_manager::{set_eventbus_ptr, set_logger_ptr};

fn main() {
    // DPI awareness — ensure overlay coordinates match raw mouse input
    unsafe { windows_sys::Win32::UI::HiDpi::SetProcessDpiAwareness(windows_sys::Win32::UI::HiDpi::PROCESS_PER_MONITOR_DPI_AWARE); }

    // false=不写文件(看是否卡死)  true=写文件(正常)
    core::logger::set_file_logging(true);

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
                    pm.logger().info("core", "Mouse hook unregistered");
                }
            }
        }));
    }

    set_eventbus_ptr(Arc::into_raw(app.eventbus.clone()) as *mut std::ffi::c_void);
    set_logger_ptr(Arc::into_raw(app.logger.clone()) as *mut std::ffi::c_void);
    plugin_manager_window::set_pm(app.plugin_manager.clone());

    {
        let log2 = app.logger.clone();
        core::capability::register_intent(std::sync::Arc::new(move |intent| {
            use core::render_intent::RenderIntent;
            match intent {
                RenderIntent::Window(cfg) => {
                    log2.info("core", &format!("render window: {}x{}", cfg.width, cfg.height));
                    window_manager::spawn_window(cfg);
                }
                RenderIntent::Overlay(_) => log2.warn("core", "Overlay not implemented"),
            }
        }));
    }

    {
        let log3 = app.logger.clone();
        core::capability::register_paste(std::sync::Arc::new(move |text: String| {
            log3.debug("core", &format!("paste: {} chars", text.len()));
            crate::clipboard::paste(&text);
        }));
    }

    core::capability::register_read_text(std::sync::Arc::new(|| crate::clipboard::read_text()));

    core::capability::register_save_file(std::sync::Arc::new(|content: String, default_name: String| {
        let wide_path = save_file_dialog(&default_name);
        if let Some(path) = wide_path {
            let _ = std::fs::write(&path, &content);
        }
    }));

    core::capability::register_send_keys(std::sync::Arc::new(|keys: String| {
        crate::act::input::send_keys(&keys);
    }));

    crate::overlay::init();
    core::capability::register_overlay(std::sync::Arc::new(|json: String| {
        crate::overlay::handle_cmd(&json)
    }));

    core::capability::register_capture(std::sync::Arc::new(|x: i32, y: i32, w: i32, h: i32| {
        crate::act::screen::capture(x, y, w, h)
    }));

    crate::clipboard::init(app.tray.hwnd as isize, &app.logger);
    crate::hotkey::init();

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
            0 | -1 => break,
            _ => {
                if msg.message == WM_HOTKEY && msg.wParam == 1 {
                    crate::hotkey::on_hotkey(1);
                    continue;
                }
                if msg.message == WM_HOTKEY && msg.wParam == 2 {
                    crate::hotkey::on_hotkey(2);
                    continue;
                }
                if msg.message == WM_HOTKEY && msg.wParam == 3 {
                    crate::hotkey::on_hotkey(3);
                    continue;
                }
                unsafe {
                    TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }
        }
    }
}

#[link(name = "comdlg32")]
extern "system" {
    fn GetSaveFileNameW(param: *mut crate::plugin_manager_window::OPENFILENAMEW) -> i32;
}

fn save_file_dialog(default_name: &str) -> Option<String> {
    let mut buf = [0u16; 1024];
    for (i, c) in default_name.encode_utf16().enumerate() {
        if i < buf.len() - 1 { buf[i] = c; }
    }
    let filter: Vec<u16> = "XML Files (*.xml)\0*.xml\0All Files (*.*)\0*.*\0\0".encode_utf16().collect();
    let def_ext: Vec<u16> = "xml\0".encode_utf16().collect();
    let mut ofn = crate::plugin_manager_window::OPENFILENAMEW {
        lStructSize: std::mem::size_of::<crate::plugin_manager_window::OPENFILENAMEW>() as u32,
        hwndOwner: std::ptr::null_mut(),
        hInstance: std::ptr::null_mut(),
        lpstrFilter: filter.as_ptr(),
        lpstrCustomFilter: std::ptr::null_mut(),
        nMaxCustFilter: 0,
        nFilterIndex: 1,
        lpstrFile: buf.as_mut_ptr(),
        nMaxFile: 1024,
        lpstrFileTitle: std::ptr::null_mut(),
        nMaxFileTitle: 0,
        lpstrInitialDir: std::ptr::null(),
        lpstrTitle: std::ptr::null(),
        Flags: 0x0002,
        nFileOffset: 0,
        nFileExtension: 0,
        lpstrDefExt: def_ext.as_ptr(),
        lCustData: 0,
        lpfnHook: None,
        lpTemplateName: std::ptr::null(),
        pvReserved: std::ptr::null_mut(),
        dwReserved: 0,
        FlagsEx: 0,
    };
    let result = unsafe { GetSaveFileNameW(&mut ofn) };
    if result == 0 { return None; }
    let path_len = buf.iter().position(|&c| c == 0).unwrap_or(0);
    Some(String::from_utf16_lossy(&buf[..path_len]))
}
