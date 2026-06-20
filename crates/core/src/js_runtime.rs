use crate::event::{ClipboardItemSelected, Event};
use crate::plugin::Plugin;
use crate::plugin_context::PluginContext;
use crate::render_intent::*;
use rquickjs::{context::Context, function::Func, object::Object as JsObj, Runtime};
use serde::Deserialize;
use std::path::Path;
use std::sync::{Arc, Mutex};

pub static JS_INTENT_CB: Mutex<Option<Box<dyn Fn(RenderIntent) + Send + 'static>>> = Mutex::new(None);

pub fn set_js_intent_callback(cb: Box<dyn Fn(RenderIntent) + Send + 'static>) {
    *JS_INTENT_CB.lock().unwrap() = Some(cb);
}

pub static JS_CLIPBOARD_PASTE_CB: Mutex<Option<Box<dyn Fn(String) + Send + 'static>>> = Mutex::new(None);

pub fn set_clipboard_paste_callback(cb: Box<dyn Fn(String) + Send + 'static>) {
    *JS_CLIPBOARD_PASTE_CB.lock().unwrap() = Some(cb);
}

#[derive(Deserialize)]
struct JsWindowConfig {
    width: u32,
    height: u32,
    position: String,
    auto_close: bool,
    draws: Vec<JsDrawCmd>,
}

#[derive(Deserialize)]
#[serde(tag = "type")]
enum JsDrawCmd {
    #[serde(rename = "text")]
    Text { x: i32, y: i32, text: String, font_size: u32, color: u32 },
    #[serde(rename = "separator")]
    Separator { x: i32, y: i32, width: u32, color: u32 },
    #[serde(rename = "hitarea")]
    HitArea { x: i32, y: i32, width: u32, height: u32, id: u64 },
    #[serde(rename = "image")]
    Image { x: i32, y: i32, width: u32, height: u32, data: Vec<u8> },
}

pub struct JsPlugin {
    rt: Runtime,
    context: Context,
    ctx: PluginContext,
    subs: Vec<String>,
}

fn js_func_name(event_type: &str) -> String {
    format!("on_{}", event_type.replace('.', "_"))
}

impl JsPlugin {
    pub fn load(
        script_path: &Path,
        logger: Arc<crate::logger::Logger>,
        eventbus: Arc<crate::eventbus::EventBus>,
        config_path: std::path::PathBuf,
        plugin_name: String,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let rt = Runtime::new()?;
        let context = Context::full(&rt)?;
        let script = std::fs::read_to_string(script_path)?;

        let mut subs = Vec::new();

        context.with(|ctx| {
            let global = ctx.globals();
            let js_ctx = JsObj::new(ctx.clone())?;
            let pn = plugin_name.clone();
            let l = logger.clone();
            js_ctx.set("log", Func::new(move |level: String, msg: String| {
                match level.as_str() {
                    "debug" => l.debug(&pn, &msg),
                    "info" => l.info(&pn, &msg),
                    "warn" => l.warn(&pn, &msg),
                    "error" => l.error(&pn, &msg),
                    _ => l.info(&pn, &msg),
                }
            }))?;

            let eb = eventbus.clone();
            js_ctx.set("commit_intent", Func::new(move |type_name: String, cfg_json: String| {
                let intent = match type_name.as_str() {
                    "window" => {
                        let jsc = match serde_json::from_str::<JsWindowConfig>(&cfg_json) {
                            Ok(c) => c,
                            Err(_) => return,
                        };
                        let pos = match jsc.position.as_str() {
                            "follow_focus" => WindowPosition::FollowFocus,
                            "screen_center" => WindowPosition::ScreenCenter,
                            _ => WindowPosition::NearCursor,
                        };
                        let eb3 = eb.clone();
                        RenderIntent::Window(WindowConfig {
                            width: jsc.width,
                            height: jsc.height,
                            position: pos,
                            auto_close: jsc.auto_close,
                            draws: jsc.draws.into_iter().map(|d| match d {
                                JsDrawCmd::Text { x, y, text, font_size, color } =>
                                    DrawCmd::Text(DrawText { x, y, text, font_size, color }),
                                JsDrawCmd::Separator { x, y, width, color } =>
                                    DrawCmd::Separator(DrawSeparator { x, y, width, color }),
                                JsDrawCmd::HitArea { x, y, width, height, id } =>
                                    DrawCmd::HitArea(DrawHitArea { x, y, width, height, id }),
                                JsDrawCmd::Image { x, y, width, height, data } =>
                                    DrawCmd::Image(DrawImage { x, y, width, height, data }),
                            }).collect(),
                            on_hit: Some(Arc::new(move |id| {
                                eb3.publish(Arc::new(ClipboardItemSelected { id }));
                            })),
                            on_close: None,
                        })
                    }
                    _ => return,
                };
                if let Ok(cb_lock) = JS_INTENT_CB.lock() {
                    if let Some(ref cb) = *cb_lock {
                        cb(intent);
                    }
                }
            }))?;

            js_ctx.set("close_intent", Func::new(move || {}))?;

            // ctx.clipboard.paste(text)
            let clipboard_obj = JsObj::new(ctx.clone())?;
            clipboard_obj.set("paste", Func::new(|text: String| {
                if let Ok(cb_lock) = JS_CLIPBOARD_PASTE_CB.lock() {
                    if let Some(ref cb) = *cb_lock {
                        cb(text);
                    }
                }
            }))?;
            js_ctx.set("clipboard", clipboard_obj)?;

            global.set("ctx", js_ctx)?;
            ctx.eval::<(), _>(script.as_str())?;

            for event_type in &[
                "mouse.down", "mouse.move", "mouse.up",
                "app.started", "app.shutdown",
                "hotkey.show_clipboard", "clipboard.changed",
                "clipboard.item_selected",
            ] {
                let func_name = js_func_name(event_type);
                let check = format!("typeof {} === 'function'", func_name);
                let exists: bool = ctx.eval(check.as_str())?;
                if exists {
                    subs.push(event_type.to_string());
                }
            }
            Ok::<_, rquickjs::Error>(())
        })?;

        Ok(JsPlugin {
            rt,
            context,
            ctx: PluginContext {
                logger,
                eventbus,
                config_path,
                plugin_name: plugin_name.clone(),
            },
            subs,
        })
    }

    pub fn handle_event(&self, event: &dyn Event) {
        let et = event.event_type();
        if !self.subs.iter().any(|s| s == et) {
            return;
        }
        let func_name = js_func_name(et);
        let js = if let Some(me) = event.mouse_event() {
            let btn = match me.button {
                crate::event::MouseButton::Left => "Left",
                crate::event::MouseButton::Right => "Right",
                crate::event::MouseButton::Middle => "Middle",
                _ => "None",
            };
            format!(
                "{}({{ button: \"{}\", x: {}, y: {}, timestamp: {} }})",
                func_name, btn, me.x, me.y, me.timestamp
            )
        } else if let Some(text) = event.clipboard_text() {
            let escaped = text.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n").replace('\r', "");
            format!("{}(\"{}\")", func_name, escaped)
        } else if let Some(ev) = event.as_any().downcast_ref::<ClipboardItemSelected>() {
            format!("on_clipboard_item_selected({{ id: {} }})", ev.id)
        } else {
            format!("{}()", func_name)
        };
        let _ = self.context.with(|ctx| {
            ctx.eval::<(), _>(js.as_str())
        });
    }
}

impl Plugin for JsPlugin {
    fn on_load(&mut self) -> Result<(), Box<dyn std::error::Error>> { Ok(()) }
    fn on_unload(&mut self) {}
    fn on_event(&mut self, event: Arc<dyn Event>) {
        self.handle_event(event.as_ref());
    }
}
