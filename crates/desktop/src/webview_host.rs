use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use webview2::{Environment, WebView, WebMessageReceivedEventArgs};
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Gdi::GetStockObject;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleA;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

fn pump_until(flag: &AtomicBool, timeout_ms: u64) -> bool {
    let start = std::time::Instant::now();
    while !flag.load(Ordering::SeqCst) {
        if start.elapsed().as_millis() as u64 > timeout_ms {
            return false;
        }
        let mut msg = unsafe { std::mem::zeroed::<MSG>() };
        if unsafe { PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, 1) } != 0 {
            unsafe {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        } else {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }
    true
}

/// Create a WebView2 window and route bridge messages to `plugin_name`.
pub fn create_webview_window(
    html_path: &str,
    window_title: &str,
    width: i32,
    height: i32,
    plugin_name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let html_path = html_path.to_string();
    let window_title = window_title.to_string();
    let plugin_name = plugin_name.to_string();

    std::thread::spawn(move || {
        unsafe {
            let _ = windows_sys::Win32::System::Com::CoInitializeEx(
                std::ptr::null_mut(),
                windows_sys::Win32::System::Com::COINIT_APARTMENTTHREADED as u32,
            );
        }

        let hinstance = unsafe { GetModuleHandleA(std::ptr::null()) };

        let class_name: Vec<u16> = "CapaHubWebView\0".encode_utf16().collect();
        let wc = WNDCLASSW {
            style: 0,
            lpfnWndProc: Some(wndproc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: hinstance,
            hIcon: std::ptr::null_mut(),
            hCursor: unsafe { LoadCursorW(std::ptr::null_mut(), IDC_ARROW) },
            hbrBackground: unsafe { GetStockObject(0) },
            lpszMenuName: std::ptr::null_mut(),
            lpszClassName: class_name.as_ptr() as *mut u16,
        };
        unsafe { RegisterClassW(&wc) };

        let title_wide: Vec<u16> = window_title.encode_utf16().chain(Some(0)).collect();
        let hwnd = unsafe {
            CreateWindowExW(
                0x00000080,
                class_name.as_ptr(),
                title_wide.as_ptr(),
                0x80000000 | 0x00C00000 | 0x00040000 | 0x00080000,
                -1, -1, width, height,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                hinstance,
                std::ptr::null(),
            )
        };

        if hwnd.is_null() {
            return;
        }

        let env_result = Arc::new(Mutex::new(None::<webview2::Result<Environment>>));
        let env_flag = Arc::new(AtomicBool::new(false));

        if Environment::builder()
            .build({
                let env_done = env_result.clone();
                let flag = env_flag.clone();
                move |result| {
                    *env_done.lock().unwrap() = Some(result);
                    flag.store(true, Ordering::SeqCst);
                    Ok(())
                }
            })
            .is_err()
        {
            unsafe { DestroyWindow(hwnd) };
            return;
        }

        if !pump_until(&env_flag, 15000) {
            unsafe { DestroyWindow(hwnd) };
            return;
        }

        let environment = match env_result.lock().unwrap().take() {
            Some(Ok(env)) => env,
            _ => {
                unsafe { DestroyWindow(hwnd) };
                return;
            }
        };

        let ctl_result = Arc::new(Mutex::new(None::<webview2::Result<webview2::Controller>>));
        let ctl_flag = Arc::new(AtomicBool::new(false));
        let hwnd_webview = hwnd as *mut winapi::shared::windef::HWND__;

        if environment
            .create_controller(hwnd_webview, {
                let ctl_done = ctl_result.clone();
                let flag = ctl_flag.clone();
                move |result| {
                    *ctl_done.lock().unwrap() = Some(result);
                    flag.store(true, Ordering::SeqCst);
                    Ok(())
                }
            })
            .is_err()
        {
            unsafe { DestroyWindow(hwnd) };
            return;
        }

        if !pump_until(&ctl_flag, 15000) {
            unsafe { DestroyWindow(hwnd) };
            return;
        }

        let controller = match ctl_result.lock().unwrap().take() {
            Some(Ok(ctl)) => ctl,
            _ => {
                unsafe { DestroyWindow(hwnd) };
                return;
            }
        };

        let webview = match controller.get_webview() {
            Ok(wv) => wv,
            _ => {
                unsafe { DestroyWindow(hwnd) };
                return;
            }
        };

        if let Ok(settings) = webview.get_settings() {
            let _ = settings.put_is_script_enabled(true);
            let _ = settings.put_is_web_message_enabled(true);
            let _ = settings.put_are_dev_tools_enabled(true);
        }

        // Route bridge messages to the owning plugin.
        let pn = plugin_name.clone();
        let _ = webview.add_web_message_received(
            move |_sender: WebView, args: WebMessageReceivedEventArgs| -> webview2::Result<()> {
                let json = args.try_get_web_message_as_string().unwrap_or_default();
                let ptr = crate::hook_manager::get_eventbus_ptr();
                if !ptr.is_null() {
                    if let Ok(msg) = serde_json::from_str::<serde_json::Value>(&json) {
                        let action = msg["action"].as_str().unwrap_or("").to_string();
                        let payload = msg["data"].to_string();
                        let eb = unsafe { &*(ptr as *const core::eventbus::EventBus) };
                        eb.publish(std::sync::Arc::new(core::event::PluginAction {
                            plugin: pn.clone(),
                            action,
                            payload,
                        }));
                    }
                }
                Ok(())
            },
        );

        let bounds = winapi::shared::windef::RECT { left: 0, top: 0, right: width, bottom: height };
        let _ = controller.put_bounds(bounds);
        let _ = controller.put_is_visible(true);

        unsafe {
            ShowWindow(hwnd, 5);
            SetForegroundWindow(hwnd);
        }

        let html_content = match std::fs::read_to_string(&html_path) {
            Ok(c) => c,
            _ => {
                unsafe { DestroyWindow(hwnd) };
                return;
            }
        };

        let _ = webview.navigate_to_string(&html_content);

        let mut msg = unsafe { std::mem::zeroed::<MSG>() };
        loop {
            let result = unsafe { GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) };
            if result == 0 || result == -1 {
                break;
            }
            unsafe {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    });

    Ok(())
}

unsafe extern "system" fn wndproc(
    hwnd: HWND, msg: u32, wparam: usize, lparam: isize,
) -> isize {
    match msg {
        WM_CLOSE => { DestroyWindow(hwnd); PostQuitMessage(0); 0 }
        WM_DESTROY => { PostQuitMessage(0); 0 }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}
