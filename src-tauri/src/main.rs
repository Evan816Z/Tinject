#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![cfg_attr(debug_assertions, windows_subsystem = "windows")]

mod cli;
mod config;
mod injector;

use config::Config;
use injector::InjectionMethod;
use injector::process;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tao::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};
use wry::{http::Response, WebViewBuilder};
use std::borrow::Cow;
use clap::Parser;

/// 应用状态
struct AppState {
    config: Config,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            config: Config::load(),
        }
    }
}

/// 注入请求
#[derive(Debug, Deserialize)]
struct InjectRequest {
    dll_paths: Vec<String>,
    method: String,
    batch_delay_ms: u64,
    target_processes: Vec<String>,
}

/// 注入响应
#[derive(Debug, Serialize)]
struct InjectResponse {
    success: bool,
    results: Vec<InjectResult>,
    message: String,
}

#[derive(Debug, Serialize)]
struct InjectResult {
    dll_path: String,
    success: bool,
    method: String,
    message: String,
}

/// 执行DLL注入
fn execute_injection(request: InjectRequest, config: &Config) -> InjectResponse {
    let timeout_ms = config.injection.process_timeout_ms;
    let auto_fallback = config.injection.auto_fallback;

    let mut results = Vec::new();
    let mut all_success = true;

    // 解析注入方式
    let method: Result<InjectionMethod, _> = request.method.parse();
    let method = method.unwrap_or(InjectionMethod::CreateRemoteThread);

    // 遍历所有目标进程
    for process_name in &request.target_processes {
        // 等待目标进程启动
        let pid = match process::wait_for_process(process_name, timeout_ms) {
            Some(pid) => pid,
            None => {
                results.push(InjectResult {
                    dll_path: String::new(),
                    success: false,
                    method: String::new(),
                    message: format!("无法找到进程: {}", process_name),
                });
                all_success = false;
                continue;
            }
        };

        // 批量注入DLL
        for (index, dll_path) in request.dll_paths.iter().enumerate() {
            let result = if auto_fallback && request.method == "auto" {
                injector::inject_dll_auto(pid, dll_path)
            } else {
                injector::inject_dll(pid, dll_path, method)
            };

            let inject_result = match result {
                Ok(r) => InjectResult {
                    dll_path: dll_path.clone(),
                    success: true,
                    method: r.method,
                    message: r.message,
                },
                Err(e) => {
                    all_success = false;
                    InjectResult {
                        dll_path: dll_path.clone(),
                        success: false,
                        method: method.to_string(),
                        message: format!("注入失败: {}", e),
                    }
                }
            };

            results.push(inject_result);

            // 批量注入间隔
            if index < request.dll_paths.len() - 1 && request.batch_delay_ms > 0 {
                std::thread::sleep(std::time::Duration::from_millis(request.batch_delay_ms));
            }
        }
    }

    InjectResponse {
        success: all_success,
        results,
        message: if all_success {
            "所有DLL注入成功".to_string()
        } else {
            "部分DLL注入失败".to_string()
        },
    }
}

/// 获取前端资源目录的绝对路径
fn get_src_dir() -> std::path::PathBuf {
    let exe_dir = std::env::current_exe()
        .unwrap_or_default()
        .parent()
        .unwrap_or(std::path::Path::new("."))
        .to_path_buf();
    // 开发模式：exe 在 src-tauri/target/debug/，前端在项目根 src/
    let dev_path = exe_dir.join("../../../src");
    if dev_path.join("index.html").exists() {
        return dev_path;
    }
    // 发布模式：前端在 exe 同级 src/ 目录
    exe_dir.join("src")
}

/// 根据文件扩展名返回 MIME 类型
fn mime_type(path: &str) -> &'static str {
    if path.ends_with(".html") {
        "text/html"
    } else if path.ends_with(".css") {
        "text/css"
    } else if path.ends_with(".js") {
        "application/javascript"
    } else if path.ends_with(".png") {
        "image/png"
    } else if path.ends_with(".jpg") || path.ends_with(".jpeg") {
        "image/jpeg"
    } else if path.ends_with(".svg") {
        "image/svg+xml"
    } else if path.ends_with(".ico") {
        "image/x-icon"
    } else {
        "application/octet-stream"
    }
}

/// 打开系统文件选择对话框
fn open_file_dialog(filter: Option<&str>) -> Option<String> {
    let mut dialog = rfd::FileDialog::new();

    match filter {
        Some("dll") => {
            dialog = dialog.add_filter("DLL 文件", &["dll"]);
        }
        Some("image") => {
            dialog = dialog.add_filter("图片文件", &["png", "jpg", "jpeg", "bmp", "webp"]);
        }
        _ => {}
    }

    dialog.pick_file().map(|p| p.to_string_lossy().to_string())
}

fn main() {
    // 初始化日志
    env_logger::init();

    // 解析命令行参数
    let cli = cli::Cli::parse();

    // 如果有子命令，执行CLI模式
    if cli.command.is_some() {
        cli::execute_cli(cli);
        return;
    }

    // 否则启动GUI模式
    let state = Arc::new(Mutex::new(AppState::default()));
    let pending_callbacks: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title("Tinject")
        .with_inner_size(tao::dpi::LogicalSize::new(900.0, 620.0))
        .with_min_inner_size(tao::dpi::LogicalSize::new(780.0, 520.0))
        .with_decorations(false)
        .with_transparent(true)
        .build(&event_loop)
        .expect("创建窗口失败");

    let state_clone = state.clone();
    let pending_clone = pending_callbacks.clone();
    let src_dir = get_src_dir();

    // 创建 WebView，使用自定义协议加载本地文件
    let webview = WebViewBuilder::new()
        .with_custom_protocol("tinject".into(), move |_webview_id, request| {
            let path = request.uri().path();
            let relative = path.trim_start_matches('/');

            // 解码 URL 编码
            let decoded = percent_encoding::percent_decode_str(relative)
                .decode_utf8_lossy()
                .to_string();

            // Windows 路径处理：去掉可能存在的前导反斜杠
            let decoded_normalized = decoded.trim_start_matches('\\').replace('/', "\\");

            // 先尝试作为绝对路径（背景图等外部文件）
            let file_path = if std::path::Path::new(&decoded_normalized).is_absolute()
                && std::path::Path::new(&decoded_normalized).exists()
            {
                std::path::PathBuf::from(&decoded_normalized)
            } else {
                // 否则从 src_dir 解析
                src_dir.join(&decoded_normalized)
            };

            if file_path.exists() {
                let content = std::fs::read(&file_path).unwrap_or_default();
                let mime = mime_type(&decoded_normalized);
                Response::builder()
                    .header("Content-Type", mime)
                    .body(Cow::Owned(content))
                    .unwrap()
            } else {
                Response::builder()
                    .status(404)
                    .body(Cow::Owned(format!("Not found: {}", decoded_normalized).into_bytes()))
                    .unwrap()
            }
        })
        .with_ipc_handler(move |request| {
            let body = request.body();
            if let Ok(msg) = serde_json::from_str::<serde_json::Value>(body) {
                if let Some(cmd) = msg.get("cmd").and_then(|c| c.as_str()) {
                    let response = match cmd {
                        "inject" => {
                            if let Ok(req) = serde_json::from_value::<InjectRequest>(
                                msg.get("data").cloned().unwrap_or_default(),
                            ) {
                                let state = state_clone.lock().unwrap();
                                let result = execute_injection(req, &state.config);
                                serde_json::to_string(&result).unwrap_or_default()
                            } else {
                                r#"{"success":false,"message":"参数错误"}"#.to_string()
                            }
                        }
                        "get_config" => {
                            let state = state_clone.lock().unwrap();
                            serde_json::to_string(&state.config).unwrap_or_default()
                        }
                        "save_config" => {
                            if let Ok(config) = serde_json::from_value::<Config>(
                                msg.get("data").cloned().unwrap_or_default(),
                            ) {
                                let mut state = state_clone.lock().unwrap();
                                state.config = config.clone();
                                match config.save() {
                                    Ok(_) => r#"{"success":true}"#.to_string(),
                                    Err(e) => format!(r#"{{"success":false,"message":"{}"}}"#, e),
                                }
                            } else {
                                r#"{"success":false,"message":"参数错误"}"#.to_string()
                            }
                        }
                        "close" => {
                            std::process::exit(0);
                        }
                        "minimize" => {
                            r#"{"success":true}"#.to_string()
                        }
                        "select_file" => {
                            let filter = msg.get("filter").and_then(|f| f.as_str());
                            match open_file_dialog(filter) {
                                Some(path) => format!(r#"{{"success":true,"path":"{}"}}"#, path.replace('\\', "\\\\")),
                                None => r#"{"success":false,"message":"用户取消"}"#.to_string(),
                            }
                        }
                        "list_processes" => {
                            let processes = process::list_running_processes();
                            serde_json::to_string(&serde_json::json!({
                                "success": true,
                                "processes": processes
                            })).unwrap_or_default()
                        }
                        "read_image_base64" => {
                            if let Some(path) = msg.get("path").and_then(|p| p.as_str()) {
                                match std::fs::read(path) {
                                    Ok(bytes) => {
                                        let mime = if path.to_lowercase().ends_with(".png") {
                                            "image/png"
                                        } else if path.to_lowercase().ends_with(".jpg") || path.to_lowercase().ends_with(".jpeg") {
                                            "image/jpeg"
                                        } else if path.to_lowercase().ends_with(".bmp") {
                                            "image/bmp"
                                        } else if path.to_lowercase().ends_with(".webp") {
                                            "image/webp"
                                        } else {
                                            "image/png"
                                        };
                                        let b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &bytes);
                                        serde_json::to_string(&serde_json::json!({
                                            "success": true,
                                            "data": format!("data:{};base64,{}", mime, b64)
                                        })).unwrap_or_default()
                                    }
                                    Err(e) => serde_json::to_string(&serde_json::json!({
                                        "success": false,
                                        "message": format!("读取图片失败: {}", e)
                                    })).unwrap_or_default(),
                                }
                            } else {
                                r#"{"success":false,"message":"缺少路径参数"}"#.to_string()
                            }
                        }
                        _ => r#"{"success":false,"message":"未知命令"}"#.to_string(),
                    };
                    let js = format!(
                        "window.__callback && window.__callback('{}', {})",
                        cmd, response
                    );
                    pending_clone.lock().unwrap().push(js);
                }
            }
        })
        .with_url("tinject://localhost/index.html")
        .with_background_color((26, 26, 46, 255))
        .build(&window)
        .expect("创建 WebView 失败");

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        // 处理暂存的 JavaScript 回调
        {
            let mut pending = pending_callbacks.lock().unwrap();
            for js in pending.drain(..) {
                let _ = webview.evaluate_script(&js);
            }
        }

        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                *control_flow = ControlFlow::Exit;
            }
            _ => {}
        }
    });
}
