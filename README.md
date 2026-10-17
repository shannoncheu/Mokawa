# Mokawa

Mokawa 是使用 Vue 3 + TypeScript + Tauri 2 + Rust 开发的 Windows 10/11 x64 PikPak 桌面客户端。
应用标识：`app.mokawa.desktop`。仅支持 PikPak；不使用 Electron。新桌面入口使用 Windows WebView2，旧 GPUI 入口保留用于回退。

> 当前为开发基础版本，尚未达到完整客户端交付标准。Windows Runner 的编译、安装/卸载和窗口启动已验证；真实登录、文件访问、Windows 10/11 本机 DPI 与视觉验收仍待完成。没有内置任何其他客户端的私有密钥，也不展示模拟网盘数据。

## Vue 桌面版本

- 中文界面、自定义标题栏、浅色/深色/跟随系统，Noto Sans SC 本地字体及 Windows 字体回退。
- 设置 → 个性化：本地头像、背景图片、填充/完整显示/拉伸/居中、位置、模糊、亮度、图片和遮罩不透明度、面板不透明度、重置及即时预览。
- 图片不上传。限制 12 MiB、1600 万像素和解码分配，头像裁成 256×256，背景缩至最多 2560×1600；各保留一份，旧图片 URL 会释放。
- 主题和滑块即时生效，写盘防抖并显示保存状态。为保证可读性，面板不透明度范围 80–100%；窗口本身不透明，模糊只作用于应用内背景。
- Rust 保管凭据并复用现有 PikPak 文件/下载实现，前端不接触 Token；文件列表/网格虚拟滚动，任务中心按事件更新。
- **官方授权仍未接入**：Mokawa 专用注册、回调和 Token 使用范围待确认。当前新用户不能登录；不提供未经确认的密码入口，不伪造文件或空间用量。上传、移动/复制、收藏、离线任务、分享和媒体仍待实现/真实验证。

```powershell
npm ci
npm test
npm run build
npm run desktop -- dev
# Windows x64 安装包
npm run desktop:build -- --ci
```

需要 Rust、Visual Studio C++ x64 工具、Windows SDK、Node.js 24 和 Windows WebView2。NSIS 安装器在需要时下载微软 WebView2 引导程序，因此缺失运行时时需要联网。安装包未签名。Windows 构建证据与迁移边界见 [迁移记录](docs/VUE_MIGRATION.md)。

安装新版本前先退出旧 GPUI 版本，避免两个版本同时修改任务记录。卸载默认保留本地会话、个性化设置和任务记录；先在应用中清除本地会话/退出登录、重置外观可移除相应数据。手动数据目录为 `%LOCALAPPDATA%/Mokawa`，不要将其中数据或凭据导出到公开问题记录。

## 保留的 GPUI 版本

以下为旧原生入口和原有功能说明，代码保留，不代表 Vue 迁移已完成全部真实账号验收。

## 当前实现

- GPUI 中文原生窗口，自定义 Windows 标题栏；浅色、深色和跟随系统主题；独立登录、文件、任务中心和设置页；可编辑输入（含 IME）、目录搜索、名称排序、面包屑、多选、详情、加载和错误状态。
- PikPak HTTP 层：登录挑战、密码登录、刷新、分页列表、详情、新建目录、重命名、确认后批量移入回收站。调用真实端点，但目前仅有 Mock 验证。
- Windows Credential Manager 会话保存/恢复/移除；账号页说明连接状态，`mokawa-auth.exe --logout` 可移除本地会话。
- Tokio 后台任务、过期请求隔离、退出取消、超时和敏感错误脱敏。
- 下载任务中心：选择本地目录、两任务并发、进度、平均速度、取消、禁止覆盖、任务记录持久化和重启中断标记。网络传输及真实下载尚未实测；不支持暂停/续传或自动重试。
- Windows 构建和 NSIS 安装脚本、CI、API 调查和测试记录。

尚未实现：官方授权登录接入、移动/复制、云端搜索、类型过滤、收藏、拖放、上传、离线任务、分享、回收站页面、媒体、托盘、多账号和其他 P2 功能。未实现功能显示明确说明，不展示模拟数据。

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

公开 OIDC 端点和 S256 已确认，见 [OAuth 调查](docs/OAUTH.md)。**Mokawa 的桌面客户端注册、OAuth 原生客户端权限/回调契约、Token 对文件 API 的适用范围尚未确认，因此当前版本不能为新用户完成登录。** 不应通过复制官方 CLI 的私有 client ID/secret、读取它的凭据文件或让用户导出会话来绕过。

首次启动显示中文登录页，授权尚未开放时清晰说明注册限制。可查看官方授权说明，并尝试验证本应用系统凭据库已有会话。只有真实文件请求验证成功后才进入文件页；没有会话时不会误报凭据库故障。F5 刷新文件；搜索仅过滤已加载的当前目录。双击文件夹进入，Ctrl 点击多选。名称输入用于新建目录和重命名。删除需要确认。选择单个文件后点击“下载”，使用原生目录选择器创建下载任务。断电/强制终止可能留下 `.mokawa.part`，协作取消和 future 中止会清理自己创建的临时文件，进程强杀后不会静默删除未知文件或覆盖文件；重新下载前应人工核对和清理对应临时文件。

设置页可保存列表/网格、详情面板与主题偏好；主题选择会立即自动保存；窗口较窄时隐藏详情栏。退出账号通过 UI 或 `mokawa-auth.exe --logout` 移除本地会话；尚未实现服务端撤销授权。真实文件操作仍全部待合法账号验收。

## 测试与安装

```powershell
cargo fmt --check
cargo test --locked --all-targets
cargo clippy --locked --features desktop --all-targets -- -D warnings
makensis installer\mokawa.nsi
```

安装器输出 `dist/Mokawa-0.1.0-x64-setup.exe`，仅安装当前用户，卸载入口在 Windows 应用设置与安装目录。签名证书尚未配置，产物未签名。见 [Windows 验收清单](docs/WINDOWS.md) 与 [实际验证记录](docs/VALIDATION.md)。

## 安全与兼容性

Mokawa 与 PikPak 无官方关联。非官方接口可能变更、限流或触发账号风控；没有真实账号验证的功能不能视为可用。密码不持久化；会话仅使用系统凭据库，保存失败会报错而不会降级写明文。日志不记录请求体、响应体、Token、验证码或带签名的下载 URL。不要启用 HTTP wire 调试日志或导出明文凭据。

[API 能力矩阵](docs/PIKPAK_API.md) · [架构及实施计划](docs/ARCHITECTURE.md) · [安全说明](SECURITY.md) · [依赖许可证](THIRD_PARTY.md) · [变更记录](CHANGELOG.md)

## 已验证的开发版产物

[Windows CI 与安装包](https://github.com/shannoncheu/Mokawa/actions/runs/38981753842) 对应代码 `ce9a945`：15 项核心/Mock + 1 项 Windows 系统凭据库测试、release、安装/卸载和五个窗口场景全部通过。安装包未签名；账号授权仍未接入，不能用于新账号登录。

## 界面与账号使用

标题栏左侧为 Mokawa，右侧为最小化、最大化/还原和关闭；支持拖动和双击。登录页可打开“外观设置”，选择“浅色”“深色”“跟随系统”。选择主题会立即保存；其他文件显示偏好请点击“保存设置”。

文件页工具区可搜索当前目录、切换排序、列表/网格、新建文件夹、重命名、下载和移入回收站。重命名使用上方名称输入框；右键菜单提供同一操作入口。删除仍需确认。未接入的上传、收藏、离线下载、分享和回收站管理明确提示不可用，不展示模拟成功数据。

没有有效会话时显示登录入口；授权按钮会如实提示注册尚未完成，不会打开虚构授权地址或索取密码。会话验证期间可以取消。凭据库访问失败、没有会话和保存数据损坏分别提示；不会在保存失败时宣称保存成功。
