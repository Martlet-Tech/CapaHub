use core::logger::LogEntry;
use crossbeam_channel::Receiver;
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleA;

const ES_MULTILINE: u32 = 0x0004;
const ES_READONLY: u32 = 0x0800;
const ES_AUTOVSCROLL: u32 = 0x0040;
const EM_SETSEL: u32 = 0x00B1;
const EM_REPLACESEL: u32 = 0x00C2;
const EM_SCROLLCARET: u32 = 0x00B7;
const GWLP_WNDPROC: i32 = -4;

type WndProc = unsafe extern "system" fn(HWND, u32, usize, isize) -> isize;

static mut ORIG_WNDPROC: Option<WndProc> = None;

pub struct LogWindow {
    pub hwnd: HWND,
}

impl LogWindow {
    pub fn new(rx: Receiver<LogEntry>, _config: &core::config::Config) -> Self {
        let hinstance = unsafe { GetModuleHandleA(std::ptr::null()) };

        let hwnd = unsafe {
            CreateWindowExW(
                0,
                "EDIT\0".encode_utf16().collect::<Vec<u16>>().as_ptr(),
                "CapaHub - Log\0".encode_utf16().collect::<Vec<u16>>().as_ptr(),
                WS_OVERLAPPEDWINDOW | WS_VISIBLE | ES_MULTILINE | ES_READONLY | ES_AUTOVSCROLL | WS_VSCROLL,
                CW_USEDEFAULT, CW_USEDEFAULT, 700, 500,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                hinstance,
                std::ptr::null(),
            )
        };

        unsafe {
            let old = SetWindowLongPtrW(hwnd, GWLP_WNDPROC, log_wndproc as *const () as usize as isize);
            ORIG_WNDPROC = Some(std::mem::transmute::<isize, WndProc>(old));
        }

        let edit_ptr = hwnd as usize;
        std::thread::spawn(move || {
            for entry in rx {
                let timestamp = chrono::DateTime::from_timestamp_millis(entry.timestamp as i64)
                    .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                    .unwrap_or_else(|| "unknown".to_string());

                let line = format!(
                    "[{}] [{}] [{}] {}\r\n",
                    timestamp, entry.level, entry.plugin, entry.message
                );

                let edit = edit_ptr as HWND;
                unsafe {
                    let len = GetWindowTextLengthW(edit);
                    SendMessageW(edit, EM_SETSEL, len as usize, len as isize);
                    let wide: Vec<u16> = line.encode_utf16().collect();
                    SendMessageW(edit, EM_REPLACESEL, 0, wide.as_ptr() as isize);
                    SendMessageW(edit, EM_SCROLLCARET, 0, 0);
                }
            }
        });

        LogWindow { hwnd }
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
    if let Some(orig) = ORIG_WNDPROC {
        orig(hwnd, msg, wparam, lparam)
    } else {
        DefWindowProcW(hwnd, msg, wparam, lparam)
    }
}
