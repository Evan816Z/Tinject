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
    if !std::path::Path::new(dll_path).exists() {
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
    let methods = [
        InjectionMethod::CreateRemoteThread,
        InjectionMethod::NtCreateThreadEx,
        InjectionMethod::QueueUserAPC,
        InjectionMethod::ManualMap,
    ];

    let mut last_err = InjectError::InjectionDenied;
    for method in &methods {
        match inject_dll(pid, dll_path, *method) {
            Ok(result) => return Ok(result),
            Err(e) => {
                log::warn!("注入方式 {} 失败: {}", method, e);
                last_err = e;
            }
        }
    }
    Err(last_err)
}
