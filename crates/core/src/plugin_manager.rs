use crate::config::Config;
use crate::event::{PluginDisabled, PluginEnabled, PluginLoaded};
use crate::eventbus::{EventBus, SubscriptionId};
use crate::logger::Logger;
use crate::plugin::Plugin;
use crate::plugin_context::PluginContext;
use plugin_api::PluginContextFFI;
use crate::storage::Storage;
use libloading::Library;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Deserialize)]
struct PluginManifest {
    plugin: PluginMeta,
    hooks: HooksDecl,
    events: EventDecl,
    #[serde(default)]
    capabilities: CapabilitiesDecl,
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

#[derive(Debug, Clone, Deserialize, Default)]
struct CapabilitiesDecl {
    #[serde(default)]
    clipboard: bool,
    #[serde(default)]
    input: bool,
    #[serde(default)]
    overlay: bool,
    #[serde(default)]
    screen: bool,
}

#[derive(Debug, Clone, Deserialize)]
struct EventDecl {
    subscribes: Vec<String>,
    publishes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PluginState {
    Loaded,
    Enabled,
    Disabled,
    Unloaded,
}

#[derive(Debug, Clone)]
pub struct PluginInfo {
    pub name: String,
    pub version: String,
    pub description: String,
    pub state: PluginState,
}

struct LoadedPlugin {
    manifest: PluginManifest,
    instance: Arc<Mutex<Box<dyn Plugin>>>,
    state: PluginState,
    subscriptions: Vec<SubscriptionId>,
    _lib: Option<Library>,
}

pub struct PluginManager {
    plugins: Mutex<Vec<LoadedPlugin>>,
    eventbus: Arc<EventBus>,
    logger: Arc<Logger>,
    config: Arc<Config>,
    storage: Arc<Storage>,
}

impl PluginManager {
    pub fn new(eventbus: Arc<EventBus>, logger: Arc<Logger>, config: Arc<Config>, storage: Arc<Storage>) -> Self {
        PluginManager {
            plugins: Mutex::new(Vec::new()),
            eventbus,
            logger,
            config,
            storage,
        }
    }

    pub fn scan_and_load(&self) -> Result<(), Box<dyn std::error::Error>> {
        let plugins_dir = &self.config.plugins_dir;
        if !plugins_dir.exists() {
            self.logger.info("core", &format!("Plugin dir not found: {:?}", plugins_dir));
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
                self.logger.warn("core", &format!("Load plugin failed {:?}: {}", plugin_dir, e));
            }
        }
        Ok(())
    }

    pub fn install_from_path(&self, dap_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        let file = fs::File::open(dap_path)?;
        let mut archive = zip::ZipArchive::new(file)?;

        let mut manifest_content = String::new();

        for i in 0..archive.len() {
            let mut entry = archive.by_index(i)?;
            let path = entry.name().to_string().replace('\\', "/");

            if path == "plugin.toml" {
                manifest_content.clear();
                std::io::Read::read_to_string(&mut entry, &mut manifest_content)?;
            }
        }

        if manifest_content.is_empty() {
            return Err("plugin.toml not found in archive".into());
        }

        let manifest: PluginManifest = toml::from_str(&manifest_content)?;
        let plugin_name = manifest.plugin.name.clone();
        let dll_target = format!("{}.dll", plugin_name);
        let target_dir = self.config.plugins_dir.join(&plugin_name);

        if target_dir.exists() {
            return Err(format!("Plugin '{}' already exists", plugin_name).into());
        }

        fs::create_dir_all(&target_dir)?;

        for i in 0..archive.len() {
            let mut entry = archive.by_index(i)?;
            let path = entry.name().to_string().replace('\\', "/");
            let file_name = path.rsplit('/').next().unwrap_or("");

            if file_name.is_empty() || path == "plugin.toml" {
                let dest = target_dir.join("plugin.toml");
                let mut out = fs::File::create(&dest)?;
                std::io::copy(&mut entry, &mut out)?;
                continue;
            }

            if path.starts_with("assets/") || path.contains("/assets/") {
                let rel = path.trim_start_matches("assets/").trim_start_matches('/');
                let dest_dir = target_dir.join("assets");
                fs::create_dir_all(&dest_dir)?;
                let dest = dest_dir.join(rel);
                let mut out = fs::File::create(&dest)?;
                std::io::copy(&mut entry, &mut out)?;
                continue;
            }

            if file_name == dll_target {
                let dest = target_dir.join(&dll_target);
                let mut out = fs::File::create(&dest)?;
                std::io::copy(&mut entry, &mut out)?;
                continue;
            }

            if file_name == "index.html" {
                let dest = target_dir.join("index.html");
                let mut out = fs::File::create(&dest)?;
                std::io::copy(&mut entry, &mut out)?;
                continue;
            }
        }

        self.load_plugin(&target_dir)?;
        self.logger.info("core", &format!("Installed plugin: {}", plugin_name));
        Ok(())
    }

    pub fn enable_plugin(&self, name: &str) -> Result<(), Box<dyn std::error::Error>> {
        let (needs_hook, result) = {
            let mut plugins = self.plugins.lock().unwrap();
            let plugin = plugins.iter_mut().find(|p| p.manifest.plugin.name == name);
            match plugin {
                Some(p) if p.state == PluginState::Loaded || p.state == PluginState::Disabled => {
                    let r = {
                        let mut inst = p.instance.lock().unwrap();
                        inst.on_enable()
                    };
                    if let Err(e) = r {
                        (false, Err(e))
                    } else {
                        let subs = self.subscribe_plugin_events(&p.manifest, &p.instance);
                        p.subscriptions = subs;
                        p.state = PluginState::Enabled;
                        let hook = p.manifest.hooks.mouse.unwrap_or(false);
                        self.logger.info("core", &format!("Plugin enabled: {}", name));
                        (hook, Ok(()))
                    }
                }
                Some(p) if p.state == PluginState::Enabled => (false, Ok(())),
                Some(_) => (false, Err("Plugin is unloaded".into())),
                None => (false, Err(format!("Plugin not found: {}", name).into())),
            }
        };
        if result.is_ok() {
            self.eventbus.publish(Arc::new(PluginEnabled {
                name: name.to_string(),
                needs_hook,
            }));
        }
        result
    }

    pub fn disable_plugin(&self, name: &str) -> Result<(), Box<dyn std::error::Error>> {
        let should_publish = {
            let mut plugins = self.plugins.lock().unwrap();
            let plugin = plugins.iter_mut().find(|p| p.manifest.plugin.name == name);
            match plugin {
                Some(p) if p.state == PluginState::Enabled => {
                    for sid in &p.subscriptions {
                        self.eventbus.unsubscribe(*sid);
                    }
                    p.subscriptions.clear();
                    if let Ok(mut inst) = p.instance.lock() {
                        inst.on_disable();
                    }
                    p.state = PluginState::Disabled;
                    self.logger.info("core", &format!("Plugin disabled: {}", name));
                    true
                }
                _ => false,
            }
        };
        if should_publish {
            self.eventbus.publish(Arc::new(PluginDisabled { name: name.to_string() }));
        }
        Ok(())
    }

    pub fn delete_plugin(&self, name: &str) -> Result<(), Box<dyn std::error::Error>> {
        let had_hooks = {
            let mut plugins = self.plugins.lock().unwrap();
            if let Some(pos) = plugins.iter().position(|p| p.manifest.plugin.name == name) {
                let mut p = plugins.remove(pos);
                for sid in &p.subscriptions {
                    self.eventbus.unsubscribe(*sid);
                }
                p.subscriptions.clear();
                let had = p.manifest.hooks.mouse.unwrap_or(false);
                if let Ok(mut inst) = p.instance.lock() {
                    inst.on_unload();
                }
                p.state = PluginState::Unloaded;
                had
            } else {
                false
            }
        };
        if had_hooks {
            self.eventbus.publish(Arc::new(PluginDisabled { name: name.to_string() }));
        }
        let plugin_dir = self.config.plugins_dir.join(name);
        if plugin_dir.exists() {
            fs::remove_dir_all(&plugin_dir)?;
        }
        self.logger.info("core", &format!("Plugin deleted: {}", name));
        Ok(())
    }

    pub fn plugin_list(&self) -> Vec<PluginInfo> {
        let plugins = self.plugins.lock().unwrap();
        plugins
            .iter()
            .map(|p| PluginInfo {
                name: p.manifest.plugin.name.clone(),
                version: p.manifest.plugin.version.clone(),
                description: p
                    .manifest
                    .plugin
                    .description
                    .clone()
                    .unwrap_or_default(),
                state: p.state.clone(),
            })
            .collect()
    }

    pub fn has_plugin(&self, name: &str) -> bool {
        let plugins = self.plugins.lock().unwrap();
        plugins.iter().any(|p| p.manifest.plugin.name == name)
    }

    pub fn logger(&self) -> &Logger {
        &self.logger
    }

    pub fn plugin_dir(&self, name: &str) -> Option<PathBuf> {
        let path = self.config.plugins_dir.join(name);
        if path.exists() { Some(path) } else { None }
    }

    pub fn enable_all(&self) {
        let names: Vec<String> = {
            let plugins = self.plugins.lock().unwrap();
            plugins
                .iter()
                .filter(|p| p.state == PluginState::Loaded)
                .map(|p| p.manifest.plugin.name.clone())
                .collect()
        };
        for name in names {
            if let Err(e) = self.enable_plugin(&name) {
                self.logger.warn("core", &format!("Enable plugin '{}' failed: {}", name, e));
            }
        }
        self.logger.info("core", &format!("Enabled {} plugins", {
            let plugins = self.plugins.lock().unwrap();
            plugins.iter().filter(|p| p.state == PluginState::Enabled).count()
        }));
    }

    pub fn unload_all(&self) {
        let names: Vec<String> = {
            let plugins = self.plugins.lock().unwrap();
            plugins.iter().map(|p| p.manifest.plugin.name.clone()).collect()
        };
        for name in names {
            let _ = self.disable_plugin(&name);
            let plugin = {
                let mut plugins = self.plugins.lock().unwrap();
                plugins.iter().position(|p| p.manifest.plugin.name == name).map(|pos| plugins.remove(pos))
            };
            if let Some(mut p) = plugin {
                p.state = PluginState::Unloaded;
                if let Ok(mut inst) = p.instance.lock() {
                    inst.on_unload();
                }
            }
        }
        self.logger.info("core", "All plugins unloaded");
    }

    pub fn hook_requests(&self) -> HashMap<String, bool> {
        let plugins = self.plugins.lock().unwrap();
        let mut requests = HashMap::new();
        for plugin in plugins.iter() {
            if let Some(mouse) = plugin.manifest.hooks.mouse {
                if mouse && plugin.state == PluginState::Enabled {
                    requests.insert(plugin.manifest.plugin.name.clone(), true);
                }
            }
        }
        requests
    }

    pub fn any_hook_requested(&self) -> bool {
        let plugins = self.plugins.lock().unwrap();
        plugins.iter().any(|p| {
            p.manifest.hooks.mouse.unwrap_or(false) && p.state == PluginState::Enabled
        })
    }

    fn load_plugin(&self, plugin_dir: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
        let manifest_path = plugin_dir.join("plugin.toml");
        let content = fs::read_to_string(&manifest_path)?;
        let manifest: PluginManifest = toml::from_str(&content)?;

        let plugin_name = manifest.plugin.name.clone();

        {
            let plugins = self.plugins.lock().unwrap();
            if plugins.iter().any(|p| p.manifest.plugin.name == plugin_name) {
                return Err(format!("Plugin '{}' already loaded", plugin_name).into());
            }
        }

        // Check if this is a JS plugin (main.js) or Rust plugin (.dll)
        let js_path = plugin_dir.join("main.js");
        if js_path.exists() {
            return self.load_js_plugin(plugin_dir, &manifest, &plugin_name);
        }

        let dll_name = format!("{}.dll", manifest.plugin.name);
        let dll_path = plugin_dir.join(&dll_name);
        let _lib = unsafe { Library::new(&dll_path) }?;

        let ctx = PluginContext {
            plugin_name: plugin_name.clone(),
            config_path: plugin_dir.join("config"),
            logger: self.logger.clone(),
            eventbus: self.eventbus.clone(),
            storage: self.storage.clone(),
        };

        let ctx_ffi = ctx.to_ffi();

        let create_fn: libloading::Symbol<
            unsafe extern "C" fn(*const PluginContextFFI) -> *mut std::ffi::c_void,
        > = unsafe { _lib.get(b"plugin_create")? };

        let plugin_ptr = unsafe { create_fn(&ctx_ffi) };
        let fat_ptr_box: Box<*mut dyn Plugin> =
            unsafe { Box::from_raw(plugin_ptr as *mut *mut dyn Plugin) };
        let mut plugin: Box<dyn Plugin> = unsafe { Box::from_raw(*fat_ptr_box) };

        plugin.on_load()?;

        let plugin_arc = Arc::new(Mutex::new(plugin));

        let mut plugins = self.plugins.lock().unwrap();
        plugins.push(LoadedPlugin {
            manifest,
            _lib: Some(_lib),
            instance: plugin_arc,
            state: PluginState::Loaded,
            subscriptions: Vec::new(),
        });

        self.logger.info("core", &format!("Plugin loaded: {} v{}", plugin_name, plugins.last().unwrap().manifest.plugin.version));
        self.eventbus.publish(Arc::new(PluginLoaded { name: plugin_name }));

        Ok(())
    }

    fn load_js_plugin(
        &self,
        plugin_dir: &PathBuf,
        manifest: &PluginManifest,
        plugin_name: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let js_path = plugin_dir.join("main.js");
        if !js_path.exists() {
            return Err("main.js not found".into());
        }
        let caps = crate::js_runtime::JsCapabilities {
            clipboard: manifest.capabilities.clipboard,
            input: manifest.capabilities.input,
            overlay: manifest.capabilities.overlay,
            screen: manifest.capabilities.screen,
        };
        let plugin = crate::js_runtime::JsPlugin::load(
            &js_path,
            self.logger.clone(),
            self.eventbus.clone(),
            self.storage.clone(),
            plugin_dir.join("config"),
            plugin_name.to_string(),
            &manifest.events.subscribes,
            caps,
        )?;
        let plugin_arc = Arc::new(Mutex::new(Box::new(plugin) as Box<dyn Plugin>));
        self.plugins.lock().unwrap().push(LoadedPlugin {
            manifest: manifest.clone(),
            instance: plugin_arc,
            state: PluginState::Loaded,
            subscriptions: Vec::new(),
            _lib: None,
        });
        self.logger.info("core", &format!("JS plugin loaded: {}", plugin_name));
        self.eventbus.publish(Arc::new(PluginLoaded { name: plugin_name.to_string() }));
        Ok(())
    }

    fn subscribe_plugin_events(
        &self,
        manifest: &PluginManifest,
        instance: &Arc<Mutex<Box<dyn Plugin>>>,
    ) -> Vec<SubscriptionId> {
        let mut ids = Vec::new();
        let log = self.logger.clone();
        let plugin_name = manifest.plugin.name.clone();
        let count = Arc::new(AtomicU64::new(0));
        for event_type in &manifest.events.subscribes {
            let pa = instance.clone();
            let l = log.clone();
            let et = event_type.clone();
            let pn = plugin_name.clone();
            let c = count.clone();
            let id = self.eventbus.subscribe(&et, Arc::new(move |event| {
                if event.event_type() == "mouse.move" {
                    let n = c.fetch_add(1, Ordering::Relaxed);
                    if n % 100 != 0 {
                        if let Ok(mut p) = pa.lock() { p.on_event(event); }
                        return;
                    }
                }
                l.debug("evbus", &format!("dispatch {} to {}", event.event_type(), pn));
                if let Ok(mut p) = pa.lock() {
                    p.on_event(event);
                }
            }));
            ids.push(id);
        }
        ids
    }
}
