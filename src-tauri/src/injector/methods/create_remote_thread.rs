use super::super::InjectError;
use std::path::Path;
use windows::core::PCSTR;
use windows::Win32::Foundation::CloseHandle;
use windows::Win32::System::Memory::{
    VirtualAllocEx, MEM_COMMIT, MEM_RESERVE, PAGE_READWRITE,
};
use windows::Win32::System::LibraryLoader::{GetModuleHandleA, GetProcAddress};
use windows::Win32::System::Threading::{
    CreateRemoteThread, OpenProcess, WaitForSingleObject,
    PROCESS_ALL_ACCESS, INFINITE,
};


/// CreateRemoteThread 注入
/// 经典注入方法：在目标进程中分配内存写入DLL路径，通过CreateRemoteThread调用LoadLibraryA
/// 兼容性最好，但最容易被安全软件检测
pub fn inject(pid: u32, dll_path: &str) -> Result<(), InjectError> {
    let dll_path = Path::new(dll_path)
        .canonicalize()
        .map_err(|_| InjectError::DllNotFound(dll_path.to_string()))?;
    let dll_str = dll_path.to_str().ok_or(InjectError::DllNotFound(dll_path.display().to_string()))?;

    unsafe {
        // 打开目标进程
        let process = OpenProcess(PROCESS_ALL_ACCESS, false, pid)
            .map_err(|_| InjectError::OpenProcessFailed(pid))?;

        // 在目标进程中分配内存
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

        // 写入DLL路径到目标进程内存
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
            .map_err(|_| InjectError::CreateRemoteThreadFailed(pid))?;
        let load_library = GetProcAddress(
            kernel32,
            PCSTR(b"LoadLibraryA\0".as_ptr()),
        )
        .ok_or(InjectError::CreateRemoteThreadFailed(pid))?;

        // 创建远程线程执行LoadLibraryA
        let thread = CreateRemoteThread(
            process,
            None,
            0,
            Some(std::mem::transmute(load_library)),
            Some(remote_mem),
            0,
            None,
        )
        .map_err(|_| InjectError::CreateRemoteThreadFailed(pid))?;

        // 等待线程完成
        let _ = WaitForSingleObject(thread, INFINITE);
        let _ = CloseHandle(thread);
        let _ = CloseHandle(process);
    }

    Ok(())
}
