use plugin_api::{Event, MouseButton, Plugin, PluginContext, PluginContextFFI};
use std::ffi::c_void;
use std::sync::Arc;

struct TemplatePlugin {
    ctx: PluginContext,
    tracking: bool,
}

impl Plugin for TemplatePlugin {
    fn on_load(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.ctx.logger.info("template", "插件已加载");
        Ok(())
    }

    fn on_unload(&mut self) {
        self.ctx.logger.info("template", "插件已卸载");
    }

    fn on_event(&mut self, event: Arc<dyn Event>) {
        match event.event_type() {
            "mouse.down" => {
                if let Some(me) = event.mouse_event() {
                    let btn = match me.button {
                        MouseButton::Left => "Left",
                        MouseButton::Right => "Right",
                        MouseButton::Middle => "Middle",
                        _ => "Other",
                    };
                    self.ctx.logger.debug("template", &format!(
                        "mouse down: button={}, x={}, y={}", btn, me.x, me.y
                    ));
                    if me.button == MouseButton::Right {
                        self.tracking = true;
                    }
                }
            }
            "mouse.move" => {
                if self.tracking {
                    if let Some(me) = event.mouse_event() {
                        self.ctx.logger.debug("template", &format!(
                            "mouse move: x={}, y={}", me.x, me.y
                        ));
                    }
                }
            }
            "mouse.up" => {
                if self.tracking {
                    self.tracking = false;
                    if let Some(me) = event.mouse_event() {
                        self.ctx.logger.debug("template", &format!(
                            "mouse up: button=Right, x={}, y={}", me.x, me.y
                        ));
                    }
                }
            }
            _ => {}
        }
    }
}

#[no_mangle]
pub extern "C" fn plugin_create(ctx_ptr: *const PluginContextFFI) -> *mut c_void {
    let ctx = unsafe { (*ctx_ptr).to_plugin_context() };
    let plugin = TemplatePlugin {
        ctx,
        tracking: false,
    };
    let trait_obj: Box<dyn Plugin> = Box::new(plugin);
    let fat_ptr = Box::into_raw(trait_obj);
    let thin_box: Box<*mut dyn Plugin> = Box::new(fat_ptr);
    Box::into_raw(thin_box) as *mut c_void
}

#[no_mangle]
pub extern "C" fn plugin_destroy(ptr: *mut c_void) {
    if !ptr.is_null() {
        unsafe {
            let fat_ptr_box: Box<*mut dyn Plugin> = Box::from_raw(ptr as *mut *mut dyn Plugin);
            let _plugin: Box<dyn Plugin> = Box::from_raw(*fat_ptr_box);
        }
    }
}
