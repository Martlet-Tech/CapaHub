mod bootstrap;
mod hook_manager;
mod icon_loader;
mod log_window;
mod plugin_manager_window;
mod tray;
mod window_manager;

use bootstrap::App;
use std::sync::Arc;
use windows_sys::Win32::UI::WindowsAndMessaging::*;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
use plugin_api::{AppStarted, Event, PluginDisabled, PluginEnabled, ShowClipboard};
use crate::hook_manager::{get_eventbus_ptr, set_eventbus_ptr, set_logger_ptr};

const MOD_CONTROL: u32 = 0x0002;
const MOD_SHIFT: u32 = 0x0004;

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
        core::js_runtime::set_js_intent_callback(Box::new(move |intent| {
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
        core::js_runtime::set_clipboard_paste_callback(Box::new(move |text: String| {
            log3.debug("core", &format!("paste: {} chars", text.len()));
            paste_text(&text);
        }));
    }

    core::js_runtime::set_clipboard_read_callback(Box::new(|| clipboard_read_text()));

    unsafe {
        RegisterHotKey(std::ptr::null_mut(), 1, MOD_CONTROL | MOD_SHIFT, 'V' as u32);
        let ok = AddClipboardFormatListener(app.tray.hwnd);
        if ok == 0 {
            app.logger.warn("core", "AddClipboardFormatListener failed");
        } else {
            app.logger.info("core", "Clipboard listener registered");
        }
    }

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
                    let ptr = get_eventbus_ptr();
                    if !ptr.is_null() {
                        let eb = unsafe { &*(ptr as *const core::eventbus::EventBus) };
                        eb.publish(Arc::new(ShowClipboard));
                    }
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

#[link(name = "user32")]
extern "system" {
    fn RegisterHotKey(hwnd: *mut std::ffi::c_void, id: i32, fsModifiers: u32, vk: u32) -> i32;
    fn AddClipboardFormatListener(hwnd: *mut std::ffi::c_void) -> i32;
}

fn paste_text(text: &str) {
    unsafe {
        if OpenClipboard(std::ptr::null_mut()) == 0 {
            return;
        }
        let _ = EmptyClipboard();

        // Set clipboard to our text as CF_UNICODETEXT (13)
        let wide: Vec<u16> = text.encode_utf16().chain(Some(0)).collect();
        let h = GlobalAlloc(0x0002, (wide.len() * 2) as usize); // GMEM_MOVEABLE
        if !h.is_null() {
            let dst = GlobalLock(h) as *mut u16;
            if !dst.is_null() {
                std::ptr::copy_nonoverlapping(wide.as_ptr(), dst, wide.len());
                GlobalUnlock(h);
            }
            crate::hook_manager::CLIPBOARD_SELF_CHANGE.store(true, std::sync::atomic::Ordering::SeqCst);
            SetClipboardData(13, h); // CF_UNICODETEXT = 13
        }

        CloseClipboard();

        // Simulate Ctrl+V
        let mut inputs = [
            INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: VK_CONTROL as u16, wScan: 0, dwFlags: 0, time: 0, dwExtraInfo: 0 } },
            },
            INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: 'V' as u16, wScan: 0, dwFlags: 0, time: 0, dwExtraInfo: 0 } },
            },
            INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: 'V' as u16, wScan: 0, dwFlags: KEYEVENTF_KEYUP, time: 0, dwExtraInfo: 0 } },
            },
            INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: VK_CONTROL as u16, wScan: 0, dwFlags: KEYEVENTF_KEYUP, time: 0, dwExtraInfo: 0 } },
            },
        ];
        SendInput(4, inputs.as_mut_ptr(), std::mem::size_of::<INPUT>() as i32);
    }
}

fn clipboard_read_text() -> Option<String> {
    unsafe {
        if OpenClipboard(std::ptr::null_mut()) == 0 {
            return None;
        }
        let h = GetClipboardData(13); // CF_UNICODETEXT
        if h.is_null() {
            CloseClipboard();
            return None;
        }
        let ptr = GlobalLock(h) as *const u16;
        if ptr.is_null() {
            CloseClipboard();
            return None;
        }
        let mut len = 0;
        while *ptr.add(len) != 0 { len += 1; }
        let text = String::from_utf16_lossy(std::slice::from_raw_parts(ptr, len));
        GlobalUnlock(h);
        CloseClipboard();
        Some(text)
    }
}

#[link(name = "user32")]
extern "system" {
    fn OpenClipboard(hwnd: *mut std::ffi::c_void) -> i32;
    fn CloseClipboard() -> i32;
    fn EmptyClipboard() -> i32;
    fn SetClipboardData(uFormat: u32, hMem: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
    fn GetClipboardData(uFormat: u32) -> *mut std::ffi::c_void;
}

#[link(name = "kernel32")]
extern "system" {
    fn GlobalAlloc(uFlags: u32, dwBytes: usize) -> *mut std::ffi::c_void;
    fn GlobalLock(hMem: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
    fn GlobalUnlock(hMem: *mut std::ffi::c_void) -> i32;
}

#[link(name = "user32")]
extern "system" {
    fn SendInput(cInputs: u32, pInputs: *mut INPUT, cbSize: i32) -> u32;
}
