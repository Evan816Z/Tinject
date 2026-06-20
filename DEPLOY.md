# Tinject 部署教程

## 环境准备

### 1. 安装 Rust

```bash
# Windows PowerShell
Invoke-WebRequest -Uri https://win.rustup.rs/x86_64 -OutFile rustup-init.exe
.\rustup-init.exe -y

# 验证安装
rustc --version
cargo --version
```

### 2. 安装 Visual Studio Build Tools

下载并安装 [Visual Studio Build Tools 2022](https://visualstudio.microsoft.com/visual-cpp-build-tools/)

安装时选择：
- "使用 C++ 的桌面开发"
- Windows 10/11 SDK

### 3. 安装 Tauri CLI

```bash
cargo install tauri-cli --version "^2.0.0"
```

### 4. 安装 WebView2

Windows 10/11 通常已预装 WebView2。如未安装，从[微软官网](https://developer.microsoft.com/en-us/microsoft-edge/webview2/)下载安装。

## 开发模式

```bash
# 克隆项目
git clone https://github.com/yourusername/Tinject.git
cd Tinject

# 进入后端目录
cd src-tauri

# 运行开发版本
cargo tauri dev
```

开发模式支持热重载，修改前端代码后会自动刷新。

## 生产构建

### 构建发布版本

```bash
cd src-tauri
cargo tauri build
```

构建产物：
- 可执行文件：`src-tauri/target/release/tinject.exe`
- 安装包：`src-tauri/target/release/bundle/`

### 手动编译（不使用 Tauri CLI）

```bash
cd src-tauri
cargo build --release
```

## 分发部署

### 方式一：直接分发可执行文件

将以下文件打包分发：
- `tinject.exe`（主程序）
- `WebView2Loader.dll`（如目标系统未安装 WebView2）

### 方式二：创建安装包

使用 Tauri 生成的 MSI/NSIS 安装包：
- MSI：`src-tauri/target/release/bundle/msi/Tinject-1.0.0-x64.msi`
- NSIS：`src-tauri/target/release/bundle/nsis/Tinject-1.0.0-x64.exe`

### 方式三：绿色版部署

创建便携版：
1. 复制 `tinject.exe` 到目标目录
2. 创建 `config.json` 配置文件（可选）
3. 打包为 ZIP 分发

## 配置部署

### 默认配置文件

在程序同目录创建 `config.json`：

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

### 批量部署配置

在企业环境中，可以预置配置文件：

```powershell
# 创建配置文件
$config = @{
    injection = @{
        target_process = "javaw.exe"
        method = "auto"
        batch_delay_ms = 500
        process_timeout_ms = 30000
        auto_fallback = $true
    }
    ui = @{
        theme = "glass"
        opacity = 0.85
        blur_intensity = 20
        accent_color = "#00d4ff"
    }
} | ConvertTo-Json -Depth 10

$config | Out-File -FilePath "C:\Program Files\Tinject\config.json" -Encoding UTF8
```

## 命令行自动化部署

### 静默安装脚本

```powershell
# install.ps1
$installerUrl = "https://github.com/yourusername/Tinject/releases/download/v1.0.0/Tinject-1.0.0-x64.msi"
$installerPath = "$env:TEMP\Tinject.msi"

# 下载安装包
Invoke-WebRequest -Uri $installerUrl -OutFile $installerPath

# 静默安装
Start-Process msiexec.exe -ArgumentList "/i `"$installerPath`" /qn /norestart" -Wait

# 清理
Remove-Item $installerPath
```

### 静默注入脚本

```batch
@echo off
:: inject.bat
set TINJECT_PATH=C:\Program Files\Tinject\tinject.exe
set DLL_PATH=C:\Mods\visual_mod.dll

"%TINJECT_PATH%" silent --dlls "%DLL_PATH%" --process javaw.exe --method auto
```

## 性能优化

### 编译优化

`Cargo.toml` 已配置发布优化：

```toml
[profile.release]
panic = "abort"
codegen-units = 1
lto = true
opt-level = "s"
strip = true
```

### 运行时优化

- 使用 `auto` 注入方式，自动选择最优方案
- 合理设置批量注入间隔，避免过快触发检测
- 关闭不必要的日志输出

## 故障排查

### 问题：注入失败

**解决方案**：
1. 确认以管理员身份运行
2. 检查目标进程是否存在
3. 尝试其他注入方式
4. 查看日志输出定位问题

### 问题：界面无法显示

**解决方案**：
1. 检查 WebView2 是否已安装
2. 更新显卡驱动
3. 尝试使用命令行模式

### 问题：程序崩溃

**解决方案**：
1. 检查 `config.json` 格式是否正确
2. 确认 DLL 文件路径有效
3. 查看 Windows 事件查看器获取错误信息

## 更新维护

### 版本更新

```bash
# 拉取最新代码
git pull origin main

# 重新构建
cd src-tauri
cargo tauri build
```

### 配置备份

定期备份 `config.json` 文件，避免配置丢失。

## 安全建议

1. **权限管理**：仅在需要时以管理员身份运行
2. **文件验证**：注入前验证 DLL 文件来源
3. **日志监控**：定期检查注入日志
4. **及时更新**：保持程序版本最新

---

如有问题，请提交 Issue 或联系开发者。
