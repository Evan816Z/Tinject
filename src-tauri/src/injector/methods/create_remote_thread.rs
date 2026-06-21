use super::super::InjectError;
use std::path::Path;
use windows::core::PCSTR;
use windows::Win32::Foundation::CloseHandle;

use windows::Win32::System::LibraryLoader::{GetModuleHandleA, GetProcAddress};
use windows::Win32::System::Threading::{
    CreateRemoteThread, OpenProcess, WaitForSingleObject,
    INFINITE, PROCESS_CREATE_THREAD, PROCESS_QUERY_INFORMATION,
    PROCESS_VM_OPERATION, PROCESS_VM_READ, PROCESS_VM_WRITE,
};


/// CreateRemoteThread 注入
/// 经典注入方法：在目标进程中分配内存写入DLL路径，通过CreateRemoteThread调用LoadLibraryA
/// 兼容性最好，但最容易被安全软件检测
pub fn inject(pid: u32, dll_path: &str) -> Result<(), InjectError> {
    let dll_path = Path::new(dll_path)
        .canonicalize()
        .map_err(|_| InjectError::DllNotFound(dll_path.to_string()))?;
    let dll_str = dll_path.to_str().ok_or(InjectError::DllNotFound(dll_path.display().to_string()))?;

    crate::injector::validate_architecture(pid, &dll_path)?;

    unsafe {
        // 打开目标进程，使用最小必要权限以降低被反作弊检测的概率
        let access = PROCESS_CREATE_THREAD
            | PROCESS_QUERY_INFORMATION
            | PROCESS_VM_OPERATION
            | PROCESS_VM_READ
            | PROCESS_VM_WRITE;
        log::debug!("OpenProcess access=0x{:x}", access.0);
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
                InjectError::CreateRemoteThreadFailed(pid)
            })?;
        let load_library = GetProcAddress(
            kernel32,
            PCSTR(b"LoadLibraryA\0".as_ptr()),
        )
        .ok_or_else(|| {
            log::error!("GetProcAddress(LoadLibraryA) 失败");
            InjectError::CreateRemoteThreadFailed(pid)
        })?;
        log::debug!("LoadLibraryA 地址: {:?}", load_library);

        // 创建远程线程执行LoadLibraryA
        log::info!("创建远程线程调用 LoadLibraryA");
        let thread = CreateRemoteThread(
            process,
            None,
            0,
            Some(std::mem::transmute(load_library)),
            Some(remote_mem),
            0,
            None,
        )
        .map_err(|e| {
            log::error!("CreateRemoteThread 失败: {}", e);
            InjectError::CreateRemoteThreadFailed(pid)
        })?;
        log::debug!("远程线程创建成功: {:?}", thread);

        // 等待线程完成
        log::debug!("等待远程线程完成...");
        let _ = WaitForSingleObject(thread, INFINITE);
        log::info!("远程线程执行完成");
        let _ = CloseHandle(thread);
        let _ = CloseHandle(process);
    }

    Ok(())
}
