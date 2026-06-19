use crate::config::Config;
use crate::event::PluginLoaded;
use crate::eventbus::EventBus;
use crate::logger::Logger;
use crate::plugin::Plugin;
use crate::plugin_context::{PluginContext, PluginContextFFI};
use libloading::Library;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Deserialize)]
struct PluginManifest {
    plugin: PluginMeta,
    hooks: HooksDecl,
    events: EventDecl,
}

#[derive(Debug, Clone, Deserialize)]
struct PluginMeta {
    name: String,
    version: String,
    description: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct HooksDecl {
    mouse: Option<bool>,
    keyboard: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
struct EventDecl {
    subscribes: Vec<String>,
    publishes: Vec<String>,
}

enum PluginState {
    Loaded,
    Enabled,
    Unloaded,
}

type CreatePluginFn = unsafe extern "C" fn(*const PluginContextFFI) -> *mut std::ffi::c_void;
type DestroyPluginFn = unsafe extern "C" fn(*mut std::ffi::c_void);

struct LoadedPlugin {
    manifest: PluginManifest,
    _lib: Library,
    instance: Option<Arc<Mutex<Box<dyn Plugin>>>>,
    state: PluginState,
}

pub struct PluginManager {
    plugins: Vec<LoadedPlugin>,
    eventbus: Arc<EventBus>,
    logger: Arc<Logger>,
    config: Arc<Config>,
}

impl PluginManager {
    pub fn new(eventbus: Arc<EventBus>, logger: Arc<Logger>, config: Arc<Config>) -> Self {
        PluginManager {
            plugins: Vec::new(),
            eventbus,
            logger,
            config,
        }
    }

    pub fn scan_and_load(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let plugins_dir = &self.config.plugins_dir;
        if !plugins_dir.exists() {
            self.logger.info("core", &format!("插件目录不存在，跳过扫描: {:?}", plugins_dir));
            return Ok(());
        }

        let entries = fs::read_dir(plugins_dir)?;
        for entry in entries {
            let entry = entry?;
            let plugin_dir = entry.path();
            if !plugin_dir.is_dir() {
                continue;
            }

            let manifest_path = plugin_dir.join("plugin.toml");
            if !manifest_path.exists() {
                continue;
            }

            if let Err(e) = self.load_plugin(&plugin_dir) {
                self.logger.warn("core", &format!("加载插件失败 {:?}: {}", plugin_dir, e));
            }
        }

        Ok(())
    }

    fn load_plugin(&mut self, plugin_dir: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
        let manifest_path = plugin_dir.join("plugin.toml");
        let content = fs::read_to_string(&manifest_path)?;
        let manifest: PluginManifest = toml::from_str(&content)?;

        let dll_name = format!("{}.dll", manifest.plugin.name);
        let dll_path = plugin_dir.join(&dll_name);

        let plugin_name = manifest.plugin.name.clone();
        let _lib = unsafe { Library::new(&dll_path) }?;

        let ctx = PluginContext {
            plugin_name: plugin_name.clone(),
            config_path: plugin_dir.join("config"),
            logger: self.logger.clone(),
            eventbus: self.eventbus.clone(),
        };

        let ctx_ffi = PluginContextFFI::from(&ctx);

        let create_fn: libloading::Symbol<CreatePluginFn> =
            unsafe { _lib.get(b"plugin_create")? };

        let plugin_ptr = unsafe { create_fn(&ctx_ffi) };
        let fat_ptr_box: Box<*mut dyn Plugin> = unsafe { Box::from_raw(plugin_ptr as *mut *mut dyn Plugin) };
        let mut plugin: Box<dyn Plugin> = unsafe { Box::from_raw(*fat_ptr_box) };

        plugin.on_load()?;

        let plugin_arc = Arc::new(Mutex::new(plugin));
        self.plugins.push(LoadedPlugin {
            manifest,
            _lib,
            instance: Some(plugin_arc.clone()),
            state: PluginState::Loaded,
        });

        for event_type in &self.plugins.last().unwrap().manifest.events.subscribes {
            let event_type = event_type.clone();
            let pa = plugin_arc.clone();
            let log = self.logger.clone();
            self.eventbus.subscribe(&event_type, Arc::new(move |event| {
                log.debug("evbus", &format!("dispatch {} to template", event.event_type()));
                if let Ok(mut p) = pa.lock() {
                    p.on_event(event);
                }
            }));
        }

        if self.plugins.last().unwrap().manifest.hooks.mouse.unwrap_or(false) {
            self.logger.info("core", &format!("插件 '{}' 请求 mouse_hook", plugin_name));
        }

        self.logger.info("core", &format!("插件已加载: {} v{}", plugin_name, self.plugins.last().unwrap().manifest.plugin.version));
        self.eventbus.publish(Arc::new(PluginLoaded { name: plugin_name }));

        Ok(())
    }

    pub fn enable_all(&mut self) {
        for plugin in &mut self.plugins {
            plugin.state = PluginState::Enabled;
        }
        self.logger.info("core", &format!("已启用 {} 个插件", self.plugins.len()));
    }

    pub fn unload_all(&mut self) {
        for plugin in &mut self.plugins {
            if let Some(ref instance) = plugin.instance {
                if let Ok(mut p) = instance.lock() {
                    p.on_unload();
                }
            }
            plugin.state = PluginState::Unloaded;
        }
        self.logger.info("core", "所有插件已卸载");
    }

    pub fn hook_requests(&self) -> HashMap<String, bool> {
        let mut requests = HashMap::new();
        for plugin in &self.plugins {
            if let Some(mouse) = plugin.manifest.hooks.mouse {
                if mouse {
                    requests.insert(plugin.manifest.plugin.name.clone(), true);
                }
            }
        }
        requests
    }
}

impl Drop for PluginManager {
    fn drop(&mut self) {
        self.unload_all();
    }
}
