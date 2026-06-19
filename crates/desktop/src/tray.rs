use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;
use windows_sys::Win32::UI::Shell::*;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleA;

#[link(name = "user32")]
extern "system" {
    fn GetCursorPos(lppoint: *mut POINT) -> i32;
}

const WM_TRAY_NOTIFY: u32 = WM_APP + 1;
const ID_TRAY_EXIT: usize = 1001;
const ID_TRAY_SHOW_LOG: usize = 1002;
const GWLP_WNDPROC: i32 = -4;

pub struct TrayIcon {
    hwnd: HWND,
}

impl TrayIcon {
    pub fn new() -> Self {
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

        unsafe {
            let mut nid = std::mem::zeroed::<NOTIFYICONDATAW>();
            nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
            nid.hWnd = hwnd;
            nid.uID = 1;
            nid.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
            nid.uCallbackMessage = WM_TRAY_NOTIFY;
            nid.hIcon = LoadIconW(std::ptr::null_mut(), IDI_APPLICATION);
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
        unsafe {
            SetWindowLongPtrW(self.hwnd, GWLP_USERDATA, log_hwnd as isize);
        }
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
        if lparam as u32 == WM_RBUTTONUP {
            let hmenu = CreatePopupMenu();
            let s1: Vec<u16> = "Show Log\0".encode_utf16().collect();
            AppendMenuW(hmenu, MF_STRING, ID_TRAY_SHOW_LOG, s1.as_ptr());
            AppendMenuW(hmenu, MF_SEPARATOR, 0, std::ptr::null());
            let s2: Vec<u16> = "Exit\0".encode_utf16().collect();
            AppendMenuW(hmenu, MF_STRING, ID_TRAY_EXIT, s2.as_ptr());

            let mut pos = std::mem::zeroed::<POINT>();
            GetCursorPos(&mut pos);
            SetForegroundWindow(hwnd);
            TrackPopupMenu(hmenu, TPM_LEFTALIGN | TPM_BOTTOMALIGN, pos.x, pos.y, 0, hwnd, std::ptr::null());
            DestroyMenu(hmenu);
        }
        return 0;
    }

    if msg == WM_COMMAND {
        match wparam {
            ID_TRAY_EXIT => {
                PostQuitMessage(0);
                return 0;
            }
            ID_TRAY_SHOW_LOG => {
                let log_hwnd = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as HWND;
                if !log_hwnd.is_null() {
                    ShowWindow(log_hwnd, SW_SHOW);
                    SetForegroundWindow(log_hwnd);
                }
                return 0;
            }
            _ => {}
        }
    }

    DefWindowProcW(hwnd, msg, wparam, lparam)
}
