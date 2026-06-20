use core::logger::LogEntry;
use crossbeam_channel::Receiver;
use std::ffi::c_void;
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleA;
use crate::icon_loader;

const GWLP_WNDPROC: i32 = -4;
const WM_SETICON: u32 = 0x0080;
const ICON_SMALL: usize = 0;
const ICON_BIG: usize = 1;
const LBS_NOSEL: u32 = 0x4000;
const ID_CLEAR_BTN: usize = 2001;
const ID_PIN_BTN: usize = 2002;
const WS_EX_CLIENTEDGE: u32 = 0x00000200;
const BS_AUTOCHECKBOX: u32 = 0x0003;
const BM_SETCHECK: u32 = 0x00F1;
const BM_GETCHECK: u32 = 0x00F0;

type WndProc = unsafe extern "system" fn(HWND, u32, usize, isize) -> isize;
static mut ORIG_WNDPROC: Option<WndProc> = None;

#[link(name = "gdi32")]
extern "system" {
    fn CreateFontW(
        cHeight: i32, cWidth: i32, cEscapement: i32, cOrientation: i32,
        cWeight: i32, bItalic: i32, bUnderline: i32, bStrikeOut: i32,
        iCharSet: i32, iOutPrecision: i32, iClipPrecision: i32,
        iQuality: i32, iPitchAndFamily: i32, pszFaceName: *const u16,
    ) -> isize;
}

pub struct LogWindow {
    pub hwnd: HWND,
}

impl LogWindow {
    pub fn new(rx: Receiver<LogEntry>, _config: &core::config::Config) -> Self {
        let hinstance = unsafe { GetModuleHandleA(std::ptr::null()) };
        let hwnd = Self::create_main_window(hinstance);
        Self::set_window_icon(hwnd);
        Self::subclass_window(hwnd);
        let list_hwnd = Self::create_child_controls(hwnd, hinstance);
        Self::force_layout(hwnd);
        Self::set_listbox_font(list_hwnd);
        Self::spawn_log_thread(rx, list_hwnd);
        LogWindow { hwnd }
    }

    fn create_main_window(hinstance: *mut c_void) -> HWND {
        unsafe {
            CreateWindowExW(
                WS_EX_CLIENTEDGE,
                "#32770\0".encode_utf16().collect::<Vec<u16>>().as_ptr(),
                "CapaHub - Log\0".encode_utf16().collect::<Vec<u16>>().as_ptr(),
                WS_OVERLAPPEDWINDOW | WS_VISIBLE,
                CW_USEDEFAULT, CW_USEDEFAULT, 700, 530,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                hinstance,
                std::ptr::null(),
            )
        }
    }

    fn set_window_icon(hwnd: HWND) {
        let icon = icon_loader::get_tray_icon();
        unsafe {
            SendMessageW(hwnd, WM_SETICON, ICON_SMALL, icon as isize);
            SendMessageW(hwnd, WM_SETICON, ICON_BIG, icon as isize);
        }
    }

    fn subclass_window(hwnd: HWND) {
        unsafe {
            let old = SetWindowLongPtrW(hwnd, GWLP_WNDPROC, log_wndproc as *const () as usize as isize);
            ORIG_WNDPROC = Some(std::mem::transmute::<isize, WndProc>(old));
        }
    }

    fn create_child_controls(hwnd: HWND, hinstance: *mut c_void) -> HWND {
        let list_hwnd = unsafe {
            CreateWindowExW(
                0,
                "LISTBOX\0".encode_utf16().collect::<Vec<u16>>().as_ptr(),
                std::ptr::null(),
                WS_CHILD | WS_VISIBLE | WS_BORDER | WS_VSCROLL | LBS_NOSEL,
                10, 10, 660, 470,
                hwnd,
                std::ptr::null_mut(),
                hinstance,
                std::ptr::null(),
            )
        };

        unsafe {
            CreateWindowExW(
                0,
                "BUTTON\0".encode_utf16().collect::<Vec<u16>>().as_ptr(),
                "Clear\0".encode_utf16().collect::<Vec<u16>>().as_ptr(),
                WS_CHILD | WS_VISIBLE,
                10, 490, 60, 25,
                hwnd,
                ID_CLEAR_BTN as HMENU,
                hinstance,
                std::ptr::null(),
            );

            CreateWindowExW(
                0,
                "BUTTON\0".encode_utf16().collect::<Vec<u16>>().as_ptr(),
                "Pin on Top\0".encode_utf16().collect::<Vec<u16>>().as_ptr(),
                WS_CHILD | WS_VISIBLE | BS_AUTOCHECKBOX,
                110, 490, 90, 25,
                hwnd,
                ID_PIN_BTN as HMENU,
                hinstance,
                std::ptr::null(),
            );
        }

        list_hwnd
    }

    fn force_layout(hwnd: HWND) {
        unsafe {
            let mut rc = std::mem::zeroed::<RECT>();
            GetClientRect(hwnd, &mut rc);
            SendMessageW(hwnd, WM_SIZE, 0, ((rc.bottom << 16) | (rc.right & 0xFFFF)) as isize);
        }
    }

    fn set_listbox_font(list_hwnd: HWND) {
        if list_hwnd.is_null() { return; }
        let face: Vec<u16> = "Microsoft YaHei\0".encode_utf16().collect();
        let hfont = unsafe {
            CreateFontW(-15, 0, 0, 0, 400, 0, 0, 0, 1, 0, 0, 0, 0, face.as_ptr())
        };
        if hfont != 0 {
            unsafe { SendMessageW(list_hwnd, WM_SETFONT, hfont as usize, 1); }
        }
    }

    fn spawn_log_thread(rx: Receiver<LogEntry>, list_hwnd: HWND) {
        let list_ptr = list_hwnd as usize;
        std::thread::spawn(move || {
            for entry in rx {
                let line = Self::format_entry(&entry);
                let list = list_ptr as HWND;
                unsafe {
                    let mut wide: Vec<u16> = line.encode_utf16().collect();
                    wide.push(0);
                    let count = SendMessageW(list, LB_ADDSTRING, 0, wide.as_ptr() as isize);
                    if count > 10000 {
                        SendMessageW(list, LB_DELETESTRING, 0, 0);
                    }
                    let top = SendMessageW(list, LB_GETCOUNT, 0, 0);
                    if top > 0 {
                        SendMessageW(list, LB_SETTOPINDEX, (top - 1) as usize, 0);
                    }
                }
            }
        });
    }

    fn format_entry(entry: &LogEntry) -> String {
        let timestamp = chrono::DateTime::from_timestamp_millis(entry.timestamp as i64)
            .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
            .unwrap_or_else(|| "unknown".to_string());
        format!("[{}] [{}] [{}] {}", timestamp, entry.level, entry.plugin, entry.message)
    }
}

unsafe extern "system" fn log_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: usize,
    lparam: isize,
) -> isize {
    if msg == WM_CLOSE {
        ShowWindow(hwnd, SW_HIDE);
        return 0;
    }
    if msg == WM_SIZE {
        let width = (lparam as u32) & 0xFFFF;
        let height = ((lparam as u32) >> 16) as i32;
        let list = FindWindowExW(hwnd, std::ptr::null_mut(),
            "LISTBOX\0".encode_utf16().collect::<Vec<u16>>().as_ptr(), std::ptr::null());
        if !list.is_null() {
            SetWindowPos(list, std::ptr::null_mut(), 10, 10, (width as i32) - 20, height - 60, SWP_NOZORDER as u32);
        }
        let btn_y = height - 40;
        for (id, x) in [(ID_CLEAR_BTN, 10), (ID_PIN_BTN, 110)] {
            let btn = GetDlgItem(hwnd, id as i32);
            if !btn.is_null() {
                SetWindowPos(btn, std::ptr::null_mut(), x, btn_y, 90, 25, SWP_NOZORDER as u32);
            }
        }
        return 0;
    }
    if msg == WM_COMMAND {
        let id = wparam as usize;
        if id == ID_CLEAR_BTN {
            let list = FindWindowExW(hwnd, std::ptr::null_mut(),
                "LISTBOX\0".encode_utf16().collect::<Vec<u16>>().as_ptr(), std::ptr::null());
            if !list.is_null() {
                SendMessageW(list, LB_RESETCONTENT, 0, 0);
            }
            return 0;
        }
        if id == ID_PIN_BTN {
            let checked = SendMessageW(GetDlgItem(hwnd, id as i32), BM_GETCHECK, 0, 0);
            let z = if checked != 0 { -1isize } else { -2isize };
            SetWindowPos(hwnd, z as *mut c_void, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE);
            return 0;
        }
    }
    if let Some(orig) = ORIG_WNDPROC {
        orig(hwnd, msg, wparam, lparam)
    } else {
        DefWindowProcW(hwnd, msg, wparam, lparam)
    }
}
