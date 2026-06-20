use core::plugin_manager::{PluginManager, PluginState};
use std::ffi::c_void;
use std::sync::Arc;
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleA;
use crate::icon_loader;

const WM_SETICON: u32 = 0x0080;
const ICON_SMALL: usize = 0;
const ICON_BIG: usize = 1;

#[link(name = "gdi32")]
extern "system" {
    fn CreateFontW(
        cHeight: i32, cWidth: i32, cEscapement: i32, cOrientation: i32,
        cWeight: i32, bItalic: i32, bUnderline: i32, bStrikeOut: i32,
        iCharSet: i32, iOutPrecision: i32, iClipPrecision: i32,
        iQuality: i32, iPitchAndFamily: i32, pszFaceName: *const u16,
    ) -> isize;
}

const WM_SETFONT: u32 = 0x0030;
const DEFAULT_CHARSET: i32 = 1;
const OUT_DEFAULT_PRECIS: i32 = 0;
const CLIP_DEFAULT_PRECIS: i32 = 0;
const DEFAULT_QUALITY: i32 = 0;
const DEFAULT_PITCH: i32 = 0;
const FF_DONTCARE: i32 = 0;

const ID_LIST: usize = 3001;
const ID_BTN_ENABLE: usize = 3010;
const ID_BTN_DISABLE: usize = 3011;
const ID_BTN_DELETE: usize = 3012;
const ID_BTN_INSTALL: usize = 3013;
const WM_REFRESH_LIST: u32 = WM_APP + 10;
const GWLP_WNDPROC: i32 = -4;
const LBS_NOTIFY: u32 = 0x0001;

#[repr(C)]
pub struct OPENFILENAMEW {
    pub lStructSize: u32,
    pub hwndOwner: HWND,
    pub hInstance: HINSTANCE,
    pub lpstrFilter: *const u16,
    pub lpstrCustomFilter: *mut u16,
    pub nMaxCustFilter: u32,
    pub nFilterIndex: u32,
    pub lpstrFile: *mut u16,
    pub nMaxFile: u32,
    pub lpstrFileTitle: *mut u16,
    pub nMaxFileTitle: u32,
    pub lpstrInitialDir: *const u16,
    pub lpstrTitle: *const u16,
    pub Flags: u32,
    pub nFileOffset: u16,
    pub nFileExtension: u16,
    pub lpstrDefExt: *const u16,
    pub lCustData: isize,
    pub lpfnHook: Option<unsafe extern "system" fn(HWND, u32, usize, isize) -> usize>,
    pub lpTemplateName: *const u16,
    pub pvReserved: *mut c_void,
    pub dwReserved: u32,
    pub FlagsEx: u32,
}

#[link(name = "comdlg32")]
extern "system" {
    fn GetOpenFileNameW(param: *mut OPENFILENAMEW) -> i32;
}

static mut PM: Option<Arc<PluginManager>> = None;
type WndProc = unsafe extern "system" fn(HWND, u32, usize, isize) -> isize;
static mut ORIG_WNDPROC: Option<WndProc> = None;

pub fn set_pm(pm: Arc<PluginManager>) {
    unsafe { PM = Some(pm); }
}

pub fn open_manager_window() {
    unsafe {
        let hwnd = FindWindowW(
            "CapaHubMgrClass\0".encode_utf16().collect::<Vec<u16>>().as_ptr(),
            std::ptr::null(),
        );
        if !hwnd.is_null() {
            ShowWindow(hwnd, SW_SHOW);
            SetForegroundWindow(hwnd);
            SendMessageW(hwnd, WM_REFRESH_LIST, 0, 0);
            return;
        }
    }
    create_window();
}

fn create_window() {
    let hinstance = unsafe { GetModuleHandleA(std::ptr::null()) };

    let hwnd = unsafe {
        CreateWindowExW(
            0,
            "#32770\0".encode_utf16().collect::<Vec<u16>>().as_ptr(),
            "CapaHub - Plugin Manager\0".encode_utf16().collect::<Vec<u16>>().as_ptr(),
            WS_OVERLAPPEDWINDOW | WS_VISIBLE,
            CW_USEDEFAULT, CW_USEDEFAULT, 480, 400,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            hinstance,
            std::ptr::null(),
        )
    };

    let icon = icon_loader::get_tray_icon();
    unsafe {
        SendMessageW(hwnd, WM_SETICON, ICON_SMALL, icon as isize);
        SendMessageW(hwnd, WM_SETICON, ICON_BIG, icon as isize);

        let old = SetWindowLongPtrW(hwnd, GWLP_WNDPROC, mgr_wndproc as *const () as usize as isize);
        ORIG_WNDPROC = Some(std::mem::transmute::<isize, WndProc>(old));
    }

    let list_hwnd = unsafe {
        CreateWindowExW(
            0,
            "LISTBOX\0".encode_utf16().collect::<Vec<u16>>().as_ptr(),
            std::ptr::null(),
            WS_CHILD | WS_VISIBLE | WS_BORDER | WS_VSCROLL | LBS_NOTIFY,
            10, 10, 440, 270,
            hwnd,
            ID_LIST as HMENU,
            hinstance,
            std::ptr::null(),
        )
    };

    if !list_hwnd.is_null() {
        let face: Vec<u16> = "Microsoft YaHei\0".encode_utf16().collect();
        let hfont = unsafe { CreateFontW(-16, 0, 0, 0, 400, 0, 0, 0, DEFAULT_CHARSET, OUT_DEFAULT_PRECIS, CLIP_DEFAULT_PRECIS, DEFAULT_QUALITY, DEFAULT_PITCH | FF_DONTCARE, face.as_ptr()) };
        if hfont != 0 {
            unsafe { SendMessageW(list_hwnd, WM_SETFONT, hfont as usize, 1); }
        }
    }

    unsafe {
        for (id, text, x) in &[
            (ID_BTN_ENABLE, "Enable\0", 10),
            (ID_BTN_DISABLE, "Disable\0", 90),
            (ID_BTN_DELETE, "Delete\0", 170),
            (ID_BTN_INSTALL, "Install DAP\0", 260),
        ] {
            CreateWindowExW(
                0,
                "BUTTON\0".encode_utf16().collect::<Vec<u16>>().as_ptr(),
                text.encode_utf16().collect::<Vec<u16>>().as_ptr(),
                WS_CHILD | WS_VISIBLE,
                *x, 290, 90, 28,
                hwnd,
                *id as HMENU,
                hinstance,
                std::ptr::null(),
            );
        }
    }

    refresh_list(hwnd);
}

fn refresh_list(hwnd: HWND) {
    let list = unsafe { GetDlgItem(hwnd, ID_LIST as i32) };
    if list.is_null() {
        return;
    }
    unsafe { SendMessageW(list, LB_RESETCONTENT, 0, 0); }

    let pm = unsafe { std::ptr::addr_of!(PM).as_ref().unwrap().as_ref().unwrap() };
    for info in pm.plugin_list() {
        let state_str = match info.state {
            PluginState::Enabled => "Enabled",
            PluginState::Disabled => "Disabled",
            PluginState::Loaded => "Loaded",
            PluginState::Unloaded => "Unloaded",
        };
        let line = format!("{} v{} [{}]", info.name, info.version, state_str);
        let mut wide: Vec<u16> = line.encode_utf16().collect();
        wide.push(0);
        unsafe {
            SendMessageW(list, LB_ADDSTRING, 0, wide.as_ptr() as isize);
        }
    }
}

unsafe extern "system" fn mgr_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: usize,
    lparam: isize,
) -> isize {
    if msg == WM_CLOSE {
        ShowWindow(hwnd, SW_HIDE);
        return 0;
    }

    if msg == WM_REFRESH_LIST {
        refresh_list(hwnd);
        return 0;
    }

    if msg == WM_COMMAND {
        let id = wparam as usize;
        match id {
            ID_BTN_ENABLE | ID_BTN_DISABLE | ID_BTN_DELETE => {
                let list = GetDlgItem(hwnd, ID_LIST as i32);
                let sel = SendMessageW(list, LB_GETCURSEL, 0, 0) as i32;
                if sel < 0 {
                    return 0;
                }
                let mut buf = [0u16; 256];
                let len = SendMessageW(list, LB_GETTEXT, sel as usize, buf.as_mut_ptr() as isize);
                if len == 0 {
                    return 0;
                }
                let name_end = buf.iter().position(|&c| c == ' ' as u16).unwrap_or(0);
                let name = String::from_utf16_lossy(&buf[..name_end]);

                let pm = unsafe { std::ptr::addr_of!(PM).as_ref().unwrap().as_ref().unwrap() };
                match id {
                    ID_BTN_ENABLE => { let _ = pm.enable_plugin(&name); }
                    ID_BTN_DISABLE => { let _ = pm.disable_plugin(&name); }
                    ID_BTN_DELETE => { let _ = pm.delete_plugin(&name); }
                    _ => {}
                }
                refresh_list(hwnd);
                return 0;
            }
            ID_BTN_INSTALL => {
                install_dialog(hwnd);
                return 0;
            }
            _ => {}
        }
    }

    if let Some(orig) = ORIG_WNDPROC {
        orig(hwnd, msg, wparam, lparam)
    } else {
        DefWindowProcW(hwnd, msg, wparam, lparam)
    }
}

fn install_dialog(parent: HWND) {
    let mut buf = [0u16; 1024];
    let filter: Vec<u16> = "DAP Files (*.dap)\0*.dap\0All Files (*.*)\0*.*\0\0"
        .encode_utf16()
        .collect();
    let def_ext: Vec<u16> = "dap\0".encode_utf16().collect();

    let mut ofn = OPENFILENAMEW {
        lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
        hwndOwner: parent,
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
        Flags: 0x1000 | 0x4,
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

    let result = unsafe { GetOpenFileNameW(&mut ofn) };
    if result == 0 {
        return;
    }

    let path_len = buf.iter().position(|&c| c == 0).unwrap_or(0);
    let path = String::from_utf16_lossy(&buf[..path_len]);

    let pm = unsafe { std::ptr::addr_of!(PM).as_ref().unwrap().clone().unwrap() };
    match pm.install_from_path(std::path::Path::new(&path)) {
        Ok(()) => {
            refresh_list(parent);
            pm.enable_all();
            refresh_list(parent);
        }
            Err(e) => {
                let mut msg: Vec<u16> = format!("Install failed:\n{}", e).encode_utf16().collect();
                msg.push(0);
                unsafe {
                    MessageBoxW(
                        parent,
                        msg.as_ptr(),
                        "Error\0".encode_utf16().collect::<Vec<u16>>().as_ptr(),
                        MB_OK,
                    );
                }
            }
    }
}
