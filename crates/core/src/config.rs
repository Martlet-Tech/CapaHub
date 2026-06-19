use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub data_dir: PathBuf,
    pub plugins_dir: PathBuf,
    pub logs_dir: PathBuf,
    pub autostart: bool,
}

impl Default for Config {
    fn default() -> Self {
        let local_app_data = std::env::var("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from(r"C:\Users\Default\AppData\Local"));

        let data_dir = local_app_data.join("CapaHub");
        Config {
            plugins_dir: data_dir.join("plugins"),
            logs_dir: data_dir.join("logs"),
            autostart: false,
            data_dir,
        }
    }
}

impl Config {
    pub fn load() -> Self {
        let default = Config::default();
        let config_path = default.data_dir.join("config.toml");
        if config_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&config_path) {
                if let Ok(cfg) = toml::from_str(&content) {
                    return cfg;
                }
            }
        }
        default
    }

    pub fn save(&self) {
        let _ = std::fs::create_dir_all(&self.data_dir);
        let config_path = self.data_dir.join("config.toml");
        if let Ok(content) = toml::to_string(self) {
            let _ = std::fs::write(config_path, content);
        }
    }
}
