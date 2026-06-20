use crate::config::Config;
use crate::injector::{self, InjectionMethod};
use crate::injector::process;
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tauri::{Manager, State};

/// 应用状态
pub struct AppState {
    pub config: Mutex<Config>,
}

/// 注入请求
#[derive(Debug, Deserialize)]
pub struct InjectRequest {
    pub dll_paths: Vec<String>,
    pub method: String,
    pub batch_delay_ms: u64,
}

/// 注入响应
#[derive(Debug, Serialize)]
pub struct InjectResponse {
    pub success: bool,
    pub results: Vec<InjectResult>,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct InjectResult {
    pub dll_path: String,
    pub success: bool,
    pub method: String,
    pub message: String,
}

/// 获取进程列表
#[tauri::command]
pub fn get_processes() -> Vec<String> {
    use windows::Win32::System::ProcessStatus::{EnumProcesses, GetModuleFileNameExW};
    use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ};
    use windows::Win32::Foundation::{CloseHandle, HANDLE, MAX_PATH};

    let mut pids = [0u32; 1024];
    let mut bytes_returned = 0u32;

    unsafe {
        if EnumProcesses(pids.as_mut_ptr(), std::mem::size_of_val(&pids) as u32, &mut bytes_returned).is_err() {
            return Vec::new();
        }
    }

    let count = bytes_returned as usize / std::mem::size_of::<u32>();
    let mut processes = Vec::new();

    for &pid in &pids[..count] {
        if pid == 0 {
            continue;
        }
        unsafe {
            if let Ok(handle) = OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, false, pid) {
                let mut buf = [0u16; MAX_PATH as usize];
                let len = GetModuleFileNameExW(HANDLE(handle.raw() as _), None, &mut buf);
                let _ = CloseHandle(handle);

                if len > 0 {
                    let path = String::from_utf16_lossy(&buf[..len as usize]);
                    if let Some(name) = path.rsplit('\\').next() {
                        processes.push(name.to_string());
                    }
                }
            }
        }
    }

    processes.sort();
    processes.dedup();
    processes
}

/// 执行DLL注入
#[tauri::command]
pub async fn inject_dlls(
    state: State<'_, AppState>,
    request: InjectRequest,
) -> Result<InjectResponse, String> {
    let config = state.config.lock().map_err(|e| e.to_string())?;
    let target_process = config.injection.target_process.clone();
    let timeout_ms = config.injection.process_timeout_ms;
    let auto_fallback = config.injection.auto_fallback;
    drop(config);

    // 等待目标进程启动
    let pid = process::wait_for_process(&target_process, timeout_ms)
        .ok_or_else(|| format!("无法找到进程: {}", target_process))?;

    let mut results = Vec::new();
    let mut all_success = true;

    // 解析注入方式
    let method: Result<InjectionMethod, _> = request.method.parse();
    let method = method.unwrap_or(InjectionMethod::CreateRemoteThread);

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
            tokio::time::sleep(tokio::time::Duration::from_millis(request.batch_delay_ms)).await;
        }
    }

    Ok(InjectResponse {
        success: all_success,
        results,
        message: if all_success {
            "所有DLL注入成功".to_string()
        } else {
            "部分DLL注入失败".to_string()
        },
    })
}

/// 获取当前配置
#[tauri::command]
pub fn get_config(state: State<'_, AppState>) -> Result<Config, String> {
    let config = state.config.lock().map_err(|e| e.to_string())?;
    Ok(config.clone())
}

/// 更新配置
#[tauri::command]
pub fn update_config(state: State<'_, AppState>, config: Config) -> Result<(), String> {
    let mut current = state.config.lock().map_err(|e| e.to_string())?;
    *current = config.clone();
    drop(current);
    config.save()
}

/// 关闭应用
#[tauri::command]
pub fn close_app(app: tauri::AppHandle) {
    app.exit(0);
}

/// 最小化窗口
#[tauri::command]
pub fn minimize_window(app: tauri::AppHandle) {
    if let Some(window) = app.get_window("main") {
        let _ = window.minimize();
    }
}
