use log::{Level, LevelFilter, Metadata, Record, SetLoggerError};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

/// 文件日志记录器
/// 将日志写入用户 AppData\Tinject\logs\tinject_YYYYMMDD.log
pub struct FileLogger {
    file: Mutex<fs::File>,
}

impl FileLogger {
    /// 初始化日志系统
    pub fn init() -> Result<(), SetLoggerError> {
        let logger = Self::new();
        log::set_boxed_logger(Box::new(logger))?;
        log::set_max_level(LevelFilter::Debug);
        Ok(())
    }

    fn new() -> Self {
        let log_dir = Self::log_dir();
        let _ = fs::create_dir_all(&log_dir);

        let date = Self::today_str();
        let path = log_dir.join(format!("tinject_{}.log", date));

        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .expect("无法创建日志文件");

        Self {
            file: Mutex::new(file),
        }
    }

    /// 获取日志目录路径（保存在用户 APPDATA 目录；无 APPDATA 时使用系统临时目录）
    pub fn log_dir() -> PathBuf {
        let base = std::env::var("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|_| std::env::temp_dir());
        base.join("Tinject").join("logs")
    }

    fn today_str() -> String {
        let now = time::OffsetDateTime::now_utc();
        format!("{:04}{:02}{:02}", now.year(), now.month() as u8, now.day())
    }
}

impl log::Log for FileLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Debug
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }

        let now = time::OffsetDateTime::now_utc();
        let timestamp = format!(
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02}.{:03}",
            now.year(),
            now.month() as u8,
            now.day(),
            now.hour(),
            now.minute(),
            now.second(),
            now.millisecond()
        );

        let file = record.file().unwrap_or("unknown");
        let line = record.line().unwrap_or(0);
        let msg = format!(
            "[{}] [{:<5}] [{}:{}] {}\n",
            timestamp,
            record.level(),
            file,
            line,
            record.args()
        );

        if let Ok(mut file) = self.file.lock() {
            let _ = file.write_all(msg.as_bytes());
            let _ = file.flush();
        }
    }

    fn flush(&self) {
        if let Ok(mut file) = self.file.lock() {
            let _ = file.flush();
        }
    }
}
