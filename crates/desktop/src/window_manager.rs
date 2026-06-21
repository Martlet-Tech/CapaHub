use core::render_intent::*;
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleA;
use std::sync::{Arc, Mutex};

const GWLP_USERDATA: i32 = -21;
const VK_ESCAPE: usize = 0x1B;
const VK_UP: usize = 0x26;
const VK_DOWN: usize = 0x28;
const VK_RETURN: usize = 0x0D;

static PENDING_HIT: Mutex<Option<(Arc<dyn Fn(u64) + Send + Sync + 'static>, u64)>> = Mutex::new(None);

struct WindowState {
    draws: Vec<DrawCmd>,
    hit_areas: Vec<DrawHitArea>,
    selected_idx: usize,
    hovered_idx: Option<usize>,
    created_at: u64,
    on_hit: Option<Arc<dyn Fn(u64) + Send + Sync + 'static>>,
    on_delete: Option<Arc<dyn Fn(u64) + Send + Sync + 'static>>,
}

pub fn spawn_window(config: WindowConfig) {
    let w = config.width;
    let h = config.height;
    let pos = config.position;
    let draws = config.draws;
    let selected_index = config.selected_index;
    let on_hit = config.on_hit;
    let on_delete = config.on_delete;
    std::thread::spawn(move || {
        run_window(w, h, pos, draws, selected_index, on_hit, on_delete);
    });
}

fn run_window(
    width: u32, height: u32, position: WindowPosition, draws: Vec<DrawCmd>,
    selected_index: usize,
    on_hit: Option<Arc<dyn Fn(u64) + Send + Sync + 'static>>,
    on_delete: Option<Arc<dyn Fn(u64) + Send + Sync + 'static>>,
) {
    let hinstance = unsafe { GetModuleHandleA(std::ptr::null()) };
    let (x, y) = compute_position(width, height, &position);

    let hit_areas: Vec<DrawHitArea> = draws.iter().filter_map(|cmd| {
        if let DrawCmd::HitArea(ha) = cmd { Some(ha.clone()) } else { None }
    }).collect();

    let class_name: Vec<u16> = "CapaPopup\0".encode_utf16().collect();
    let wc = WNDCLASSW {
        style: 0,
        lpfnWndProc: Some(popup_wndproc),
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: hinstance,
        hIcon: std::ptr::null_mut(),
        hCursor: unsafe { LoadCursorW(std::ptr::null_mut(), IDC_ARROW) },
        hbrBackground: unsafe { GetStockObject(WHITE_BRUSH) },
        lpszMenuName: std::ptr::null_mut(),
        lpszClassName: class_name.as_ptr() as *mut u16,
    };

    unsafe { RegisterClassW(&wc); }

    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
            class_name.as_ptr(),
            std::ptr::null(),
            WS_POPUP | WS_VISIBLE | WS_BORDER,
            x, y, width as i32, height as i32,
            std::ptr::null_mut(), std::ptr::null_mut(), hinstance, std::ptr::null(),
        )
    };

    if hwnd.is_null() { return; }

    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as u64;
    let idx = selected_index.min(hit_areas.len().saturating_sub(1));
    let state = Box::into_raw(Box::new(WindowState {
        draws,
        hit_areas,
        selected_idx: idx,
        hovered_idx: None,
        created_at: now,
        on_hit,
        on_delete,
    }));

    unsafe {
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, state as isize);
        ShowWindow(hwnd, SW_SHOW);
        SetForegroundWindow(hwnd);
        InvalidateRect(hwnd, std::ptr::null(), 1);
        UpdateWindow(hwnd);
    }

    let mut msg = unsafe { std::mem::zeroed::<MSG>() };
    loop {
        let result = unsafe { GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) };
        if result == 0 || result == -1 { break; }
        unsafe { TranslateMessage(&msg); DispatchMessageW(&msg); }
    }

    // Deferred action: popup is closed, focus restored to target window
    if let Some((cb, id)) = PENDING_HIT.lock().unwrap().take() {
        cb(id);
    }
}

fn compute_position(width: u32, height: u32, position: &WindowPosition) -> (i32, i32) {
    let sw = unsafe { 
        extern "system" { fn GetSystemMetrics(nIndex: i32) -> i32; }
        GetSystemMetrics(0) // SM_CXSCREEN
    };
    let sh = unsafe {
        extern "system" { fn GetSystemMetrics(nIndex: i32) -> i32; }
        GetSystemMetrics(1) // SM_CYSCREEN
    };
    match position {
        WindowPosition::NearCursor => {
            let mut pt = unsafe { std::mem::zeroed::<POINT>() };
            unsafe { GetCursorPos(&mut pt); }
            (pt.x.min(sw - width as i32 - 20).max(0),
             if pt.y + height as i32 + 20 > sh { (pt.y - height as i32 - 10).max(0) } else { pt.y + 20 })
        }
        WindowPosition::ScreenCenter => ((sw - width as i32) / 2, (sh - height as i32) / 2),
        WindowPosition::FollowFocus => (200, 200),
        WindowPosition::At { x, y } => (*x, *y),
    }
}

unsafe fn state_mut(ptr: isize) -> &'static mut WindowState {
    &mut *(ptr as *mut WindowState)
}

unsafe fn state_ref(ptr: isize) -> &'static WindowState {
    &*(ptr as *const WindowState)
}

fn hit_test(state: &WindowState, x: i32, y: i32) -> Option<usize> {
    state.hit_areas.iter().position(|ha| {
        x >= ha.x && x < ha.x + ha.width as i32
            && y >= ha.y && y < ha.y + ha.height as i32
    })
}

unsafe extern "system" fn popup_wndproc(
    hwnd: HWND, msg: u32, wparam: usize, lparam: isize,
) -> isize {
    match msg {
        WM_CLOSE => { DestroyWindow(hwnd); 0 }
        WM_ACTIVATE if wparam == 0 => {
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA);
            if ptr != 0 {
                let s = state_ref(ptr);
                let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as u64;
                // Only auto-close if has hit areas (interactive picker), not for passive display popups
                if now - s.created_at > 500 && !s.hit_areas.is_empty() { DestroyWindow(hwnd); }
            }
            0
        }
        WM_NCDESTROY => {
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA);
            if ptr != 0 { drop(Box::from_raw(ptr as *mut WindowState)); }
            0
        }
        WM_ERASEBKGND => 1,
        WM_MOUSEMOVE => {
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA);
            if ptr != 0 {
                let x = (lparam & 0xFFFF) as i32;
                let y = ((lparam >> 16) & 0xFFFF) as i32;
                let s = state_mut(ptr);
                let new_hover = hit_test(s, x, y);
                if new_hover != s.hovered_idx {
                    s.hovered_idx = new_hover;
                    InvalidateRect(hwnd, std::ptr::null(), 1);
                }
            }
            0
        }
        WM_LBUTTONDOWN => {
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA);
            if ptr != 0 {
                let s = state_ref(ptr);
                let idx = s.hovered_idx.unwrap_or(s.selected_idx);
                if let Some(ref cb) = s.on_hit {
                    if let Some(ha) = s.hit_areas.get(idx) {
                        *PENDING_HIT.lock().unwrap() = Some((cb.clone(), ha.id));
                    }
                }
            }
            DestroyWindow(hwnd);
            PostQuitMessage(0);
            0
        }
        WM_KEYDOWN => {
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA);
            match wparam {
                VK_ESCAPE => { DestroyWindow(hwnd); }
                VK_RETURN => {
                    if ptr != 0 {
                        let s = state_ref(ptr);
                        let idx = s.hovered_idx.unwrap_or(s.selected_idx);
                        if let Some(ref cb) = s.on_hit {
                            if let Some(ha) = s.hit_areas.get(idx) {
                                *PENDING_HIT.lock().unwrap() = Some((cb.clone(), ha.id));
                            }
                        }
                    }
                    DestroyWindow(hwnd);
                    PostQuitMessage(0);
                }
                0x2E if ptr != 0 => {
                    let s = state_ref(ptr);
                    let idx = s.hovered_idx.unwrap_or(s.selected_idx);
                    if let Some(ref cb) = s.on_delete {
                        if let Some(ha) = s.hit_areas.get(idx) {
                            cb(ha.id);
                        }
                    }
                    DestroyWindow(hwnd);
                    PostQuitMessage(0);
                }
                VK_UP | VK_DOWN if ptr != 0 => {
                    let s = state_mut(ptr);
                    let len = s.hit_areas.len();
                    if len > 1 {
                        if wparam == VK_UP && s.selected_idx > 0 {
                            s.selected_idx -= 1;
                        } else if wparam == VK_DOWN && s.selected_idx + 1 < len {
                            s.selected_idx += 1;
                        }
                        InvalidateRect(hwnd, std::ptr::null(), 1);
                    }
                }
                _ => { return DefWindowProcW(hwnd, msg, wparam, lparam); }
            }
            0
        }
        WM_PAINT => {
            let mut ps = std::mem::zeroed::<PAINTSTRUCT>();
            let hdc = BeginPaint(hwnd, &mut ps);
            let mut rect = RECT { left: 0, top: 0, right: 0, bottom: 0 };
            GetClientRect(hwnd, &mut rect);
            let white_brush = GetStockObject(WHITE_BRUSH);
            FillRect(hdc, &rect, white_brush);

            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA);
            if ptr != 0 {
                let s = state_ref(ptr);
                let active_idx = s.hovered_idx.unwrap_or(s.selected_idx);

                // Draw highlight behind the active item
                if let Some(ha) = s.hit_areas.get(active_idx) {
                    let highlight_brush = GetStockObject(LTGRAY_BRUSH);
                    let hr = RECT {
                        left: ha.x, top: ha.y,
                        right: ha.x + ha.width as i32,
                        bottom: ha.y + ha.height as i32,
                    };
                    FillRect(hdc, &hr, highlight_brush);
                }

                for cmd in &s.draws {
                    match cmd {
                        DrawCmd::Text(t) => {
                            let hfont = CreateFontW(-(t.font_size as i32), 0,0,0,400,0,0,0,1,0,0,0,0,
                                "Microsoft YaHei\0".encode_utf16().collect::<Vec<u16>>().as_ptr());
                            if !hfont.is_null() { SelectObject(hdc, hfont); }
                            SetBkMode(hdc, TRANSPARENT as i32);
                            SetTextColor(hdc, t.color);
                            let w: Vec<u16> = t.text.encode_utf16().chain(Some(0)).collect();
                            TextOutW(hdc, t.x, t.y, w.as_ptr(), (w.len()-1) as i32);
                        }
                        DrawCmd::Separator(s) => {
                            let hpen = CreatePen(0, 1, s.color);
                            if !hpen.is_null() { SelectObject(hdc, hpen); }
                            MoveToEx(hdc, s.x, s.y, std::ptr::null_mut());
                            LineTo(hdc, s.x + s.width as i32, s.y);
                            if !hpen.is_null() { DeleteObject(hpen); }
                        }
                        _ => {}
                    }
                }
            }
            EndPaint(hwnd, &mut ps);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}
