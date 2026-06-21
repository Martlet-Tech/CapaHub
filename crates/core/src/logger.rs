use chrono::Local;
use crossbeam_channel::Sender;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::collections::VecDeque;

static FILE_ENABLED: AtomicBool = AtomicBool::new(true);

pub fn set_file_logging(enabled: bool) {
    FILE_ENABLED.store(enabled, Ordering::SeqCst);
}

#[derive(Debug, Clone)]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

impl std::fmt::Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LogLevel::Debug => write!(f, "DEBUG"),
            LogLevel::Info => write!(f, "INFO"),
            LogLevel::Warn => write!(f, "WARN"),
            LogLevel::Error => write!(f, "ERROR"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct LogEntry {
    pub level: LogLevel,
    pub message: String,
    pub plugin: String,
    pub timestamp: u64,
}

pub struct Logger {
    ring: Mutex<VecDeque<LogEntry>>,
    file: Mutex<fs::File>,
    channel: Mutex<Option<Sender<LogEntry>>>,
}

impl Logger {
    pub fn new(log_dir: &Path) -> Self {
        fs::create_dir_all(log_dir).ok();
        let log_path = log_dir.join("agent.log");
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_path)
            .expect("无法打开日志文件");

        Logger {
            ring: Mutex::new(VecDeque::with_capacity(2000)),
            file: Mutex::new(file),
            channel: Mutex::new(None),
        }
    }

    pub fn set_channel(&self, tx: Sender<LogEntry>) {
        *self.channel.lock().unwrap() = Some(tx);
    }

    pub fn log(&self, level: LogLevel, plugin: &str, message: &str) {
        let now = Local::now();
        let timestamp = now.timestamp_millis() as u64;
        let entry = LogEntry {
            level: level.clone(),
            message: message.to_string(),
            plugin: plugin.to_string(),
            timestamp,
        };

        {
            let mut ring = self.ring.lock().unwrap();
            if ring.len() >= 2000 {
                ring.pop_front();
            }
            ring.push_back(entry.clone());
        }

        if FILE_ENABLED.load(Ordering::SeqCst) {
            let mut file = self.file.lock().unwrap();
            let _ = writeln!(
                file,
                "[{}] [{}] [{}] {}",
                now.format("%Y-%m-%d %H:%M:%S"),
                level,
                plugin,
                message
            );
        }

        if let Some(tx) = self.channel.lock().unwrap().as_ref() {
            let _ = tx.send(entry);
        }
    }

    pub fn debug(&self, plugin: &str, message: &str) {
        self.log(LogLevel::Debug, plugin, message);
    }

    pub fn info(&self, plugin: &str, message: &str) {
        self.log(LogLevel::Info, plugin, message);
    }

    pub fn warn(&self, plugin: &str, message: &str) {
        self.log(LogLevel::Warn, plugin, message);
    }

    pub fn error(&self, plugin: &str, message: &str) {
        self.log(LogLevel::Error, plugin, message);
    }

    pub fn recent(&self, count: usize) -> Vec<LogEntry> {
        let ring = self.ring.lock().unwrap();
        let len = ring.len();
        let start = if len > count { len - count } else { 0 };
        ring.iter().skip(start).take(count).cloned().collect()
    }
}
