use super::super::InjectError;
use std::path::Path;
use windows::core::PCSTR;
use windows::Win32::Foundation::CloseHandle;
use windows::Win32::System::Memory::{
    VirtualAllocEx, MEM_COMMIT, MEM_RESERVE, PAGE_READWRITE,
};
use windows::Win32::System::LibraryLoader::{GetModuleHandleA, GetProcAddress};
use windows::Win32::System::Threading::{
    OpenProcess, OpenThread, QueueUserAPC,
    THREAD_SUSPEND_RESUME, THREAD_GET_CONTEXT, THREAD_SET_CONTEXT,
    PROCESS_ALL_ACCESS,
};

/// QueueUserAPC 注入
/// 向目标进程所有线程排队APC回调，当线程进入alertable状态时执行LoadLibraryA
/// 不创建新线程，隐蔽性较好，但依赖线程状态，对Java进程可能不稳定
pub fn inject(pid: u32, dll_path: &str) -> Result<(), InjectError> {
    let dll_path = Path::new(dll_path)
        .canonicalize()
        .map_err(|_| InjectError::DllNotFound(dll_path.to_string()))?;
    let dll_str = dll_path.to_str().ok_or(InjectError::DllNotFound(dll_path.display().to_string()))?;

    unsafe {
        let process = OpenProcess(PROCESS_ALL_ACCESS, false, pid)
            .map_err(|_| InjectError::OpenProcessFailed(pid))?;

        // 分配内存并写入DLL路径
        let path_bytes = dll_str.as_bytes();
        let remote_mem = VirtualAllocEx(
            process,
            None,
            path_bytes.len() + 1,
            MEM_COMMIT | MEM_RESERVE,
            PAGE_READWRITE,
        );

        if remote_mem.is_null() {
            let _ = CloseHandle(process);
            return Err(InjectError::VirtualAllocFailed(pid));
        }

        let mut written = 0usize;
        let write_result = windows::Win32::System::Diagnostics::Debug::WriteProcessMemory(
            process,
            remote_mem,
            path_bytes.as_ptr() as *const _,
            path_bytes.len(),
            Some(&mut written),
        );

        if write_result.is_err() {
            let _ = CloseHandle(process);
            return Err(InjectError::WriteProcessMemoryFailed(pid));
        }

        // 获取LoadLibraryA地址
        let kernel32 = GetModuleHandleA(PCSTR(b"kernel32.dll\0".as_ptr()))
            .map_err(|_| InjectError::QueueUserAPCFailed(pid))?;
        let load_library = GetProcAddress(
            kernel32,
            PCSTR(b"LoadLibraryA\0".as_ptr()),
        )
        .ok_or(InjectError::QueueUserAPCFailed(pid))?;

        // 枚举目标进程的所有线程并队列APC
        let thread_ids = get_process_thread_ids(pid);
        let mut queued = false;

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
                }
                let _ = CloseHandle(thread_handle);
            }
        }

        let _ = CloseHandle(process);

        if !queued {
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
