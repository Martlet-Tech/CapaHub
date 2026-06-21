use core::eventbus::EventBus;
use core::logger::Logger;
use plugin_api::{Event, KeyDown, KeyEvent, KeyUp, MouseButton, MouseDown, MouseEvent, MouseMove, MouseUp};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use windows_sys::Win32::UI::WindowsAndMessaging::*;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleA;
use std::ffi::c_void;

pub struct HookManager {
    eventbus: Arc<EventBus>,
    mouse_hook: Mutex<Option<isize>>,
    keyboard_hook: Mutex<Option<isize>>,
}

impl HookManager {
    pub fn new(eventbus: Arc<EventBus>) -> Self {
        HookManager {
            eventbus,
            mouse_hook: Mutex::new(None),
            keyboard_hook: Mutex::new(None),
        }
    }

    pub fn register_mouse_hook(&self) -> Result<(), &'static str> {
        let mut hook = self.mouse_hook.lock().unwrap();
        if hook.is_some() {
            return Ok(());
        }

        let hook_proc: HOOKPROC = Some(mouse_proc_callback);

        let hook_handle = unsafe {
            let module = GetModuleHandleA(std::ptr::null());
            SetWindowsHookExA(WH_MOUSE_LL, hook_proc, module, 0)
        };

        if hook_handle.is_null() {
            return Err("SetWindowsHookEx failed");
        }

        *hook = Some(hook_handle as isize);
        Ok(())
    }

    pub fn unregister_mouse_hook(&self) {
        let mut hook = self.mouse_hook.lock().unwrap();
        if let Some(h) = hook.take() {
            unsafe {
                UnhookWindowsHookEx(h as *mut c_void);
            }
        }
    }

    pub fn has_mouse_hook(&self) -> bool {
        self.mouse_hook.lock().unwrap().is_some()
    }

    pub fn register_keyboard_hook(&self) -> Result<(), &'static str> {
        let mut hook = self.keyboard_hook.lock().unwrap();
        if hook.is_some() { return Ok(()); }
        let hook_proc: HOOKPROC = Some(keyboard_proc_callback);
        let hook_handle = unsafe {
            let module = GetModuleHandleA(std::ptr::null());
            SetWindowsHookExA(WH_KEYBOARD_LL, hook_proc, module, 0)
        };
        if hook_handle.is_null() { return Err("SetWindowsHookEx WH_KEYBOARD_LL failed"); }
        *hook = Some(hook_handle as isize);
        Ok(())
    }

    pub fn unregister_keyboard_hook(&self) {
        let mut hook = self.keyboard_hook.lock().unwrap();
        if let Some(h) = hook.take() {
            unsafe { UnhookWindowsHookEx(h as *mut c_void); }
        }
    }

    pub fn has_keyboard_hook(&self) -> bool {
        self.keyboard_hook.lock().unwrap().is_some()
    }
}

unsafe extern "system" fn mouse_proc_callback(ncode: i32, wparam: usize, lparam: isize) -> isize {
    if ncode < 0 {
        return CallNextHookEx(std::ptr::null_mut(), ncode, wparam, lparam);
    }

    let msg_id = wparam as u32;

    if msg_id != WM_MOUSEMOVE && msg_id != WM_LBUTTONDOWN && msg_id != WM_LBUTTONUP
        && msg_id != WM_RBUTTONDOWN && msg_id != WM_RBUTTONUP
        && msg_id != WM_MBUTTONDOWN && msg_id != WM_MBUTTONUP
    {
        return CallNextHookEx(std::ptr::null_mut(), ncode, wparam, lparam);
    }

    let info = &*(lparam as *const MSLLHOOKSTRUCT);

    let button = match msg_id {
        WM_LBUTTONDOWN | WM_LBUTTONUP => MouseButton::Left,
        WM_RBUTTONDOWN | WM_RBUTTONUP => MouseButton::Right,
        WM_MBUTTONDOWN | WM_MBUTTONUP => MouseButton::Middle,
        _ => MouseButton::None,
    };

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    let mouse_event = MouseEvent {
        button,
        x: info.pt.x,
        y: info.pt.y,
        timestamp,
    };

    let event_bus_ptr = get_eventbus_ptr();
    if !event_bus_ptr.is_null() {
        let eventbus = &*(event_bus_ptr as *const EventBus);
        let arc_event: Arc<dyn Event> = match msg_id {
            WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MBUTTONDOWN => {
                Arc::new(MouseDown(mouse_event))
            }
            WM_LBUTTONUP | WM_RBUTTONUP | WM_MBUTTONUP => {
                Arc::new(MouseUp(mouse_event))
            }
            _ => {
                Arc::new(MouseMove(mouse_event))
            }
        };

        let logger_ptr = get_logger_ptr();
        if !logger_ptr.is_null() {
            let logger = &*(logger_ptr as *const Logger);
            if msg_id == WM_MOUSEMOVE {
                if MOVE_COUNT == 0 {
                    MOVE_START_X = info.pt.x;
                    MOVE_START_Y = info.pt.y;
                }
                MOVE_COUNT += 1;
                if MOVE_COUNT >= 100 {
                    logger.debug("hook", &format!("mouse move x{} ({}→{})", MOVE_COUNT, format_pos(MOVE_START_X, MOVE_START_Y), format_pos(info.pt.x, info.pt.y)));
                    MOVE_COUNT = 0;
                }
            } else {
                let btn_name = match msg_id {
                    WM_LBUTTONDOWN | WM_LBUTTONUP => "Left",
                    WM_RBUTTONDOWN | WM_RBUTTONUP => "Right",
                    WM_MBUTTONDOWN | WM_MBUTTONUP => "Middle",
                    _ => "None",
                };
                let action = if msg_id == WM_LBUTTONDOWN || msg_id == WM_RBUTTONDOWN || msg_id == WM_MBUTTONDOWN { "down" } else { "up" };
                logger.debug("hook", &format!("mouse {} {} at {}", action, btn_name, format_pos(info.pt.x, info.pt.y)));
            }
        }

        eventbus.publish(arc_event);
    }

    if EAT_MOUSE { return 1; }

    CallNextHookEx(std::ptr::null_mut(), ncode, wparam, lparam)
}

unsafe extern "system" fn keyboard_proc_callback(ncode: i32, wparam: usize, lparam: isize) -> isize {
    if ncode < 0 {
        return CallNextHookEx(std::ptr::null_mut(), ncode, wparam, lparam);
    }
    let pressed = wparam as u32 == WM_KEYDOWN || wparam as u32 == WM_SYSKEYDOWN;
    let info = &*(lparam as *const KBDLLHOOKSTRUCT);
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64;
    let key_event = KeyEvent { vk_code: info.vkCode, pressed, timestamp };

    let event_bus_ptr = get_eventbus_ptr();
    if !event_bus_ptr.is_null() {
        let eventbus = &*(event_bus_ptr as *const EventBus);
        let arc_event: Arc<dyn Event> = if pressed {
            Arc::new(KeyDown(key_event))
        } else {
            Arc::new(KeyUp(key_event))
        };
        eventbus.publish(arc_event);
    }

    if EAT_MOUSE { return 1; }

    CallNextHookEx(std::ptr::null_mut(), ncode, wparam, lparam)
}

static mut EVENTBUS_PTR: *mut c_void = std::ptr::null_mut();
static mut LOGGER_PTR: *mut c_void = std::ptr::null_mut();
static mut MOVE_COUNT: u32 = 0;
static mut MOVE_START_X: i32 = 0;
static mut MOVE_START_Y: i32 = 0;

static mut EAT_MOUSE: bool = false;
pub fn set_eat_mouse(eat: bool) { unsafe { EAT_MOUSE = eat; } }

pub(crate) fn set_eventbus_ptr(ptr: *mut c_void) {
    unsafe { EVENTBUS_PTR = ptr; }
}

pub(crate) fn set_logger_ptr(ptr: *mut c_void) {
    unsafe { LOGGER_PTR = ptr; }
}

pub(crate) fn get_eventbus_ptr() -> *mut c_void {
    unsafe { EVENTBUS_PTR }
}

fn get_logger_ptr() -> *mut c_void {
    unsafe { LOGGER_PTR }
}

fn format_pos(x: i32, y: i32) -> String {
    format!("{},{}", x, y)
}
