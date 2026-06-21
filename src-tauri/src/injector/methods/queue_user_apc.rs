use super::super::InjectError;
use std::path::Path;
use windows::core::PCSTR;
use windows::Win32::Foundation::CloseHandle;

use windows::Win32::System::LibraryLoader::{GetModuleHandleA, GetProcAddress};
use windows::Win32::System::Threading::{
    OpenProcess, OpenThread, QueueUserAPC,
    THREAD_SUSPEND_RESUME, THREAD_GET_CONTEXT, THREAD_SET_CONTEXT,
    PROCESS_QUERY_INFORMATION, PROCESS_VM_OPERATION, PROCESS_VM_READ, PROCESS_VM_WRITE,
};

/// QueueUserAPC 注入
/// 向目标进程所有线程排队APC回调，当线程进入alertable状态时执行LoadLibraryA
/// 不创建新线程，隐蔽性较好，但依赖线程状态，对Java进程可能不稳定
pub fn inject(pid: u32, dll_path: &str) -> Result<(), InjectError> {
    let dll_path = Path::new(dll_path)
        .canonicalize()
        .map_err(|_| InjectError::DllNotFound(dll_path.to_string()))?;
    let dll_str = dll_path.to_str().ok_or(InjectError::DllNotFound(dll_path.display().to_string()))?;

    crate::injector::validate_architecture(pid, &dll_path)?;

    unsafe {
        let access = PROCESS_QUERY_INFORMATION
            | PROCESS_VM_OPERATION
            | PROCESS_VM_READ
            | PROCESS_VM_WRITE;
        log::debug!("QueueUserAPC OpenProcess access=0x{:x}", access.0);
        let process = OpenProcess(access, false, pid)
            .map_err(|e| {
                log::error!("OpenProcess 失败: {}", e);
                InjectError::OpenProcessFailed(pid)
            })?;
        log::debug!("OpenProcess 成功: handle={:?}", process);

        // 写入带空终止符的 DLL 路径
        let remote_mem = crate::injector::write_remote_dll_path(process, dll_str)?;

        // 获取LoadLibraryA地址
        let kernel32 = GetModuleHandleA(PCSTR(b"kernel32.dll\0".as_ptr()))
            .map_err(|e| {
                log::error!("GetModuleHandleA(kernel32) 失败: {}", e);
                InjectError::QueueUserAPCFailed(pid)
            })?;
        let load_library = GetProcAddress(
            kernel32,
            PCSTR(b"LoadLibraryA\0".as_ptr()),
        )
        .ok_or_else(|| {
            log::error!("GetProcAddress(LoadLibraryA) 失败");
            InjectError::QueueUserAPCFailed(pid)
        })?;
        log::debug!("LoadLibraryA 地址: {:?}", load_library);

        // 枚举目标进程的所有线程并队列APC
        let thread_ids = get_process_thread_ids(pid);
        log::info!("发现目标进程线程数: {}", thread_ids.len());
        let mut queued = false;
        let mut queued_count = 0;

        for tid in &thread_ids {
            let thread = OpenThread(
                THREAD_SUSPEND_RESUME | THREAD_GET_CONTEXT | THREAD_SET_CONTEXT,
                false,
                *tid,
            );

            if let Ok(thread_handle) = thread {
                let result = QueueUserAPC(
                    Some(std::mem::transmute(load_library)),
                    thread_handle,
                    remote_mem as usize,
                );
                if result != 0 {
                    queued = true;
                    queued_count += 1;
                } else {
                    log::debug!("QueueUserAPC 对线程 {} 排队失败", tid);
                }
                let _ = CloseHandle(thread_handle);
            } else {
                log::debug!("OpenThread({}) 失败", tid);
            }
        }

        let _ = CloseHandle(process);
        log::info!("APC 排队完成: 成功 {} 个线程", queued_count);

        if !queued {
            log::error!("没有成功排队任何 APC");
            return Err(InjectError::QueueUserAPCFailed(pid));
        }
    }

    Ok(())
}

/// 获取进程的所有线程ID
fn get_process_thread_ids(pid: u32) -> Vec<u32> {
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Thread32First, Thread32Next, CREATE_TOOLHELP_SNAPSHOT_FLAGS, THREADENTRY32,
    };

    let mut thread_ids = Vec::new();

    unsafe {
        let snapshot = CreateToolhelp32Snapshot(CREATE_TOOLHELP_SNAPSHOT_FLAGS(0), pid);
        if let Ok(snapshot) = snapshot {
            let mut entry = THREADENTRY32 {
                dwSize: std::mem::size_of::<THREADENTRY32>() as u32,
                ..Default::default()
            };

            if Thread32First(snapshot, &mut entry).is_ok() {
                loop {
                    if entry.th32OwnerProcessID == pid {
                        thread_ids.push(entry.th32ThreadID);
                    }
                    if Thread32Next(snapshot, &mut entry).is_err() {
                        break;
                    }
                }
            }
            let _ = CloseHandle(snapshot);
        }
    }

    thread_ids
}
