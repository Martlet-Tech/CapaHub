// Hotkey capability — RegisterHotKey / WM_HOTKEY dispatch to events.
use crate::hook_manager;
use core::eventbus::EventBus;
use std::sync::Arc;

pub fn init() {
    unsafe {
        // 1: Ctrl+Shift+V → clipboard
        windows_sys::Win32::UI::Input::KeyboardAndMouse::RegisterHotKey(std::ptr::null_mut(), 1, 0x0002 | 0x0004, 'V' as u32);
        // 2: F5 → screenshot
        windows_sys::Win32::UI::Input::KeyboardAndMouse::RegisterHotKey(std::ptr::null_mut(), 2, 0, 0x74);
        // 3: F6 → counter / user-defined
        windows_sys::Win32::UI::Input::KeyboardAndMouse::RegisterHotKey(std::ptr::null_mut(), 3, 0, 0x75);
    }
}

pub fn on_hotkey(id: i32) -> bool {
    let ptr = hook_manager::get_eventbus_ptr();
    if ptr.is_null() { return false; }
    let eb = unsafe { &*(ptr as *const EventBus) };
    match id {
        1 => eb.publish(Arc::new(crate::clipboard::events::ShowClipboard)),
        2 => eb.publish(Arc::new(core::event::DynamicEvent { event_type: "screenshot.capture", repr: String::new() })),
        3 => eb.publish(Arc::new(core::event::DynamicEvent { event_type: "hotkey.f6", repr: String::new() })),
        _ => return false,
    }
    true
}
