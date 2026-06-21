// Screen capture capability — GDI screenshot to file.
use std::ffi::c_void;
use std::io::Write;
use windows_sys::Win32::Graphics::Gdi::*;

pub fn capture(x: i32, y: i32, w: i32, h: i32) -> String {
    let w = w.max(1); let h = h.max(1);
    let (pixels, out_w, out_h) = unsafe { capture_raw(x, y, w, h) };
    if pixels.is_empty() { return String::new(); }

    // Save to temp dir
    let temp_dir = std::env::temp_dir();
    let path = temp_dir.join(format!("screenshot_{}.bmp", std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis()));

    let file = std::fs::File::create(&path);
    if let Ok(mut f) = file {
        write_bmp(&mut f, &pixels, out_w, out_h);
        path.to_string_lossy().to_string()
    } else {
        String::new()
    }
}

unsafe fn capture_raw(x: i32, y: i32, w: i32, h: i32) -> (Vec<u8>, i32, i32) {
    let screen_dc = GetDC(std::ptr::null_mut());
    if screen_dc.is_null() { return (vec![], 0, 0); }
    let mem_dc = CreateCompatibleDC(screen_dc);
    if mem_dc.is_null() { ReleaseDC(std::ptr::null_mut(), screen_dc); return (vec![], 0, 0); }
    let bitmap = CreateCompatibleBitmap(screen_dc, w, h);
    if bitmap.is_null() { DeleteDC(mem_dc); ReleaseDC(std::ptr::null_mut(), screen_dc); return (vec![], 0, 0); }
    let old_bmp = SelectObject(mem_dc, bitmap);
    BitBlt(mem_dc, 0, 0, w, h, screen_dc, x, y, SRCCOPY);

    let mut bmi = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: w, biHeight: -h, // top-down
            biPlanes: 1, biBitCount: 32, biCompression: 0, // BI_RGB
            biSizeImage: 0, biXPelsPerMeter: 0, biYPelsPerMeter: 0,
            biClrUsed: 0, biClrImportant: 0,
        },
        bmiColors: [RGBQUAD { rgbBlue: 0, rgbGreen: 0, rgbRed: 0, rgbReserved: 0 }],
    };
    let data_size = (w * h * 4) as usize;
    let mut pixels = vec![0u8; data_size];
    GetDIBits(mem_dc, bitmap, 0, h as u32, pixels.as_mut_ptr() as *mut c_void, &mut bmi, DIB_RGB_COLORS);

    SelectObject(mem_dc, old_bmp);
    DeleteObject(bitmap);
    DeleteDC(mem_dc);
    ReleaseDC(std::ptr::null_mut(), screen_dc);
    (pixels, w, h)
}

fn write_bmp(f: &mut dyn Write, pixels: &[u8], w: i32, h: i32) {
    let file_size = 54 + pixels.len() as u32;
    let nh = -h; // top-down DIB uses negative height
    let header: [u8; 54] = [
        b'B', b'M',
        (file_size & 0xFF) as u8, ((file_size >> 8) & 0xFF) as u8, ((file_size >> 16) & 0xFF) as u8, ((file_size >> 24) & 0xFF) as u8,
        0, 0, 0, 0,
        54, 0, 0, 0,
        40, 0, 0, 0,
        (w & 0xFF) as u8, ((w >> 8) & 0xFF) as u8, ((w >> 16) & 0xFF) as u8, ((w >> 24) & 0xFF) as u8,
        (nh & 0xFF) as u8, ((nh >> 8) & 0xFF) as u8, ((nh >> 16) & 0xFF) as u8, ((nh >> 24) & 0xFF) as u8,
        1, 0, // planes
        32, 0, // bits per pixel
        0, 0, 0, 0, // no compression
        ((pixels.len() & 0xFF) as u8), (((pixels.len() >> 8) & 0xFF) as u8), (((pixels.len() >> 16) & 0xFF) as u8), (((pixels.len() >> 24) & 0xFF) as u8),
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    ];
    let _ = f.write_all(&header);
    let _ = f.write_all(pixels);
}
