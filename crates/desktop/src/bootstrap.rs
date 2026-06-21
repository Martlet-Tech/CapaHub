use core::config::Config;
use core::eventbus::EventBus;
use core::logger::Logger;
use core::plugin_manager::PluginManager;
use core::storage::Storage;
use crossbeam_channel::unbounded;
use std::sync::Arc;

use crate::hook_manager::HookManager;
use crate::icon_loader;
use crate::log_window::LogWindow;
use crate::tray::TrayIcon;

pub struct App {
    pub config: Config,
    pub logger: Arc<Logger>,
    pub eventbus: Arc<EventBus>,
    pub hook_manager: Arc<HookManager>,
    pub plugin_manager: Arc<PluginManager>,
    pub storage: Arc<core::storage::Storage>,
    pub tray: TrayIcon,
}

impl App {
    pub fn bootstrap() -> Self {
        let config = Config::load();
        std::fs::create_dir_all(&config.logs_dir).ok();
        let logger = Arc::new(Logger::new(&config.logs_dir));
        let eventbus = Arc::new(EventBus::new());
        let hook_manager = Arc::new(HookManager::new(eventbus.clone()));

        let (log_tx, log_rx) = unbounded();
        logger.set_channel(log_tx);

        let log_window = LogWindow::new(log_rx, &config);

        let tray = TrayIcon::with_icon(icon_loader::get_tray_icon());

        let storage = Arc::new(
            Storage::new(&config.data_dir.join("storage.db"))
                .unwrap_or_else(|e| {
                    logger.error("core", &format!("Storage init failed: {}", e));
                    std::process::exit(1);
                })
        );

        let plugin_manager = Arc::new(PluginManager::new(
            eventbus.clone(),
            logger.clone(),
            Arc::new(config.clone()),
            storage.clone(),
        ));

        if let Err(e) = plugin_manager.scan_and_load() {
            logger.error("core", &format!("Plugin load error: {}", e));
        }
        plugin_manager.enable_all();

        if plugin_manager.mouse_hook_requested() {
            if let Err(e) = hook_manager.register_mouse_hook() {
                logger.error("core", &format!("Mouse hook failed: {}", e));
            } else {
                logger.info("core", "Mouse hook registered");
            }
        }
        if plugin_manager.keyboard_hook_requested() {
            if let Err(e) = hook_manager.register_keyboard_hook() {
                logger.error("core", &format!("Keyboard hook failed: {}", e));
            } else {
                logger.info("core", "Keyboard hook registered");
            }
        }

        tray.set_log_hwnd(log_window.hwnd);
        tray.set_plugin_manager(plugin_manager.clone());

        App { config, logger, eventbus, hook_manager, plugin_manager, storage, tray }
    }
}
