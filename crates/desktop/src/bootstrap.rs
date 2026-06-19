use core::config::Config;
use core::eventbus::EventBus;
use core::logger::Logger;
use core::plugin_manager::PluginManager;
use crossbeam_channel::unbounded;
use std::sync::Arc;

use crate::hook_manager::HookManager;
use crate::log_window::LogWindow;
use crate::tray::TrayIcon;

pub struct App {
    pub config: Config,
    pub logger: Arc<Logger>,
    pub eventbus: Arc<EventBus>,
    pub hook_manager: Arc<HookManager>,
    pub plugin_manager: Arc<PluginManager>,
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
        let tray = TrayIcon::new();
        tray.set_log_hwnd(log_window.hwnd);

        let mut plugin_manager = PluginManager::new(
            eventbus.clone(),
            logger.clone(),
            Arc::new(config.clone()),
        );

        if let Err(e) = plugin_manager.scan_and_load() {
            logger.error("core", &format!("插件加载失败: {}", e));
        }
        plugin_manager.enable_all();

        if plugin_manager.hook_requests().values().any(|&v| v) {
            if let Err(e) = hook_manager.register_mouse_hook() {
                logger.error("core", &format!("鼠标钩子注册失败: {}", e));
            } else {
                logger.info("core", "鼠标钩子已注册");
            }
        }

        App {
            config,
            logger,
            eventbus,
            hook_manager,
            plugin_manager: Arc::new(plugin_manager),
            tray,
        }
    }
}
