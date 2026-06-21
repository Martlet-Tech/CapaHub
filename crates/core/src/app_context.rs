use crate::config::Config;
use crate::eventbus::EventBus;
use crate::logger::Logger;
use crate::plugin_manager::PluginManager;
use crate::storage::StorageProvider;
use std::sync::Arc;

pub struct AppContext {
    pub eventbus: Arc<EventBus>,
    pub logger: Arc<Logger>,
    pub config: Arc<Config>,
    pub plugin_manager: Arc<PluginManager>,
    pub storage: Arc<dyn StorageProvider>,
}
