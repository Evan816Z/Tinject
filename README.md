# Tinject

Windows 平台轻量级 DLL 注入工具，采用 Rust + WebView2 构建，拥有现代液态玻璃 UI。

## 功能特性

### 注入引擎
- **4 种注入方式**：CreateRemoteThread / NtCreateThreadEx / QueueUserAPC / ManualMap
- **自动降级**：首选方式失败时自动尝试其他方式
- **批量注入**：支持多 DLL、多进程同时注入，可自定义间隔
- **进程等待**：等待目标进程启动后自动注入
- **架构检查**：自动匹配 x86/x64，防止崩溃

### 进程管理
- **进程选择器**：枚举系统运行进程，支持搜索和批量选择
- **实时状态**：目标进程运行状态每 1.5 秒自动刷新
- **手动添加**：直接输入进程名添加目标

### 界面设计
- **液态玻璃风格**：磨砂透明 + 高光反射效果
- **三套主题**：现代（玻璃）/ 深色 / 浅色
- **自定义外观**：透明度、模糊强度、主色调、背景图片
- **无框窗口**：自定义标题栏，支持最小化和关闭

### 便携与持久化
- **单文件运行**：前端资源编译进二进制，无需外部文件
- **全局记忆**：DLL 列表、进程选择、注入方式自动保存
- **配置存储**：保存至 `%APPDATA%\Tinject\config.json`
- **详细日志**：记录每个关键步骤，保存至 `%APPDATA%\Tinject\logs\`

### 命令行模式
- **静默注入**：无需 GUI，后台完成注入
- **全自动模式**：等待进程启动后自动注入

## 系统要求

- Windows 10/11 (64-bit)
- 管理员权限（注入需要）

## 快速开始

从 [Releases](https://github.com/Evan816Z/Tinject/releases) 下载 `tinject.exe`，以管理员身份运行即可。

## 使用说明

### GUI 模式

1. 以管理员身份运行 `tinject.exe`
2. 添加目标进程（手动输入或从运行中选择）
3. 添加 DLL 文件（支持拖拽排序）
4. 选择注入方式
5. 点击"开始注入"

### 命令行模式

```bash
# 静默注入
tinject.exe silent -d "C:\path\to\mod.dll" -p javaw.exe -m auto --delay 500

# 全自动模式（等待进程启动）
tinject.exe auto -d "C:\path\to\mod.dll" -p javaw.exe -t 30000
```

参数说明：
| 参数 | 说明 | 默认值 |
|------|------|--------|
| `-d, --dlls` | DLL 文件路径（支持多个） | - |
| `-p, --process` | 目标进程名称 | javaw.exe |
| `-m, --method` | 注入方式：auto/remote_thread/nt_thread/apc/manual | auto |
| `--delay` | 批量注入间隔（毫秒） | 500 |
| `-t, --timeout` | 等待进程超时（毫秒） | 30000 |

## 项目结构

```
Tinject/
├── src/                          # 前端源码（编译时嵌入二进制）
│   ├── index.html
│   ├── css/style.css
│   └── js/
│       ├── app.js                # 前端交互逻辑
│       └── liquid-glass.js       # 液态玻璃效果
├── src-tauri/                    # Rust 后端
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs               # 程序入口
│       ├── cli.rs                # 命令行模块
│       ├── config.rs             # 配置管理
│       ├── logger.rs             # 文件日志
│       └── injector/
│           ├── mod.rs            # 注入器主模块
│           ├── process.rs        # 进程枚举与管理
│           └── methods/          # 注入方法实现
│               ├── create_remote_thread.rs
│               ├── nt_create_thread.rs
│               ├── queue_user_apc.rs
│               └── manual_map.rs
└── README.md
```

## 编译构建

```bash
cd src-tauri
cargo build --release
```

输出：`src-tauri/target/release/tinject.exe`

## 开源协议

MIT License
