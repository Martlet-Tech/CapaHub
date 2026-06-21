use crate::capability;
use crate::event::Event;
use crate::plugin::Plugin;
use crate::plugin_context::PluginContext;
use crate::render_intent::*;
use crate::storage::Storage;
use rquickjs::{context::Context, function::Func, object::Object as JsObj, Runtime};
use serde::Deserialize;
use std::path::Path;
use std::sync::Arc;

#[derive(Deserialize)]
struct JsWindowConfig {
    width: u32,
    height: u32,
    position: String,
    auto_close: bool,
    draws: Vec<JsDrawCmd>,
    #[serde(default)]
    selected_index: usize,
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

#[derive(Debug, Clone, Default)]
pub struct JsCapabilities {
    pub clipboard: bool,
    pub input: bool,
    pub overlay: bool,
    pub screen: bool,
}

impl JsPlugin {
    pub fn load(
        script_path: &Path,
        logger: Arc<crate::logger::Logger>,
        eventbus: Arc<crate::eventbus::EventBus>,
        storage: Arc<Storage>,
        config_path: std::path::PathBuf,
        plugin_name: String,
        subscribes: &[String],
        caps: JsCapabilities,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let rt = Runtime::new()?;
        let context = Context::full(&rt)?;
        let script = std::fs::read_to_string(script_path)?;

        let mut subs = Vec::new();
        let pn_store = plugin_name.clone();
        let st = storage.clone();

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
            let log2 = logger.clone();
            js_ctx.set("commit_intent", Func::new(move |type_name: String, cfg_json: String| {
                let intent = match type_name.as_str() {
                    "window" => {
                        let jsc = match serde_json::from_str::<JsWindowConfig>(&cfg_json) {
                            Ok(c) => c,
                            Err(e) => {
                                log2.error("js", &format!("commit_intent parse fail: {}", e));
                                return;
                            }
                        };
                        let pos = match jsc.position.as_str() {
                            "follow_focus" => WindowPosition::FollowFocus,
                            "screen_center" => WindowPosition::ScreenCenter,
                            _ => WindowPosition::NearCursor,
                        };
                        let eb3 = eb.clone();
                        let eb4 = eb.clone();
                        RenderIntent::Window(WindowConfig {
                            width: jsc.width,
                            height: jsc.height,
                            position: pos,
                            auto_close: jsc.auto_close,
                            selected_index: jsc.selected_index,
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
                                eb3.publish(Arc::new(crate::event::DynamicEvent {
                                    event_type: "clipboard.item_selected",
                                    repr: format!("{{ id: {} }}", id),
                                }));
                            })),
                            on_delete: Some(Arc::new(move |id| {
                                eb4.publish(Arc::new(crate::event::DynamicEvent {
                                    event_type: "clipboard.item_deleted",
                                    repr: format!("{{ id: {} }}", id),
                                }));
                            })),
                            on_close: None,
                        })
                    }
                    _ => return,
                };
                if let Some(provider) = capability::intent_provider() {
                    provider(intent);
                }
            }))?;

            js_ctx.set("close_intent", Func::new(move || {}))?;

            js_ctx.set("save_file_dialog", Func::new(|content: String, default_name: String| {
                if let Some(provider) = capability::save_file_provider() {
                    provider(content, default_name);
                }
            }))?;

            if caps.clipboard {
                let clipboard_obj = JsObj::new(ctx.clone())?;
                clipboard_obj.set("paste", Func::new(|text: String| {
                    if let Some(provider) = capability::paste_provider() {
                        provider(text);
                    }
                }))?;
                clipboard_obj.set("readText", Func::new(|| -> Option<String> {
                    capability::read_text_provider().and_then(|p| p())
                }))?;
                js_ctx.set("clipboard", clipboard_obj)?;
            }

            if caps.input {
                let input_obj = JsObj::new(ctx.clone())?;
                input_obj.set("sendKeys", Func::new(|keys: String| {
                    if let Some(provider) = capability::send_keys_provider() {
                        provider(keys);
                    }
                }))?;
                js_ctx.set("input", input_obj)?;
            }

            // ── ctx.overlay (only if capability declared) ─────
            if caps.overlay {
                let overlay_obj = JsObj::new(ctx.clone())?;
                overlay_obj.set("cmd", Func::new(|json: String| -> String {
                    capability::overlay_provider()
                        .and_then(|p| match p(json) {
                            Ok(handle) => Some(handle.to_string()),
                            Err(e) => Some(format!("error:{}", e)),
                        })
                        .unwrap_or_else(|| "error:no provider".to_string())
                }))?;
                js_ctx.set("overlay", overlay_obj)?;
            }

            // ── ctx.screen (only if capability declared) ──────
            if caps.screen {
                let screen_obj = JsObj::new(ctx.clone())?;
                screen_obj.set("capture", Func::new(|x: i32, y: i32, w: i32, h: i32| -> String {
                    capability::capture_provider()
                        .map(|p| p(x, y, w, h))
                        .unwrap_or_default()
                }))?;
                js_ctx.set("screen", screen_obj)?;
            }

            let st2 = st.clone();
            let pns2 = pn_store.clone();
            let storage_obj = JsObj::new(ctx.clone())?;
            storage_obj.set("get", Func::new(move |key: String| -> Option<String> {
                st2.get(&pns2, &key)
            }))?;
            let st3 = st.clone();
            let pns3 = pn_store.clone();
            storage_obj.set("set", Func::new(move |key: String, value: String| {
                st3.set(&pns3, &key, &value);
            }))?;
            js_ctx.set("storage", storage_obj)?;

            global.set("ctx", js_ctx)?;
            ctx.eval::<(), _>(script.as_str())?;

            for event_type in subscribes {
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
                storage,
                config_path,
                plugin_name: plugin_name.clone(),
            },
            subs,
        })
    }
}

impl Plugin for JsPlugin {
    fn on_load(&mut self) -> Result<(), Box<dyn std::error::Error>> { Ok(()) }
    fn on_unload(&mut self) {}
    fn on_event(&mut self, event: Arc<dyn Event>) {
        let et = event.event_type();
        if !self.subs.iter().any(|s| s == et) {
            return;
        }
        let func_name = js_func_name(et);
        let repr = event.js_repr();
        let js = if repr.is_empty() {
            format!("{}()", func_name)
        } else {
            format!("{}({})", func_name, repr)
        };
        let _ = self.context.with(|ctx| {
            ctx.eval::<(), _>(js.as_str())
        });
    }
}
