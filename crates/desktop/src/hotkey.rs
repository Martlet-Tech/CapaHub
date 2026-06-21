// Hotkey capability — RegisterHotKey / WM_HOTKEY dispatch to events.
use crate::hook_manager;
use core::eventbus::EventBus;
use std::sync::Arc;

pub fn init() {
    unsafe {
        // F5 → screenshot
        windows_sys::Win32::UI::Input::KeyboardAndMouse::RegisterHotKey(std::ptr::null_mut(), 2, 0, 0x74);
    }
}

pub fn on_hotkey(id: i32) -> bool {
    let ptr = hook_manager::get_eventbus_ptr();
    if ptr.is_null() { return false; }
    let eb = unsafe { &*(ptr as *const EventBus) };
    match id {
        1 => eb.publish(Arc::new(crate::clipboard::events::ShowClipboard)),
        2 => eb.publish(Arc::new(core::event::DynamicEvent { event_type: "screenshot.capture", repr: String::new() })),
        _ => return false,
    }
    true
}
