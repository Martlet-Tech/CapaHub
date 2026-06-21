use std::collections::HashMap;
use std::sync::Mutex;

pub trait StorageProvider: Send + Sync {
    fn get(&self, plugin: &str, key: &str) -> Option<String>;
    fn set(&self, plugin: &str, key: &str, value: &str);
}

pub struct InMemoryStorage {
    data: Mutex<HashMap<(String, String), String>>,
}

impl InMemoryStorage {
    pub fn new() -> Self {
        InMemoryStorage { data: Mutex::new(HashMap::new()) }
    }
}

impl StorageProvider for InMemoryStorage {
    fn get(&self, plugin: &str, key: &str) -> Option<String> {
        self.data.lock().unwrap().get(&(plugin.to_string(), key.to_string())).cloned()
    }
    fn set(&self, plugin: &str, key: &str, value: &str) {
        self.data.lock().unwrap().insert((plugin.to_string(), key.to_string()), value.to_string());
    }
}
