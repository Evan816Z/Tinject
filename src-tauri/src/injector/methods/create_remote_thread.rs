use super::super::{InjectContext, InjectError};
use windows::Win32::Foundation::CloseHandle;
use windows::Win32::System::Threading::{
    CreateRemoteThread, WaitForSingleObject, INFINITE,
};

/// CreateRemoteThread 注入
/// 经典注入方法：在目标进程中分配内存写入DLL路径，通过CreateRemoteThread调用LoadLibraryA
/// 兼容性最好，但最容易被安全软件检测
pub fn inject_with_context(ctx: &InjectContext) -> Result<(), InjectError> {
    unsafe {
        log::info!("创建远程线程调用 LoadLibraryA");
        let thread = CreateRemoteThread(
            ctx.process_handle,
            None,
            0,
            Some(std::mem::transmute(ctx.load_library_addr)),
            Some(ctx.remote_dll_path),
            0,
            None,
        )
        .map_err(|e| {
            log::error!("CreateRemoteThread 失败: {}", e);
            InjectError::CreateRemoteThreadFailed(0)
        })?;
        log::debug!("远程线程创建成功: {:?}", thread);

        log::debug!("等待远程线程完成...");
        let _ = WaitForSingleObject(thread, INFINITE);
        log::info!("远程线程执行完成");
        let _ = CloseHandle(thread);
    }

    Ok(())
}
