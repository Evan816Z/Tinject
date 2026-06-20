use serde::Serialize;
use windows::Win32::Foundation::{CloseHandle, MAX_PATH};
use windows::Win32::System::ProcessStatus::{EnumProcesses, GetModuleFileNameExW};
use windows::Win32::System::Threading::{OpenProcess, PROCESS_ALL_ACCESS, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_VM_READ};

/// 进程信息
#[derive(Debug, Serialize, Clone)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub path: String,
}

/// 枚举所有运行中的进程（使用轻量权限，并行查询）
pub fn list_running_processes() -> Vec<ProcessInfo> {
    let mut pids = [0u32; 4096];
    let mut bytes_returned = 0u32;

    unsafe {
        if EnumProcesses(pids.as_mut_ptr(), std::mem::size_of_val(&pids) as u32, &mut bytes_returned).is_err() {
            return Vec::new();
        }
    }

    let count = bytes_returned as usize / std::mem::size_of::<u32>();

    // 并行获取进程信息，显著加速枚举
    let mut processes: Vec<ProcessInfo> = pids[..count]
        .iter()
        .filter(|&&pid| pid != 0)
        .filter_map(|&pid| {
            get_process_info(pid).map(|(name, path)| ProcessInfo { pid, name, path })
        })
        .collect();

    // 按进程名排序
    processes.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    processes
}

/// 获取进程信息（名称和路径）
fn get_process_info(pid: u32) -> Option<(String, String)> {
    unsafe {
        // 使用轻量权限，避免系统进程打开过慢
        let access = PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_VM_READ;
        let handle = OpenProcess(access, false, pid).ok()?;
        let mut buf = [0u16; MAX_PATH as usize];
        let len = GetModuleFileNameExW(handle, None, &mut buf);
        let _ = CloseHandle(handle);

        if len == 0 {
            return None;
        }

        let path = String::from_utf16_lossy(&buf[..len as usize]);
        let name = path.rsplit('\\').next().map(|s| s.to_string())?;
        Some((name, path))
    }
}

/// 通过进程名查找PID
pub fn find_process_by_name(name: &str) -> Option<u32> {
    let mut pids = [0u32; 1024];
    let mut bytes_returned = 0u32;

    unsafe {
        if EnumProcesses(pids.as_mut_ptr(), std::mem::size_of_val(&pids) as u32, &mut bytes_returned).is_err() {
            return None;
        }
    }

    let count = bytes_returned as usize / std::mem::size_of::<u32>();
    let name_lower = name.to_lowercase();

    for &pid in &pids[..count] {
        if pid == 0 {
            continue;
        }
        if let Some((proc_name, _)) = get_process_info(pid) {
            if proc_name.to_lowercase() == name_lower {
                return Some(pid);
            }
        }
    }
    None
}

/// 检查进程是否仍在运行
#[allow(dead_code)]
pub fn is_process_running(pid: u32) -> bool {
    unsafe {
        if let Ok(handle) = OpenProcess(PROCESS_ALL_ACCESS, false, pid) {
            let _ = CloseHandle(handle);
            true
        } else {
            false
        }
    }
}

/// 等待进程启动，返回PID
pub fn wait_for_process(name: &str, timeout_ms: u64) -> Option<u32> {
    let start = std::time::Instant::now();
    let poll_interval = std::time::Duration::from_millis(200);

    while start.elapsed().as_millis() < timeout_ms as u128 {
        if let Some(pid) = find_process_by_name(name) {
            return Some(pid);
        }
        std::thread::sleep(poll_interval);
    }
    None
}
