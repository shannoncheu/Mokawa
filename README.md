# Mokawa

**A calmer way to manage your files.**

Mokawa 是使用 Rust + GPUI 开发的 Windows 10/11 x64 原生 PikPak 客户端。
应用标识：`app.mokawa.desktop`。不包含 WebView、Electron、Tauri 或多网盘 Provider。

> 当前为开发基础版本，尚未达到完整客户端交付标准。真实登录、文件访问、Windows 视觉效果和安装包需要验收。没有内置任何其他客户端的私有密钥，也不展示模拟网盘数据。

## 当前实现

- GPUI 原生深色三栏窗口；可编辑文本输入（含 IME）、目录搜索、名称排序、面包屑、多选、详情、加载和错误状态。
- PikPak HTTP 层：登录挑战、密码登录、刷新、分页列表、详情、新建目录、重命名、确认后批量移入回收站。调用真实端点，但目前仅有 Mock 验证。
- Windows Credential Manager 会话保存/恢复/移除，认证辅助程序 `mokawa-auth.exe`。
- Tokio 后台任务、过期请求隔离、退出取消、超时和敏感错误脱敏。
- 下载基础函数：取消、进度、独占临时文件、禁止覆盖；队列持久化和中断标记。尚未接入 UI；不支持暂停/续传。
- Windows 构建和 NSIS 安装脚本、CI、API 调查和测试记录。

尚未实现：完整图形登录、移动/复制、云端搜索、类型过滤、收藏、右键菜单、拖放、下载任务 UI、上传、离线任务、分享、回收站页面、媒体、托盘、多账号和其他 P2 功能。导航中 `planned` 项不可点击。

## Windows 构建

安装 Rust（仓库固定工具链）、Visual Studio C++ x64 构建工具及 Windows SDK、CMake、Git。使用 x64 Native Tools PowerShell：

```powershell
cargo test --locked --all-targets
cargo check --locked --features desktop --all-targets
cargo build --locked --release --features desktop --bins
.\target\release\mokawa.exe
```

GPUI 固定 crates.io `=0.2.2`，依赖锁定于 `Cargo.lock`；检查的是该发行包附带的实际源码，而非使用最新 main 的 API。详见 [GPUI 调查](docs/GPUI.md)。本仓库不发布 Linux/macOS 应用；云端 Linux 仅用于核心测试和 UI 源码类型检查。

## 认证前置条件

目前没有确认可供 Mokawa 使用的官方第三方客户端注册。**这会阻止普通用户完成登录，是正式交付阻塞项。** 不应从参考客户端复制 client secret 或验证码签名来绕过这个问题。

只有在获得 PikPak 许可和属于 Mokawa 的客户端身份后，才能在本机设置 `MOKAWA_PIKPAK_CLIENT_ID` 和 `MOKAWA_PIKPAK_CLIENT_SECRET`，运行 `mokawa-auth.exe`。程序通过终端隐藏输入账号、密码和获授权的稳定 device ID，直接连接 PikPak，再将会话写入 Windows Credential Manager。不要将凭据作为命令参数、提交到 Git 或粘贴到 issue。验证码要求会明确中止登录；当前未完成图形验证回调。此流程不是已验证可用的官方授权实现。

打开 Mokawa 后按 F5 读取会话并加载文件。搜索仅过滤已加载的当前目录。双击文件夹进入，Ctrl 点击多选。名称输入框用于新建目录和重命名。删除需要点击确认。退出账号通过 UI 或 `mokawa-auth.exe --logout` 移除本地会话；未验证服务端撤销会话接口。

## 测试与安装

```powershell
cargo fmt --check
cargo test --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings
makensis installer\mokawa.nsi
```

安装器输出 `dist/Mokawa-0.1.0-x64-setup.exe`，仅安装当前用户，卸载入口在 Windows 应用设置与安装目录。签名证书尚未配置，产物未签名。见 [Windows 验收清单](docs/WINDOWS.md) 与 [实际验证记录](docs/VALIDATION.md)。

## 安全与兼容性

Mokawa 与 PikPak 无官方关联。非官方接口可能变更、限流或触发账号风控；没有真实账号验证的功能不能视为可用。密码不持久化；会话仅使用系统凭据库，保存失败会报错而不会降级写明文。日志不记录请求体、响应体、Token、验证码或带签名的下载 URL。不要启用 HTTP wire 调试日志或导出明文凭据。

[API 能力矩阵](docs/PIKPAK_API.md) · [架构及实施计划](docs/ARCHITECTURE.md) · [安全说明](SECURITY.md) · [依赖许可证](THIRD_PARTY.md) · [变更记录](CHANGELOG.md)
