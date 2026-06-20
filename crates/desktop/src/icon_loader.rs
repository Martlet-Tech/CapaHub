use std::ffi::c_void;
use std::sync::atomic::{AtomicUsize, Ordering};

#[link(name = "user32")]
extern "system" {
    fn LoadImageW(
        hinst: *mut c_void,
        lpszName: *const u16,
        uType: u32,
        cxDesired: i32,
        cyDesired: i32,
        fuLoad: u32,
    ) -> *mut c_void;
}

const IMAGE_ICON: u32 = 1;
const LR_LOADFROMFILE: u32 = 0x00000010;
const LR_DEFAULTSIZE: u32 = 0x00000040;

static TRAY_ICON_PNG: &[u8] = include_bytes!("../../../resources/tray-icon.png");
static CACHED_ICON: AtomicUsize = AtomicUsize::new(0);

pub fn get_tray_icon() -> *mut c_void {
    let cached = CACHED_ICON.load(Ordering::Relaxed);
    if cached != 0 {
        return cached as *mut c_void;
    }
    let hicon = load_icon_from_png_bytes(TRAY_ICON_PNG);
    if let Some(ptr) = hicon {
        CACHED_ICON.store(ptr as usize, Ordering::Relaxed);
        ptr
    } else {
        std::ptr::null_mut()
    }
}

pub fn load_icon_from_png_bytes(png_data: &[u8]) -> Option<*mut c_void> {
    if png_data.len() > 0xffff_ffff {
        return None;
    }
    let png_size = png_data.len() as u32;
    let ico_data = build_ico_with_png(png_data, png_size);

    let temp_dir = std::env::temp_dir().join("CapaHub");
    let _ = std::fs::create_dir_all(&temp_dir);
    let ico_path = temp_dir.join("tray-icon.ico");
    std::fs::write(&ico_path, &ico_data).ok()?;

    let wide: Vec<u16> = ico_path.to_string_lossy().encode_utf16().chain(Some(0)).collect();
    let hicon = unsafe {
        LoadImageW(
            std::ptr::null_mut(),
            wide.as_ptr(),
            IMAGE_ICON,
            0, 0,
            LR_LOADFROMFILE | LR_DEFAULTSIZE,
        )
    };
    if hicon.is_null() { None } else { Some(hicon) }
}

fn build_ico_with_png(png_data: &[u8], png_size: u32) -> Vec<u8> {
    let mut ico = Vec::new();
    ico.push(0); ico.push(0);
    ico.push(1); ico.push(0);
    ico.push(1); ico.push(0);
    ico.push(0); ico.push(0); ico.push(0); ico.push(0);
    ico.push(1); ico.push(0);
    ico.push(32); ico.push(0);
    ico.extend_from_slice(&png_size.to_le_bytes());
    let offset: u32 = 6 + 16;
    ico.extend_from_slice(&offset.to_le_bytes());
    ico.extend_from_slice(png_data);
    ico
}
