use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// 应用程序配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// 注入配置
    pub injection: InjectionConfig,
    /// UI配置
    pub ui: UiConfig,
}

/// 持久化的进程选择项
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedProcess {
    /// 进程名称
    pub name: String,
    /// 是否被选中
    pub selected: bool,
}

/// 注入配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InjectionConfig {
    /// 目标进程名称（单个，向后兼容）
    #[serde(default)]
    pub target_process: String,
    /// 目标进程列表（多个，向后兼容）
    #[serde(default)]
    pub target_processes: Vec<String>,
    /// 持久化的进程选择列表（含选中状态）
    #[serde(default)]
    pub persisted_processes: Vec<PersistedProcess>,
    /// 注入方式
    #[serde(default = "default_method")]
    pub method: String,
    /// DLL文件路径列表
    #[serde(default)]
    pub dll_paths: Vec<String>,
    /// 批量注入间隔（毫秒）
    #[serde(default = "default_batch_delay")]
    pub batch_delay_ms: u64,
    /// 等待进程启动超时（毫秒）
    #[serde(default = "default_timeout")]
    pub process_timeout_ms: u64,
    /// 自动降级到备用注入方式
    #[serde(default = "default_auto_fallback")]
    pub auto_fallback: bool,
}

fn default_method() -> String {
    "auto".to_string()
}

fn default_batch_delay() -> u64 {
    500
}

fn default_timeout() -> u64 {
    30000
}

fn default_auto_fallback() -> bool {
    true
}

/// UI配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    /// 主题：dark, light, glass
    pub theme: String,
    /// 透明度 0.0-1.0
    pub opacity: f32,
    /// 模糊强度
    pub blur_intensity: u32,
    /// 主色调
    pub accent_color: String,
    /// 窗口宽度
    pub window_width: f64,
    /// 窗口高度
    pub window_height: f64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            injection: InjectionConfig {
                target_process: "javaw.exe".to_string(),
                target_processes: vec!["javaw.exe".to_string()],
                persisted_processes: vec![PersistedProcess {
                    name: "javaw.exe".to_string(),
                    selected: true,
                }],
                method: "auto".to_string(),
                dll_paths: Vec::new(),
                batch_delay_ms: 500,
                process_timeout_ms: 30000,
                auto_fallback: true,
            },
            ui: UiConfig {
                theme: "glass".to_string(),
                opacity: 0.85,
                blur_intensity: 20,
                accent_color: "#00d4ff".to_string(),
                window_width: 900.0,
                window_height: 620.0,
            },
        }
    }
}

impl Config {
    /// 获取配置文件路径（保存在用户 APPDATA 目录；无 APPDATA 时使用系统临时目录）
    pub fn config_path() -> PathBuf {
        let base = std::env::var("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|_| std::env::temp_dir());
        let dir = base.join("Tinject");
        if !dir.exists() {
            let _ = fs::create_dir_all(&dir);
        }
        dir.join("config.json")
    }

    /// 加载配置
    pub fn load() -> Self {
        let path = Self::config_path();
        log::debug!("加载配置文件: {:?}", path);
        if path.exists() {
            match fs::read_to_string(&path) {
                Ok(content) => {
                    match serde_json::from_str::<Config>(&content) {
                        Ok(config) => {
                            log::info!("配置加载成功");
                            config
                        }
                        Err(e) => {
                            log::error!("配置文件解析失败，使用默认配置: {}", e);
                            Self::default()
                        }
                    }
                }
                Err(e) => {
                    log::error!("读取配置文件失败: {}", e);
                    Self::default()
                }
            }
        } else {
            log::info!("配置文件不存在，使用默认配置");
            Self::default()
        }
    }

    /// 保存配置
    pub fn save(&self) -> Result<(), String> {
        let path = Self::config_path();
        log::debug!("保存配置文件到: {:?}", path);
        let content = serde_json::to_string_pretty(self)
            .map_err(|e| {
                log::error!("序列化配置失败: {}", e);
                format!("序列化配置失败: {}", e)
            })?;
        fs::write(&path, content)
            .map_err(|e| {
                log::error!("写入配置文件失败: {}", e);
                format!("写入配置文件失败: {}", e)
            })?;
        log::info!("配置文件保存成功");
        Ok(())
    }
}
