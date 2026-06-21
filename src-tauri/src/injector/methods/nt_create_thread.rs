use super::super::InjectError;
use std::path::Path;
use windows::core::PCSTR;
use windows::Win32::Foundation::{CloseHandle, HANDLE};

use windows::Win32::System::LibraryLoader::{GetModuleHandleA, GetProcAddress};
use windows::Win32::System::Threading::{
    OpenProcess, WaitForSingleObject, INFINITE, PROCESS_CREATE_THREAD,
    PROCESS_QUERY_INFORMATION, PROCESS_VM_OPERATION, PROCESS_VM_READ, PROCESS_VM_WRITE,
};

/// NtCreateThreadEx 注入
/// 通过ntdll的NtCreateThreadEx创建远程线程，比CreateRemoteThread更底层
/// 可绕过部分基于CreateRemoteThread的安全检测
pub fn inject(pid: u32, dll_path: &str) -> Result<(), InjectError> {
    let dll_path = Path::new(dll_path)
        .canonicalize()
        .map_err(|_| InjectError::DllNotFound(dll_path.to_string()))?;
    let dll_str = dll_path.to_str().ok_or(InjectError::DllNotFound(dll_path.display().to_string()))?;

    crate::injector::validate_architecture(pid, &dll_path)?;

    unsafe {
        let access = PROCESS_CREATE_THREAD
            | PROCESS_QUERY_INFORMATION
            | PROCESS_VM_OPERATION
            | PROCESS_VM_READ
            | PROCESS_VM_WRITE;
        log::debug!("NtCreateThreadEx OpenProcess access=0x{:x}", access.0);
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
                InjectError::NtCreateThreadExFailed(pid)
            })?;
        let load_library = GetProcAddress(
            kernel32,
            PCSTR(b"LoadLibraryA\0".as_ptr()),
        )
        .ok_or_else(|| {
            log::error!("GetProcAddress(LoadLibraryA) 失败");
            InjectError::NtCreateThreadExFailed(pid)
        })?;
        log::debug!("LoadLibraryA 地址: {:?}", load_library);

        // 动态获取NtCreateThreadEx
        let ntdll = GetModuleHandleA(PCSTR(b"ntdll.dll\0".as_ptr()))
            .map_err(|e| {
                log::error!("GetModuleHandleA(ntdll) 失败: {}", e);
                InjectError::NtCreateThreadExFailed(pid)
            })?;
        let nt_create_thread_ex = GetProcAddress(
            ntdll,
            PCSTR(b"NtCreateThreadEx\0".as_ptr()),
        )
        .ok_or_else(|| {
            log::error!("GetProcAddress(NtCreateThreadEx) 失败");
            InjectError::NtCreateThreadExFailed(pid)
        })?;
        log::debug!("NtCreateThreadEx 地址: {:?}", nt_create_thread_ex);

        // 定义NtCreateThreadEx函数指针类型
        type NtCreateThreadExFn = unsafe extern "system" fn(
            *mut isize,
            u32,
            *mut std::ffi::c_void,
            isize,
            Option<unsafe extern "system" fn(*mut std::ffi::c_void) -> u32>,
            *mut std::ffi::c_void,
            u32,
            usize,
            usize,
            usize,
            *mut std::ffi::c_void,
        ) -> i32;

        let mut thread_handle: isize = 0;
        let nt_create: NtCreateThreadExFn = std::mem::transmute(nt_create_thread_ex);

        log::info!("调用 NtCreateThreadEx 创建远程线程");
        let status = nt_create(
            &mut thread_handle,
            0x1FFFFF, // THREAD_ALL_ACCESS
            std::ptr::null_mut(),
            process.0 as isize,
            Some(std::mem::transmute(load_library)),
            remote_mem,
            0,
            0,
            0,
            0,
            std::ptr::null_mut(),
        );

        if status != 0 || thread_handle == 0 {
            log::error!("NtCreateThreadEx 失败: status=0x{:x}, thread_handle={}", status, thread_handle);
            let _ = CloseHandle(process);
            return Err(InjectError::NtCreateThreadExFailed(status as u32));
        }
        log::debug!("NtCreateThreadEx 成功: thread_handle={}", thread_handle);

        log::debug!("等待远程线程完成...");
        let _ = WaitForSingleObject(HANDLE(thread_handle as *mut _), INFINITE);
        log::info!("远程线程执行完成");
        let _ = CloseHandle(HANDLE(thread_handle as *mut _));
        let _ = CloseHandle(process);
    }

    Ok(())
}
