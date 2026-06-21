// Tinject - 注入方法模块
pub mod methods;
pub mod process;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// 注入方式枚举
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum InjectionMethod {
    /// 经典注入：在目标进程分配内存写入DLL路径，创建远程线程调用LoadLibraryA
    CreateRemoteThread,
    /// 内核级注入：通过ntdll的NtCreateThreadEx创建线程，比CreateRemoteThread更底层，可绕过部分检测
    NtCreateThreadEx,
    /// APC注入：向目标进程所有线程排队APC回调，线程进入alertable状态时执行LoadLibraryA
    QueueUserAPC,
    /// 手动映射：将DLL的PE映像直接写入目标进程内存，不调用LoadLibrary，绕过模块枚举检测
    ManualMap,
}

impl std::fmt::Display for InjectionMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InjectionMethod::CreateRemoteThread => write!(f, "CreateRemoteThread"),
            InjectionMethod::NtCreateThreadEx => write!(f, "NtCreateThreadEx"),
            InjectionMethod::QueueUserAPC => write!(f, "QueueUserAPC"),
            InjectionMethod::ManualMap => write!(f, "ManualMap"),
        }
    }
}

impl std::str::FromStr for InjectionMethod {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "createremotethread" | "remote_thread" | "0" => Ok(Self::CreateRemoteThread),
            "ntcreatethreadex" | "nt_thread" | "1" => Ok(Self::NtCreateThreadEx),
            "queueuserapc" | "apc" | "2" => Ok(Self::QueueUserAPC),
            "manualmap" | "manual" | "3" => Ok(Self::ManualMap),
            _ => Err(format!("未知注入方式: {}", s)),
        }
    }
}

/// 注入错误类型
#[derive(Error, Debug)]
pub enum InjectError {
    #[error("无法找到目标进程")]
    #[allow(dead_code)]
    ProcessNotFound,
    #[error("无法打开进程: {0}")]
    OpenProcessFailed(u32),
    #[error("内存分配失败: {0}")]
    VirtualAllocFailed(u32),
    #[error("写入内存失败: {0}")]
    WriteProcessMemoryFailed(u32),
    #[error("创建远程线程失败: {0}")]
    CreateRemoteThreadFailed(u32),
    #[error("NtCreateThreadEx 调用失败: {0}")]
    NtCreateThreadExFailed(u32),
    #[error("QueueUserAPC 调用失败: {0}")]
    QueueUserAPCFailed(u32),
    #[error("ManualMap 注入失败: {0}")]
    ManualMapFailed(String),
    #[error("DLL文件不存在: {0}")]
    DllNotFound(String),
    #[error("等待注入超时")]
    #[allow(dead_code)]
    Timeout,
    #[error("注入被拒绝或失败")]
    InjectionDenied,
    #[error("架构不匹配: {0}")]
    ArchitectureMismatch(String),
}

/// 将 DLL 路径以空终止 C 字符串形式写入目标进程内存
pub unsafe fn write_remote_dll_path(
    process: windows::Win32::Foundation::HANDLE,
    dll_path: &str,
) -> Result<*mut std::ffi::c_void, InjectError> {
    use windows::Win32::System::Memory::{VirtualAllocEx, MEM_COMMIT, MEM_RESERVE, PAGE_READWRITE};

    let path_bytes = dll_path.as_bytes();
    let alloc_size = path_bytes.len() + 1;
    log::debug!("在目标进程分配内存: size={} bytes", alloc_size);
    let remote_mem = VirtualAllocEx(process, None, alloc_size, MEM_COMMIT | MEM_RESERVE, PAGE_READWRITE);
    if remote_mem.is_null() {
        log::error!("VirtualAllocEx 失败");
        return Err(InjectError::VirtualAllocFailed(0));
    }
    log::debug!("内存分配成功: remote_mem={:?}", remote_mem);

    let mut written = 0usize;
    log::debug!("写入 DLL 路径: len={}", path_bytes.len());
    let write_result = windows::Win32::System::Diagnostics::Debug::WriteProcessMemory(
        process,
        remote_mem,
        path_bytes.as_ptr() as *const _,
        path_bytes.len(),
        Some(&mut written),
    );

    if write_result.is_err() {
        log::error!("WriteProcessMemory 写入路径失败");
        let _ = windows::Win32::Foundation::CloseHandle(process);
        return Err(InjectError::WriteProcessMemoryFailed(0));
    }
    log::debug!("路径写入完成: written={} bytes", written);

    // 写入空终止符，确保 LoadLibraryA 正确读取
    let null_byte = 0u8;
    let _ = windows::Win32::System::Diagnostics::Debug::WriteProcessMemory(
        process,
        (remote_mem as usize + path_bytes.len()) as *mut _,
        &null_byte as *const _ as *const _,
        1,
        Some(&mut written),
    );
    log::debug!("空终止符写入完成");

    Ok(remote_mem)
}

/// 检查目标进程与 DLL 的架构是否匹配
pub fn validate_architecture(pid: u32, dll_path: &std::path::Path) -> Result<(), InjectError> {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{IsWow64Process, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};

    log::debug!("开始架构匹配检查: pid={}, dll={:?}", pid, dll_path);

    unsafe {
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid)
            .map_err(|e| {
                log::error!("OpenProcess (QUERY_LIMITED) 失败: {}", e);
                InjectError::OpenProcessFailed(pid)
            })?;

        let mut is_wow64 = windows::Win32::Foundation::BOOL(0);
        let _ = IsWow64Process(process, &mut is_wow64);
        let _ = CloseHandle(process);

        // 当前进程一定是 64 位（Tauri 以 x64 构建）
        let target_is_32bit = is_wow64.as_bool();
        log::debug!("目标进程 WOW64: {}", target_is_32bit);

        let dll_data = std::fs::read(dll_path)
            .map_err(|e| {
                log::error!("读取 DLL 失败: {}", e);
                InjectError::DllNotFound(format!("读取DLL失败: {}", e))
            })?;
        if dll_data.len() < 2 || &dll_data[0..2] != b"MZ" {
            log::error!("DLL 不是有效的 PE 文件");
            return Err(InjectError::DllNotFound("无效的PE文件".to_string()));
        }

        let e_lfanew = *(dll_data.as_ptr().add(0x3C) as *const u32) as usize;
        if e_lfanew + 4 >= dll_data.len() {
            log::error!("PE 头偏移无效");
            return Err(InjectError::DllNotFound("无效的PE文件".to_string()));
        }

        let nt_signature = &dll_data[e_lfanew..e_lfanew + 4];
        if nt_signature != b"PE\0\0" {
            log::error!("PE 签名无效");
            return Err(InjectError::DllNotFound("无效的PE文件".to_string()));
        }

        let machine = *(dll_data.as_ptr().add(e_lfanew + 4) as *const u16);
        let dll_is_32bit = machine == 0x14c; // IMAGE_FILE_MACHINE_I386
        let dll_is_64bit = machine == 0x8664; // IMAGE_FILE_MACHINE_AMD64
        log::debug!("DLL 架构: machine=0x{:x}, is_32bit={}, is_64bit={}", machine, dll_is_32bit, dll_is_64bit);

        if target_is_32bit && !dll_is_32bit {
            log::error!("架构不匹配: 目标进程 32 位, DLL 非 32 位");
            return Err(InjectError::ArchitectureMismatch(
                "目标进程为 32 位，但 DLL 为 64 位".to_string(),
            ));
        }
        if !target_is_32bit && !dll_is_64bit {
            log::error!("架构不匹配: 目标进程 64 位, DLL 非 64 位");
            return Err(InjectError::ArchitectureMismatch(
                "目标进程为 64 位，但 DLL 不是 64 位".to_string(),
            ));
        }
    }

    log::info!("架构匹配检查通过");
    Ok(())
}

/// 注入结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InjectResult {
    pub success: bool,
    pub method: String,
    pub dll_path: String,
    pub message: String,
}

/// 执行DLL注入
pub fn inject_dll(
    pid: u32,
    dll_path: &str,
    method: InjectionMethod,
) -> Result<InjectResult, InjectError> {
    log::info!("准备注入 DLL: pid={}, path={}, method={}", pid, dll_path, method);

    if !std::path::Path::new(dll_path).exists() {
        log::error!("DLL 文件不存在: {}", dll_path);
        return Err(InjectError::DllNotFound(dll_path.to_string()));
    }

    match method {
        InjectionMethod::CreateRemoteThread => {
            methods::create_remote_thread::inject(pid, dll_path)?;
        }
        InjectionMethod::NtCreateThreadEx => {
            methods::nt_create_thread::inject(pid, dll_path)?;
        }
        InjectionMethod::QueueUserAPC => {
            methods::queue_user_apc::inject(pid, dll_path)?;
        }
        InjectionMethod::ManualMap => {
            methods::manual_map::inject(pid, dll_path)?;
        }
    }

    log::info!("DLL 注入成功: pid={}, path={}, method={}", pid, dll_path, method);
    Ok(InjectResult {
        success: true,
        method: method.to_string(),
        dll_path: dll_path.to_string(),
        message: format!("DLL注入成功 [{}]", method),
    })
}

/// 尝试多种方式注入DLL（自动降级）
pub fn inject_dll_auto(
    pid: u32,
    dll_path: &str,
) -> Result<InjectResult, InjectError> {
    log::info!("开始自动降级注入: pid={}, path={}", pid, dll_path);
    let methods = [
        InjectionMethod::CreateRemoteThread,
        InjectionMethod::NtCreateThreadEx,
        InjectionMethod::QueueUserAPC,
        InjectionMethod::ManualMap,
    ];

    let mut last_err = InjectError::InjectionDenied;
    for method in &methods {
        log::info!("尝试注入方式: {}", method);
        match inject_dll(pid, dll_path, *method) {
            Ok(result) => {
                log::info!("自动降级注入成功，使用方式: {}", method);
                return Ok(result);
            }
            Err(e) => {
                log::warn!("注入方式 {} 失败: {}", method, e);
                last_err = e;
            }
        }
    }
    log::error!("所有注入方式均失败: {}", last_err);
    Err(last_err)
}
