# Mokawa

**A calmer way to manage your files.**

Mokawa 是使用 Rust + GPUI 开发的 Windows 10/11 x64 原生 PikPak 客户端。
应用标识：`app.mokawa.desktop`。不包含 WebView、Electron、Tauri 或多网盘 Provider。

> 当前为开发基础版本，尚未达到完整客户端交付标准。真实登录、文件访问、Windows 视觉效果和安装包需要验收。没有内置任何其他客户端的私有密钥，也不展示模拟网盘数据。

## 当前实现

- GPUI 原生深色窗口，独立账号、文件、任务中心和设置页；可编辑输入（含 IME）、目录搜索、名称排序、面包屑、多选、详情、加载和错误状态。
- PikPak HTTP 层：登录挑战、密码登录、刷新、分页列表、详情、新建目录、重命名、确认后批量移入回收站。调用真实端点，但目前仅有 Mock 验证。
- Windows Credential Manager 会话保存/恢复/移除；账号页说明连接状态，`mokawa-auth.exe --logout` 可移除本地会话。
- Tokio 后台任务、过期请求隔离、退出取消、超时和敏感错误脱敏。
- 下载任务中心：选择本地目录、两任务并发、进度、平均速度、取消、禁止覆盖、任务记录持久化和重启中断标记。网络传输及真实下载尚未实测；不支持暂停/续传或自动重试。
- Windows 构建和 NSIS 安装脚本、CI、API 调查和测试记录。

尚未实现：官方授权登录接入、移动/复制、云端搜索、类型过滤、收藏、右键菜单、拖放、上传、离线任务、分享、回收站页面、媒体、托盘、多账号和其他 P2 功能。未实现功能显示明确说明，不展示模拟数据。

## Windows 构建

安装 Rust（仓库固定工具链）、Visual Studio C++ x64 构建工具及 Windows SDK、CMake、Git。使用 x64 Native Tools PowerShell：

```powershell
cargo test --locked --all-targets
cargo check --locked --features desktop --all-targets
cargo build --locked --release --features desktop --bins
.\target\release\mokawa.exe
```

GPUI 固定 crates.io `=0.2.2`，依赖锁定于 `Cargo.lock`；检查的是该发行包附带的实际源码，而非使用最新 main 的 API。详见 [GPUI 调查](docs/GPUI.md)。本仓库不发布 Linux/macOS 应用；云端 Linux 仅用于核心测试；GPUI 依赖及 UI 源码只在 Windows 目标编译。Linux 的 `--all-features` 检查不验证 Windows UI。

## 认证前置条件

PikPak 官方现已公开 Connected Apps，并在 [官方 CLI 文档](https://github.com/pikcloud/skills/blob/main/skills/pikpak-cli/SKILL.md) 说明浏览器 OAuth 和设备授权，且注明密码登录不再支持。此前参考客户端的密码 API 仅保留作协议/Mock 研究，`mokawa-auth.exe` 默认明确报授权接入未完成，不再索取密码。

**Mokawa 的桌面客户端注册、OAuth 端点/权限/回调契约、Token 对文件 API 的适用范围尚未确认，因此当前版本不能为新用户完成登录。** 不应通过复制官方 CLI 的私有 client ID/secret、读取它的凭据文件或让用户导出会话来绕过。

账号页可以打开官方 Connected Apps 说明，并尝试读取本应用系统凭据库已有会话。没有真实会话时会报错，不显示伪造文件。F5 刷新文件；搜索仅过滤已加载的当前目录。双击文件夹进入，Ctrl 点击多选。名称输入用于新建目录和重命名。删除需要确认。选择单个文件后点击 Download，使用原生目录选择器创建下载任务。断电/强制终止可能留下 `.mokawa.part`，当前不会静默删除它或覆盖文件；重新下载前应人工核对和清理对应临时文件。

设置页可保存行密度与详情面板偏好；窗口较窄时隐藏详情栏。退出账号通过 UI 或 `mokawa-auth.exe --logout` 移除本地会话；尚未实现服务端撤销授权。真实文件操作仍全部待合法账号验收。

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
