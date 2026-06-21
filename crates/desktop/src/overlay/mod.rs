use std::cell::Cell;
use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::Mutex;
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleA;

extern "system" { fn GetSystemMetrics(nIndex: i32) -> i32; }
const SM_XVIRTUALSCREEN: i32 = 76;
const SM_YVIRTUALSCREEN: i32 = 77;
const SM_CXVIRTUALSCREEN: i32 = 78;
const SM_CYVIRTUALSCREEN: i32 = 79;

struct Hwnd(HWND); unsafe impl Send for Hwnd {} unsafe impl Sync for Hwnd {}
struct Hdc(HDC);   unsafe impl Send for Hdc {}   unsafe impl Sync for Hdc {}
struct Hbmp(HBITMAP); unsafe impl Send for Hbmp {} unsafe impl Sync for Hbmp {}

static MANAGER: Mutex<Option<OverlayManager>> = Mutex::new(None);

struct OverlayManager { next_id: u32, windows: HashMap<u32, OverlayWindow> }

struct OverlayWindow {
    hwnd: Hwnd, w: i32, h: i32,
    hdc: Hdc, bitmap: Hbmp,
    cached: Option<Hbmp>,
    opaque: Cell<bool>,
}

pub fn init() { *MANAGER.lock().unwrap() = Some(OverlayManager { next_id: 1, windows: HashMap::new() }); }

pub fn handle_cmd(json: &str) -> Result<u32, String> {
    let v: serde_json::Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
    let cmd = v["cmd"].as_str().unwrap_or("");
    let mut mgr = MANAGER.lock().unwrap();
    let mgr = mgr.as_mut().ok_or("not initialized")?;

    match cmd {
        "create" => {
            let x = v["x"].as_i64().unwrap_or(0) as i32;
            let y = v["y"].as_i64().unwrap_or(0) as i32;
            let mut w = v["w"].as_i64().unwrap_or(0) as i32;
            let mut h = v["h"].as_i64().unwrap_or(0) as i32;
            if w == 0 { unsafe { w = GetSystemMetrics(SM_CXVIRTUALSCREEN); } }
            if h == 0 { unsafe { h = GetSystemMetrics(SM_CYVIRTUALSCREEN); } }
            let x = unsafe { GetSystemMetrics(SM_XVIRTUALSCREEN) + x };
            let y = unsafe { GetSystemMetrics(SM_YVIRTUALSCREEN) + y };
            let id = mgr.next_id; mgr.next_id += 1;
            let (hwnd, hdc, bitmap) = create_overlay(x, y, w, h)?;
            mgr.windows.insert(id, OverlayWindow { hwnd, w, h, hdc, bitmap, cached: None, opaque: Cell::new(false) });
            Ok(id)
        }
        "freeze" => { let h = v["h"].as_u64().unwrap_or(0) as u32; freeze_screen(mgr.windows.get(&h).ok_or("invalid handle")?) }
        "draw_rect" => {
            let h = v["h"].as_u64().unwrap_or(0) as u32;
            let win = mgr.windows.get(&h).ok_or("invalid handle")?;
            draw_rect_on(win, v["x"].as_i64().unwrap_or(0) as i32, v["y"].as_i64().unwrap_or(0) as i32,
                         v["rw"].as_i64().unwrap_or(0) as i32, v["rh"].as_i64().unwrap_or(0) as i32,
                         v["color"].as_u64().unwrap_or(0xFF0000) as u32, v["thickness"].as_u64().unwrap_or(2) as i32)?;
            Ok(h)
        }
        "draw_line" => { let h = v["h"].as_u64().unwrap_or(0) as u32; let win = mgr.windows.get(&h).ok_or("invalid handle")?; draw_line_on(win, v["x1"].as_i64().unwrap_or(0) as i32, v["y1"].as_i64().unwrap_or(0) as i32, v["x2"].as_i64().unwrap_or(0) as i32, v["y2"].as_i64().unwrap_or(0) as i32, v["color"].as_u64().unwrap_or(0xFF0000) as u32, v["thickness"].as_u64().unwrap_or(2) as i32)?; Ok(h) }
        "clear" => { let h = v["h"].as_u64().unwrap_or(0) as u32; clear_window(mgr.windows.get(&h).ok_or("invalid handle")?)?; Ok(h) }
        "destroy" => { let h = v["h"].as_u64().unwrap_or(0) as u32; if let Some(w) = mgr.windows.remove(&h) { destroy_overlay(w); } Ok(h) }
        _ => Err(format!("unknown cmd: {}", cmd)),
    }
}

fn create_overlay(x: i32, y: i32, w: i32, h: i32) -> Result<(Hwnd, Hdc, Hbmp), String> {
    let hinst = unsafe { GetModuleHandleA(std::ptr::null()) };
    let class: Vec<u16> = "CapaOverlay\0".encode_utf16().collect();
    unsafe { RegisterClassW(&WNDCLASSW { style:0, lpfnWndProc:Some(DefWindowProcW), cbClsExtra:0, cbWndExtra:0, hInstance:hinst, hIcon:std::ptr::null_mut(), hCursor:std::ptr::null_mut(), hbrBackground:std::ptr::null_mut(), lpszMenuName:std::ptr::null_mut(), lpszClassName:class.as_ptr() as *mut u16 }); }
    let hwnd = unsafe { CreateWindowExW(WS_EX_LAYERED|WS_EX_TRANSPARENT|WS_EX_TOPMOST|WS_EX_TOOLWINDOW, class.as_ptr(), std::ptr::null(), WS_POPUP, x, y, w, h, std::ptr::null_mut(), std::ptr::null_mut(), hinst, std::ptr::null()) };
    if hwnd.is_null() { return Err("CreateWindowEx failed".into()); }
    unsafe { ShowWindow(hwnd, SW_SHOWNOACTIVATE); }
    let (hdc, bitmap) = unsafe {
        let screen = GetDC(std::ptr::null_mut());
        let hdc = CreateCompatibleDC(screen); ReleaseDC(std::ptr::null_mut(), screen);
        if hdc.is_null() { DestroyWindow(hwnd); return Err("CreateCompatibleDC failed".into()); }
        let bmp = CreateCompatibleBitmap(GetDC(std::ptr::null_mut()), w, h);
        if bmp.is_null() { DeleteDC(hdc); DestroyWindow(hwnd); return Err("CreateCompatibleBitmap failed".into()); }
        SelectObject(hdc, bmp);
        let brush = CreateSolidBrush(0x00000000); FillRect(hdc, &RECT{left:0,top:0,right:w,bottom:h}, brush); DeleteObject(brush);
        (hdc, bmp)
    };
    Ok((Hwnd(hwnd), Hdc(hdc), Hbmp(bitmap)))
}

fn freeze_screen(win: &OverlayWindow) -> Result<u32, String> {
    unsafe {
        let screen_dc = GetDC(std::ptr::null_mut());
        let x = GetSystemMetrics(SM_XVIRTUALSCREEN); let y = GetSystemMetrics(SM_YVIRTUALSCREEN);
        BitBlt(win.hdc.0, 0, 0, win.w, win.h, screen_dc, x, y, SRCCOPY);
        ReleaseDC(std::ptr::null_mut(), screen_dc);
    }
    win.opaque.set(true);
    flush(win)?;
    Ok(0)
}

fn xor_rect(win: &OverlayWindow, x: i32, y: i32, w: i32, h: i32) -> Result<u32, String> {
    unsafe {
        let old_rop = SetROP2(win.hdc.0, R2_NOTXORPEN);
        let pen = CreatePen(PS_SOLID, 3, 0x00FFFFFF);
        let old_pen = SelectObject(win.hdc.0, pen);
        let old_brush = SelectObject(win.hdc.0, GetStockObject(NULL_BRUSH as i32));
        Rectangle(win.hdc.0, x, y, x + w, y + h);
        SelectObject(win.hdc.0, old_brush);
        SelectObject(win.hdc.0, old_pen);
        DeleteObject(pen);
        SetROP2(win.hdc.0, old_rop);
    }
    flush(win)?;
    Ok(0)
}

fn draw_rect_on(win: &OverlayWindow, x: i32, y: i32, w: i32, h: i32, color: u32, thickness: i32) -> Result<(), String> {
    let bgr = ((color & 0xFF) << 16) | (color & 0xFF00) | ((color >> 16) & 0xFF);
    unsafe {
        let pen = CreatePen(PS_SOLID, thickness, bgr);
        let old_pen = SelectObject(win.hdc.0, pen);
        let old_brush = SelectObject(win.hdc.0, GetStockObject(NULL_BRUSH as i32));
        Rectangle(win.hdc.0, x, y, x + w, y + h);
        SelectObject(win.hdc.0, old_brush);
        SelectObject(win.hdc.0, old_pen);
        DeleteObject(pen);
    }
    flush(win)
}

fn draw_line_on(win: &OverlayWindow, x1: i32, y1: i32, x2: i32, y2: i32, color: u32, thickness: i32) -> Result<(), String> {
    let bgr = ((color&0xFF)<<16)|(color&0xFF00)|((color>>16)&0xFF);
    unsafe { let pen=CreatePen(PS_SOLID,thickness,bgr); let old=SelectObject(win.hdc.0,pen); MoveToEx(win.hdc.0,x1,y1,std::ptr::null_mut()); LineTo(win.hdc.0,x2,y2); SelectObject(win.hdc.0,old); DeleteObject(pen); }
    flush(win)
}

fn clear_window(win: &OverlayWindow) -> Result<(), String> {
    unsafe { let brush=CreateSolidBrush(0x00000000); FillRect(win.hdc.0, &RECT{left:0,top:0,right:win.w,bottom:win.h}, brush); DeleteObject(brush); }
    flush(win)
}

fn flush(win: &OverlayWindow) -> Result<(), String> {
    unsafe {
        let screen = GetDC(std::ptr::null_mut());
        let blend = if win.opaque.get() { BLENDFUNCTION{BlendOp:0,BlendFlags:0,SourceConstantAlpha:255,AlphaFormat:0} } else { BLENDFUNCTION{BlendOp:0,BlendFlags:0,SourceConstantAlpha:255,AlphaFormat:1} };
        let sz = SIZE{cx:win.w,cy:win.h}; let pt=POINT{x:0,y:0};
        let ok = UpdateLayeredWindow(win.hwnd.0, screen, &pt, &sz, win.hdc.0, &pt, 0, &blend, ULW_ALPHA);
        ReleaseDC(std::ptr::null_mut(), screen);
        if ok == 0 { return Err("UpdateLayeredWindow failed".into()); }
    }
    Ok(())
}

fn destroy_overlay(win: OverlayWindow) {
    unsafe { if let Some(c) = win.cached { DeleteObject(c.0); } DeleteDC(win.hdc.0); DeleteObject(win.bitmap.0); DestroyWindow(win.hwnd.0); }
}
