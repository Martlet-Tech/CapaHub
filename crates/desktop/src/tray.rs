use core::plugin_manager::PluginManager;
use std::ffi::c_void;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;
use windows_sys::Win32::UI::Shell::*;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleA;

#[link(name = "user32")]
extern "system" {
    fn GetCursorPos(lppoint: *mut POINT) -> i32;
}

const WM_TRAY_NOTIFY: u32 = WM_APP + 1;
const ID_TRAY_SHOW_LOG: usize = 1001;
const ID_TRAY_MANAGER: usize = 1002;
const ID_TRAY_EXIT: usize = 1099;
const ID_TRAY_PLUGIN_BASE: usize = 2000;
const GWLP_WNDPROC: i32 = -4;
const WM_CLIPBOARDUPDATE: u32 = 0x031D;

static mut LOG_HWND: HWND = std::ptr::null_mut();
static mut PM: Option<Arc<PluginManager>> = None;

pub struct TrayIcon {
    pub hwnd: HWND,
}

impl TrayIcon {
    pub fn new() -> Self {
        TrayIcon::with_icon(std::ptr::null_mut())
    }

    pub fn with_icon(hicon: *mut c_void) -> Self {
        let hinstance = unsafe { GetModuleHandleA(std::ptr::null()) };

        let hwnd = unsafe {
            CreateWindowExW(
                0,
                "STATIC\0".encode_utf16().collect::<Vec<u16>>().as_ptr(),
                std::ptr::null(),
                0,
                0, 0, 0, 0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                hinstance,
                std::ptr::null(),
            )
        };

        let icon = if !hicon.is_null() {
            hicon
        } else {
            unsafe { LoadIconW(std::ptr::null_mut(), IDI_APPLICATION) }
        };

        unsafe {
            let mut nid = std::mem::zeroed::<NOTIFYICONDATAW>();
            nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
            nid.hWnd = hwnd;
            nid.uID = 1;
            nid.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
            nid.uCallbackMessage = WM_TRAY_NOTIFY;
            nid.hIcon = icon;
            let tip: Vec<u16> = "CapaHub\0".encode_utf16().collect();
            let mut i = 0;
            while i < 63 && i < tip.len() {
                nid.szTip[i] = tip[i];
                i += 1;
            }
            Shell_NotifyIconW(NIM_ADD, &nid);
        }

        unsafe {
            SetWindowLongPtrW(hwnd, GWLP_WNDPROC, tray_wndproc as *const () as usize as isize);
        }

        TrayIcon { hwnd }
    }

    pub fn set_log_hwnd(&self, log_hwnd: HWND) {
        unsafe { LOG_HWND = log_hwnd; }
    }

    pub fn set_plugin_manager(&self, pm: Arc<PluginManager>) {
        unsafe { PM = Some(pm); }
    }
}

impl Drop for TrayIcon {
    fn drop(&mut self) {
        unsafe {
            let mut nid = std::mem::zeroed::<NOTIFYICONDATAW>();
            nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
            nid.hWnd = self.hwnd;
            nid.uID = 1;
            Shell_NotifyIconW(NIM_DELETE, &nid);
            DestroyWindow(self.hwnd);
        }
    }
}

unsafe extern "system" fn tray_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: usize,
    lparam: isize,
) -> isize {
    if msg == WM_TRAY_NOTIFY {
        let action = lparam as u32;
        if action == WM_LBUTTONDBLCLK {
            crate::plugin_manager_window::open_manager_window();
            return 0;
        }
        if action == WM_RBUTTONUP {
            show_tray_menu(hwnd);
            return 0;
        }
        return 0;
    }

    if msg == WM_COMMAND {
        match wparam {
            ID_TRAY_SHOW_LOG => {
                if !LOG_HWND.is_null() {
                    ShowWindow(LOG_HWND, SW_SHOW);
                    SetForegroundWindow(LOG_HWND);
                }
                return 0;
            }
            ID_TRAY_MANAGER => {
                crate::plugin_manager_window::open_manager_window();
                return 0;
            }
            ID_TRAY_EXIT => {
                PostQuitMessage(0);
                return 0;
            }
            _ => {
                if wparam >= ID_TRAY_PLUGIN_BASE {
                    let idx = wparam - ID_TRAY_PLUGIN_BASE;
                    if let Some(ref pm) = PM {
                        let plugins = pm.plugin_list();
                        if idx < plugins.len() {
                            let name = &plugins[idx].name;
                            let eventbus_ptr = crate::hook_manager::get_eventbus_ptr();
                            if !eventbus_ptr.is_null() {
                                let eb = unsafe { &*(eventbus_ptr as *const core::eventbus::EventBus) };
                                eb.publish(std::sync::Arc::new(core::event::PluginActivate { name: name.clone() }));
                            }
                            if let Some(dir) = pm.plugin_dir(name) {
                                let html_path = dir.join("index.html");
                                if html_path.exists() {
                                    let path_str = html_path.to_string_lossy().to_string();
                                    let title = format!("{} - CapaHub", name);
                                    let _ = crate::webview_host::create_webview_window(
                                        &path_str,
                                        &title,
                                        800,
                                        600,
                                    );
                                }
                            }
                        }
                    }
                    return 0;
                }
                return 0;
            }
        }
    }

    if msg == WM_CLIPBOARDUPDATE {
        if crate::hook_manager::CLIPBOARD_SELF_CHANGE.swap(false, Ordering::SeqCst) {
            return 0;
        }
        let eventbus_ptr = crate::hook_manager::get_eventbus_ptr();
        if !eventbus_ptr.is_null() {
            let eb = unsafe { &*(eventbus_ptr as *const core::eventbus::EventBus) };
            if let Some(text) = get_clipboard_text() {
                eb.publish(std::sync::Arc::new(core::event::ClipboardChanged { text }));
            }
        }
        return 0;
    }

    DefWindowProcW(hwnd, msg, wparam, lparam)
}

fn get_clipboard_text() -> Option<String> {
    unsafe {
        if OpenClipboard(std::ptr::null_mut()) == 0 {
            return None;
        }
        let h = GetClipboardData(13);
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
    fn OpenClipboard(hwnd: *mut c_void) -> i32;
    fn CloseClipboard() -> i32;
    fn GetClipboardData(format: u32) -> *mut c_void;
    fn GlobalLock(h: *mut c_void) -> *mut c_void;
    fn GlobalUnlock(h: *mut c_void) -> i32;
}

unsafe fn show_tray_menu(hwnd: HWND) {
    let hmenu = CreatePopupMenu();

    let s1: Vec<u16> = "Show Log\0".encode_utf16().collect();
    AppendMenuW(hmenu, MF_STRING, ID_TRAY_SHOW_LOG, s1.as_ptr());

    let s2: Vec<u16> = "Plugin Manager\0".encode_utf16().collect();
    AppendMenuW(hmenu, MF_STRING, ID_TRAY_MANAGER, s2.as_ptr());

    AppendMenuW(hmenu, MF_SEPARATOR, 0, std::ptr::null());

    if let Some(ref pm) = PM {
        let plugins = pm.plugin_list();
        for (i, info) in plugins.iter().enumerate() {
            if i >= 100 { break; }
            let wide: Vec<u16> = info.name.encode_utf16().chain(Some(0)).collect();
            AppendMenuW(hmenu, MF_STRING, ID_TRAY_PLUGIN_BASE + i, wide.as_ptr());
        }
    }

    AppendMenuW(hmenu, MF_SEPARATOR, 0, std::ptr::null());

    let s3: Vec<u16> = "Exit\0".encode_utf16().collect();
    AppendMenuW(hmenu, MF_STRING, ID_TRAY_EXIT, s3.as_ptr());

    let mut pos = std::mem::zeroed::<POINT>();
    GetCursorPos(&mut pos);
    SetForegroundWindow(hwnd);
    TrackPopupMenu(hmenu, TPM_LEFTALIGN | TPM_BOTTOMALIGN, pos.x, pos.y, 0, hwnd, std::ptr::null());
    DestroyMenu(hmenu);
}
