// Platform capability providers — registered by desktop, consumed by JsPlugin.
use std::sync::{Arc, Mutex};

type SharedPasteFn = Arc<dyn Fn(String) + Send + Sync + 'static>;
type SharedReadTextFn = Arc<dyn Fn() -> Option<String> + Send + Sync + 'static>;
type SharedSaveFileFn = Arc<dyn Fn(String, String) + Send + Sync + 'static>;
type SharedIntentFn = Arc<dyn Fn(crate::render_intent::RenderIntent) + Send + Sync + 'static>;
type SharedSendKeysFn = Arc<dyn Fn(String) + Send + Sync + 'static>;
type SharedOverlayFn = Arc<dyn Fn(String) -> Result<u32, String> + Send + Sync + 'static>;
type SharedCaptureFn = Arc<dyn Fn(i32, i32, i32, i32) -> String + Send + Sync + 'static>;

static S_INTENT: Mutex<Option<SharedIntentFn>> = Mutex::new(None);
static S_PASTE: Mutex<Option<SharedPasteFn>> = Mutex::new(None);
static S_READ_TEXT: Mutex<Option<SharedReadTextFn>> = Mutex::new(None);
static S_SAVE_FILE: Mutex<Option<SharedSaveFileFn>> = Mutex::new(None);
static S_SEND_KEYS: Mutex<Option<SharedSendKeysFn>> = Mutex::new(None);
static S_OVERLAY: Mutex<Option<SharedOverlayFn>> = Mutex::new(None);
static S_CAPTURE: Mutex<Option<SharedCaptureFn>> = Mutex::new(None);

pub fn register_intent(cb: SharedIntentFn) {
    *S_INTENT.lock().unwrap() = Some(cb);
}

pub fn register_paste(cb: SharedPasteFn) {
    *S_PASTE.lock().unwrap() = Some(cb);
}

pub fn register_read_text(cb: SharedReadTextFn) {
    *S_READ_TEXT.lock().unwrap() = Some(cb);
}

pub fn register_save_file(cb: SharedSaveFileFn) {
    *S_SAVE_FILE.lock().unwrap() = Some(cb);
}

pub fn register_send_keys(cb: SharedSendKeysFn) {
    *S_SEND_KEYS.lock().unwrap() = Some(cb);
}

pub fn register_overlay(cb: SharedOverlayFn) {
    *S_OVERLAY.lock().unwrap() = Some(cb);
}

pub fn register_capture(cb: SharedCaptureFn) {
    *S_CAPTURE.lock().unwrap() = Some(cb);
}

pub(crate) fn intent_provider() -> Option<SharedIntentFn> {
    S_INTENT.lock().unwrap().clone()
}

pub(crate) fn paste_provider() -> Option<SharedPasteFn> {
    S_PASTE.lock().unwrap().clone()
}

pub(crate) fn read_text_provider() -> Option<SharedReadTextFn> {
    S_READ_TEXT.lock().unwrap().clone()
}

pub(crate) fn save_file_provider() -> Option<SharedSaveFileFn> {
    S_SAVE_FILE.lock().unwrap().clone()
}

pub(crate) fn send_keys_provider() -> Option<SharedSendKeysFn> {
    S_SEND_KEYS.lock().unwrap().clone()
}

pub(crate) fn overlay_provider() -> Option<SharedOverlayFn> {
    S_OVERLAY.lock().unwrap().clone()
}

pub(crate) fn capture_provider() -> Option<SharedCaptureFn> {
    S_CAPTURE.lock().unwrap().clone()
}
