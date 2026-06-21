// Clipboard capability — Win32 clipboard read/write, listener, hotkey.
pub mod events;

use crate::hook_manager;
use core::eventbus::EventBus;
use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;

const MOD_CONTROL: u32 = 0x0002;
const MOD_SHIFT: u32 = 0x0004;
const CF_UNICODETEXT: u32 = 13;

static CLIPBOARD_SELF_CHANGE: AtomicBool = AtomicBool::new(false);

#[link(name = "user32")]
extern "system" {
    fn OpenClipboard(hwnd: *mut c_void) -> i32;
    fn CloseClipboard() -> i32;
    fn EmptyClipboard() -> i32;
    fn SetClipboardData(uFormat: u32, hMem: *mut c_void) -> *mut c_void;
    fn GetClipboardData(uFormat: u32) -> *mut c_void;
}

#[link(name = "kernel32")]
extern "system" {
    fn GlobalAlloc(uFlags: u32, dwBytes: usize) -> *mut c_void;
    fn GlobalLock(hMem: *mut c_void) -> *mut c_void;
    fn GlobalUnlock(hMem: *mut c_void) -> i32;
}

#[link(name = "user32")]
extern "system" {
    fn RegisterHotKey(hwnd: *mut c_void, id: i32, fsModifiers: u32, vk: u32) -> i32;
    fn AddClipboardFormatListener(hwnd: *mut c_void) -> i32;
    fn SendInput(cInputs: u32, pInputs: *mut INPUT, cbSize: i32) -> u32;
}

/// Called once at startup. hwnd is the tray message window.
pub fn init(hwnd: isize, logger: &core::logger::Logger) {
    unsafe {
        RegisterHotKey(std::ptr::null_mut(), 1, MOD_CONTROL | MOD_SHIFT, 'V' as u32);
        let ok = AddClipboardFormatListener(hwnd as *mut c_void);
        if ok == 0 {
            logger.warn("clipboard", "AddClipboardFormatListener failed");
        } else {
            logger.info("clipboard", "Clipboard listener registered");
        }
    }
}

/// Handler for WM_HOTKEY. Returns true if the hotkey was consumed.
pub fn on_hotkey() -> bool {
    let ptr = hook_manager::get_eventbus_ptr();
    if ptr.is_null() { return false; }
    let eb = unsafe { &*(ptr as *const EventBus) };
    eb.publish(Arc::new(events::ShowClipboard));
    true
}

/// Handler for WM_CLIPBOARDUPDATE. Checks self-change flag, reads text, publishes event.
pub fn on_clipboard_update() {
    if CLIPBOARD_SELF_CHANGE.swap(false, Ordering::SeqCst) {
        return;
    }
    let ptr = hook_manager::get_eventbus_ptr();
    if ptr.is_null() { return; }
    let eb = unsafe { &*(ptr as *const EventBus) };
    if let Some(text) = read_text() {
        eb.publish(Arc::new(events::ClipboardChanged { text }));
    }
}

/// Read text from clipboard (CF_UNICODETEXT).
pub fn read_text() -> Option<String> {
    unsafe {
        if OpenClipboard(std::ptr::null_mut()) == 0 {
            return None;
        }
        let h = GetClipboardData(CF_UNICODETEXT);
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

/// Write text to clipboard, set self-change flag, then simulate Ctrl+V.
pub fn paste(text: &str) {
    unsafe {
        if OpenClipboard(std::ptr::null_mut()) == 0 { return; }
        let _ = EmptyClipboard();
        let wide: Vec<u16> = text.encode_utf16().chain(Some(0)).collect();
        let h = GlobalAlloc(0x0002, (wide.len() * 2) as usize);
        if !h.is_null() {
            let dst = GlobalLock(h) as *mut u16;
            if !dst.is_null() {
                std::ptr::copy_nonoverlapping(wide.as_ptr(), dst, wide.len());
                GlobalUnlock(h);
            }
            CLIPBOARD_SELF_CHANGE.store(true, Ordering::SeqCst);
            SetClipboardData(CF_UNICODETEXT, h);
        }
        CloseClipboard();

        let mut inputs = [
            INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: windows_sys::Win32::UI::Input::KeyboardAndMouse::INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: VK_CONTROL as u16, wScan: 0, dwFlags: 0, time: 0, dwExtraInfo: 0,
                    },
                },
            },
            INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: windows_sys::Win32::UI::Input::KeyboardAndMouse::INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: 'V' as u16, wScan: 0, dwFlags: 0, time: 0, dwExtraInfo: 0,
                    },
                },
            },
            INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: windows_sys::Win32::UI::Input::KeyboardAndMouse::INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: 'V' as u16, wScan: 0, dwFlags: KEYEVENTF_KEYUP, time: 0, dwExtraInfo: 0,
                    },
                },
            },
            INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: windows_sys::Win32::UI::Input::KeyboardAndMouse::INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: VK_CONTROL as u16, wScan: 0, dwFlags: KEYEVENTF_KEYUP, time: 0, dwExtraInfo: 0,
                    },
                },
            },
        ];
        SendInput(4, inputs.as_mut_ptr(), std::mem::size_of::<INPUT>() as i32);
    }
}

/// Write text to clipboard without simulating paste.
#[allow(dead_code)]
pub fn write_text(text: &str) {
    unsafe {
        if OpenClipboard(std::ptr::null_mut()) == 0 { return; }
        let _ = EmptyClipboard();
        let wide: Vec<u16> = text.encode_utf16().chain(Some(0)).collect();
        let h = GlobalAlloc(0x0002, (wide.len() * 2) as usize);
        if !h.is_null() {
            let dst = GlobalLock(h) as *mut u16;
            if !dst.is_null() {
                std::ptr::copy_nonoverlapping(wide.as_ptr(), dst, wide.len());
                GlobalUnlock(h);
            }
            CLIPBOARD_SELF_CHANGE.store(true, Ordering::SeqCst);
            SetClipboardData(CF_UNICODETEXT, h);
        }
        CloseClipboard();
    }
}
