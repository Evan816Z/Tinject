use super::super::{InjectContext, InjectError};
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::System::LibraryLoader::{GetModuleHandleA, GetProcAddress};
use windows::Win32::System::Threading::{WaitForSingleObject, INFINITE};
use windows::core::PCSTR;

/// NtCreateThreadEx 注入
/// 通过ntdll的NtCreateThreadEx创建远程线程，比CreateRemoteThread更底层
/// 可绕过部分基于CreateRemoteThread的安全检测
pub fn inject_with_context(ctx: &InjectContext) -> Result<(), InjectError> {
    unsafe {
        // 动态获取NtCreateThreadEx
        let ntdll = GetModuleHandleA(PCSTR(b"ntdll.dll\0".as_ptr()))
            .map_err(|e| {
                log::error!("GetModuleHandleA(ntdll) 失败: {}", e);
                InjectError::NtCreateThreadExFailed(0)
            })?;
        let nt_create_thread_ex = GetProcAddress(
            ntdll,
            PCSTR(b"NtCreateThreadEx\0".as_ptr()),
        )
        .ok_or_else(|| {
            log::error!("GetProcAddress(NtCreateThreadEx) 失败");
            InjectError::NtCreateThreadExFailed(0)
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
            ctx.process_handle.0 as isize,
            Some(std::mem::transmute(ctx.load_library_addr)),
            ctx.remote_dll_path,
            0,
            0,
            0,
            0,
            std::ptr::null_mut(),
        );

        if status != 0 || thread_handle == 0 {
            log::error!("NtCreateThreadEx 失败: status=0x{:x}, thread_handle={}", status, thread_handle);
            return Err(InjectError::NtCreateThreadExFailed(status as u32));
        }
        log::debug!("NtCreateThreadEx 成功: thread_handle={}", thread_handle);

        log::debug!("等待远程线程完成...");
        let _ = WaitForSingleObject(HANDLE(thread_handle as *mut _), INFINITE);
        log::info!("远程线程执行完成");
        let _ = CloseHandle(HANDLE(thread_handle as *mut _));
    }

    Ok(())
}
