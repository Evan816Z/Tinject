use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "tinject")]
#[command(version = "1.0.0")]
#[command(about = "Tinject - Minecraft Forge DLL Injector")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// 静默注入模式 - 使用默认配置注入，不显示界面
    Silent {
        /// DLL文件路径（可多个）
        #[arg(short, long, num_args = 1..)]
        dlls: Vec<String>,

        /// 目标进程名称
        #[arg(short, long, default_value = "javaw.exe")]
        process: String,

        /// 注入方式: auto, remote_thread, nt_thread, apc, manual
        #[arg(short, long, default_value = "auto")]
        method: String,

        /// 批量注入间隔（毫秒）
        #[arg(short, long, default_value = "500")]
        delay: u64,
    },

    /// 后台全自动注入模式 - 等待进程启动后自动注入
    Auto {
        /// DLL文件路径（可多个）
        #[arg(short, long, num_args = 1..)]
        dlls: Vec<String>,

        /// 目标进程名称
        #[arg(short, long, default_value = "javaw.exe")]
        process: String,

        /// 注入方式
        #[arg(short, long, default_value = "auto")]
        method: String,

        /// 批量注入间隔（毫秒）
        #[arg(short, long, default_value = "500")]
        delay: u64,

        /// 等待超时（毫秒）
        #[arg(short, long, default_value = "30000")]
        timeout: u64,
    },
}

pub fn execute_cli(cli: Cli) {
    match cli.command {
        Some(Commands::Silent { dlls, process, method, delay }) => {
            println!("[静默模式] 开始注入...");
            execute_injection(dlls, process, method, delay, 0);
        }
        Some(Commands::Auto { dlls, process, method, delay, timeout }) => {
            println!("[全自动模式] 等待进程启动...");
            execute_injection(dlls, process, method, delay, timeout);
        }
        None => {
            // 无子命令，启动GUI模式
            println!("启动GUI模式...");
        }
    }
}

fn execute_injection(dlls: Vec<String>, process_name: String, method: String, delay: u64, timeout: u64) {
    use crate::injector::{self, InjectionMethod};
    use crate::injector::process;

    // 等待进程
    let pid = if timeout > 0 {
        match process::wait_for_process(&process_name, timeout) {
            Some(pid) => pid,
            None => {
                eprintln!("错误: 无法找到进程 {} (超时)", process_name);
                std::process::exit(1);
            }
        }
    } else {
        match process::find_process_by_name(&process_name) {
            Some(pid) => pid,
            None => {
                eprintln!("错误: 无法找到进程 {}", process_name);
                std::process::exit(1);
            }
        }
    };

    println!("找到目标进程: {} (PID: {})", process_name, pid);

    let method_enum: Result<InjectionMethod, _> = method.parse();
    let method_enum = method_enum.unwrap_or(InjectionMethod::CreateRemoteThread);

    // 批量注入
    for (index, dll_path) in dlls.iter().enumerate() {
        print!("注入 {} ... ", dll_path);

        let result = if method == "auto" {
            injector::inject_dll_auto(pid, dll_path)
        } else {
            injector::inject_dll(pid, dll_path, method_enum)
        };

        match result {
            Ok(r) => println!("成功 [{}]", r.method),
            Err(e) => {
                println!("失败: {}", e);
            }
        }

        // 批量间隔
        if index < dlls.len() - 1 && delay > 0 {
            std::thread::sleep(std::time::Duration::from_millis(delay));
        }
    }

    println!("注入完成");
}
