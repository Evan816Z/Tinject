use super::super::InjectError;
use std::path::Path;
use windows::core::PCSTR;
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::System::Memory::{
    VirtualAllocEx, MEM_COMMIT, MEM_RESERVE, PAGE_READWRITE,
};
use windows::Win32::System::LibraryLoader::{GetModuleHandleA, GetProcAddress};
use windows::Win32::System::Threading::{
    OpenProcess, WaitForSingleObject, PROCESS_ALL_ACCESS, INFINITE,
};

/// NtCreateThreadEx 注入
/// 通过ntdll的NtCreateThreadEx创建远程线程，比CreateRemoteThread更底层
/// 可绕过部分基于CreateRemoteThread的安全检测
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
            .map_err(|_| InjectError::NtCreateThreadExFailed(pid))?;
        let load_library = GetProcAddress(
            kernel32,
            PCSTR(b"LoadLibraryA\0".as_ptr()),
        )
        .ok_or(InjectError::NtCreateThreadExFailed(pid))?;

        // 动态获取NtCreateThreadEx
        let ntdll = GetModuleHandleA(PCSTR(b"ntdll.dll\0".as_ptr()))
            .map_err(|_| InjectError::NtCreateThreadExFailed(pid))?;
        let nt_create_thread_ex = GetProcAddress(
            ntdll,
            PCSTR(b"NtCreateThreadEx\0".as_ptr()),
        )
        .ok_or(InjectError::NtCreateThreadExFailed(pid))?;

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
            let _ = CloseHandle(process);
            return Err(InjectError::NtCreateThreadExFailed(status as u32));
        }

        let _ = WaitForSingleObject(HANDLE(thread_handle as *mut _), INFINITE);
        let _ = CloseHandle(HANDLE(thread_handle as *mut _));
        let _ = CloseHandle(process);
    }

    Ok(())
}
