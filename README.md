# Tinject

Tinject 是一款 Windows 平台桌面应用程序，专为 Minecraft Java 版 Forge 客户端设计的 DLL 注入工具。采用 Tauri 框架开发，具有亚克力/液态玻璃磨砂透明界面，支持多种 DLL 注入方式，资源占用极低。

## 功能特性

### 核心注入功能
- **多种注入方式**：内置 4 种 DLL 注入方案
  - CreateRemoteThread - 经典远程线程注入
  - NtCreateThreadEx - NTDLL 底层注入，绕过部分检测
  - QueueUserAPC - APC 队列注入
  - ManualMap - 手动映射注入，不依赖 LoadLibrary
- **批量注入**：支持单次同时注入多个 DLL 文件
- **自定义延迟**：批量注入支持自定义间隔（毫秒）
- **自动降级**：注入失败时自动尝试备用方案
- **进程等待**：支持等待目标进程启动后自动注入

### 界面设计
- **亚克力/液态玻璃风格**：磨砂透明视觉效果
- **多主题支持**：液态玻璃、深色、浅色三种主题
- **自定义外观**：透明度、模糊强度、主色调均可调节
- **轻量化设计**：界面流畅，资源占用极低

### 命令行模式
- **静默注入模式**：无需打开界面，后台完成注入
- **全自动模式**：等待进程启动后自动注入

## 技术架构

- **后端**：Rust + Tauri 2.0
- **前端**：HTML5 + CSS3 + JavaScript（WebView）
- **注入引擎**：Windows API（windows crate）
- **构建工具**：Cargo + Tauri CLI

## 系统要求

- Windows 10/11 (64-bit)
- 管理员权限（注入需要）
- 4GB+ RAM

## 安装部署

### 开发环境搭建

1. **安装 Rust**
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

2. **安装 Tauri 依赖**
```bash
# Windows
cargo install tauri-cli --version "^2.0.0"
```

3. **克隆项目**
```bash
git clone https://github.com/yourusername/Tinject.git
cd Tinject
```

### 编译构建

```bash
# 进入后端目录
cd src-tauri

# 开发模式运行
cargo tauri dev

# 构建发布版本
cargo tauri build
```

构建完成后，可执行文件位于：
- `src-tauri/target/release/tinject.exe`

## 使用说明

### GUI 模式

1. 以管理员身份运行 `tinject.exe`
2. 在"注入"页面：
   - 设置目标进程（默认 `javaw.exe`）
   - 点击"添加 DLL"选择要注入的文件
   - 选择注入方式（推荐"自动选择"）
   - 设置批量注入间隔
   - 点击"开始注入"

### 命令行模式

#### 静默注入
```bash
tinject.exe silent --dlls "C:\path\to\mod1.dll" "C:\path\to\mod2.dll" --process javaw.exe --method auto --delay 500
```

参数说明：
- `--dlls, -d`：DLL 文件路径（支持多个）
- `--process, -p`：目标进程名称（默认 `javaw.exe`）
- `--method, -m`：注入方式（auto/remote_thread/nt_thread/apc/manual）
- `--delay`：批量注入间隔毫秒数（默认 500）

#### 全自动模式
```bash
tinject.exe auto --dlls "C:\path\to\mod.dll" --process javaw.exe --timeout 30000
```

额外参数：
- `--timeout, -t`：等待进程启动超时时间（毫秒，默认 30000）

### 配置说明

配置文件位于程序同目录下的 `config.json`，包含以下配置项：

```json
{
  "injection": {
    "target_process": "javaw.exe",
    "method": "auto",
    "dll_paths": [],
    "batch_delay_ms": 500,
    "process_timeout_ms": 30000,
    "auto_fallback": true
  },
  "ui": {
    "theme": "glass",
    "opacity": 0.85,
    "blur_intensity": 20,
    "accent_color": "#00d4ff",
    "window_width": 900,
    "window_height": 620
  }
}
```

## 项目结构

```
Tinject/
├── src/                          # 前端源码
│   ├── index.html               # 主页面
│   ├── css/
│   │   └── style.css            # 样式文件
│   └── js/
│       └── app.js               # 前端逻辑
├── src-tauri/                   # Rust 后端
│   ├── Cargo.toml              # Rust 依赖配置
│   ├── tauri.conf.json         # Tauri 配置
│   ├── build.rs                # 构建脚本
│   └── src/
│       ├── main.rs             # 程序入口
│       ├── cli.rs              # 命令行模块
│       ├── commands.rs         # Tauri 命令接口
│       ├── config.rs           # 配置管理
│       └── injector/           # 注入引擎
│           ├── mod.rs          # 注入器主模块
│           ├── process.rs      # 进程管理
│           └── methods/        # 注入方法实现
│               ├── mod.rs
│               ├── create_remote_thread.rs
│               ├── nt_create_thread.rs
│               ├── queue_user_apc.rs
│               └── manual_map.rs
└── README.md                   # 项目文档
```

## 开发指南

### 代码注释规范

所有 Rust 代码使用中文注释，遵循 rustdoc 格式：

```rust
/// 函数功能说明
/// 
/// # 参数
/// * `param` - 参数说明
/// 
/// # 返回
/// 返回值说明
```

### 添加新的注入方式

1. 在 `src-tauri/src/injector/methods/` 创建新文件
2. 实现 `inject(pid: u32, dll_path: &str) -> Result<(), InjectError>` 函数
3. 在 `injector/mod.rs` 中添加新的枚举值和注入逻辑

## 安全声明

本工具仅供学习和研究使用。使用 DLL 注入技术可能违反某些软件的使用条款，请在使用前确认合规性。作者不对因使用本工具产生的任何问题负责。

## 开源协议

MIT License

## 贡献

欢迎提交 Issue 和 Pull Request。

## 致谢

- [Tauri](https://tauri.app/) - 轻量级跨平台应用框架
- [windows-rs](https://github.com/microsoft/windows-rs) - Windows API Rust 绑定

---

**免责声明**：本工具仅用于合法的技术研究和教育目的。请勿将本工具用于任何违法或违反软件使用条款的行为。
