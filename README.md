# Mokawa

Mokawa 是面向 Windows 10/11 x64 的 PikPak 桌面客户端，采用 Vue 3、TypeScript、Tauri 2 和 Rust。应用标识为 `app.mokawa.desktop`，仅支持 PikPak。

**当前为开发预览：Mokawa 的官方授权客户端注册与回调规则仍未确认，新用户暂时无法登录。** 不提供未经确认的密码登录，不导入其他客户端的凭据，不展示模拟网盘文件或容量。尚未达到完整客户端正式交付标准。

## 界面与个性化

- 默认简体中文，自定义标题栏支持拖动、双击最大化、最小化、还原和关闭。
- 浅色、深色和跟随系统，选择后立即生效并保存；Noto Sans SC 字体随包离线加载，失败时回退 Windows 字体。
- 首次打开显示登录页，可直接打开「外观设置」。设置 → 个性化提供本地头像、背景图片、填充/完整显示/拉伸/居中、位置、模糊、亮度、图片透明度、遮罩颜色/不透明度及面板不透明度。
- 头像和背景不上传、不修改 PikPak 账号头像。源文件保持不变；支持 PNG/JPEG/WebP，限制 12 MiB、1600 万像素和解码分配；头像裁成 256×256，背景缩至最多 2560×1600。
- 滑块实时预览，写盘防抖并显示保存状态；可更换、移除、恢复默认或重试保存。为保持文字可读，面板不透明度最低 80%。窗口和文字保持不透明，背景模糊属于应用内效果，不是 Mica/Acrylic。
- 文件区采用列表/网格虚拟滚动，搜索排序在 Worker 执行；任务中心按事件更新，不进行空闲轮询。

## 功能边界

Rust 核心已实现会话存储/刷新、分页文件请求、目录导航、新建文件夹、重命名、确认后批量移入回收站、流式本地下载及 PikPak 云端离线任务，并接入新界面。**这些 PikPak 功能目前只有 Mock 验证，没有合法测试账号下的真实成功记录。** 有本应用已有会话时，也必须真实文件请求成功后才进入文件页。

搜索仅筛选已加载的当前目录，支持名称排序、双击进入目录、面包屑、多选、详情和右键操作。新建和重命名使用对话框；删除需确认，单次最多 100 项。单个/多选下载共用原生目录选择器，按 API 文件大小预检磁盘空间、最多 2 并发，并对同名目标自动追加编号避免覆盖；下载任务持久化，取消会清理本次创建的临时文件。HTTP Range 未验证，不提供暂停/续传；失败可重新下载。`.torrent` 文件仅在本地解析 BTIH 后提交 Magnet，不上传种子文件，也不运行本地 BT 引擎。云端离线任务可查询、刷新、重试失败项和删除任务记录（保留完成文件）。这些接口仍待真实账号验证。

登录页提供本机 PAT 兼容探测：令牌只发往 PikPak，读取根目录成功后才保存到 Windows Credential Manager。官方 FAQ 将 PAT 定位为个人自动化用途，并建议第三方应用通常采用自己的授权；PAT 到 Drive REST API 的实际兼容性尚未通过真实账号验证。OAuth 客户端注册与浏览器回调仍未接入。详见 [PAT 调查](docs/PAT_AUTH.md)。

尚未接入：上传、移动/复制、收藏、云端搜索、分享、完整回收站、媒体、托盘及多账号。界面明确提示相关能力不可用。

PikPak 已公开 Connected Apps，官方 CLI 文档说明浏览器和设备 OAuth、密码登录不再支持。但 Mokawa 的注册、原生客户端回调契约及 Token 对文件 API 的适用范围尚未确认。参见 [OAuth 调查](docs/OAUTH.md) 和 [API 能力矩阵](docs/PIKPAK_API.md)。不得复制其他客户端私有密钥或绕过验证码、访问控制和会员限制。

## Windows 开发与安装

需要 Rust（仓库固定工具链）、Visual Studio C++ x64 工具、Windows SDK、Node.js 24 和 Windows WebView2。

```powershell
npm ci
npm test
npm run build
npm run desktop -- dev
# Windows x64 NSIS 安装包
npm run desktop:build -- --ci
```

构建钩子自动生成依赖许可证清单。若单独执行 Tauri Rust 测试，先运行 `npm run licenses`。安装器在 `src-tauri/target/x86_64-pc-windows-msvc/release/bundle/nsis/`，只安装当前用户，未代码签名。缺少 WebView2 时安装器需联网下载微软引导程序。卸载入口位于 Windows 应用设置与安装目录。

[已通过的 Vue Windows CI 与安装包](https://github.com/shannoncheu/Mokawa/actions/runs/39012881124) 包含编译、测试、NSIS、静默安装/卸载、真实 WebView2 个性化恢复、窗口操作与短时进程树采样。实际通过 Windows 29 项核心测试、2 项桥接测试、10 项前端测试及原生验收；实际结果和限制见 [迁移记录](docs/VUE_MIGRATION.md)。GitHub Release 中较早的 GPUI 预览包不能视为本次 Vue 安装包。

安装前先退出旧 GPUI 版本，避免两个版本同时修改任务记录。卸载默认保留会话、个性化设置和任务记录；应用中的清除本地会话/退出登录和外观重置可移除相应数据。本地目录为 `%LOCALAPPDATA%/Mokawa`；不要公开其中数据、凭据或未经脱敏的日志。

## 验证与稳定性

```powershell
npm run format:check
npm test
npm run test:ui
cargo fmt --check
cargo test --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings
```

浏览器 UI 测试不代替 Windows 原生验收；Linux 不编译 Windows Tauri/GPUI 模块。Windows Runner 也不代替 Windows 10/11 实机高 DPI、系统主题、休眠唤醒、真实账号及长时间运行验收。短时内存采样不证明零泄漏，正式资源预算仍待实际负载和 1 小时/24 小时曲线确定。参见 [性能与资源审查](docs/PERFORMANCE.md)。

## 保留的 GPUI 入口

原有 GPUI 源码、核心测试和 Windows 工作流保留作为回退，固定 GPUI `=0.2.2`，仅在 Windows 目标编译。新 Vue 入口独立使用 `src-tauri/Cargo.lock`，复用根 Rust 库。

```powershell
cargo build --locked --release --features desktop --bins
.\target\release\mokawa.exe
```

原 GPUI 安装说明和历史验证见 [Windows 文档](docs/WINDOWS.md)、[历史验证记录](docs/VALIDATION.md) 和 [GPUI 调查](docs/GPUI.md)。

## 安全与兼容性

Mokawa 与 PikPak 无官方关联。非官方接口可能变化、限流或触发账号风控。Token 只在 Rust 与 Windows Credential Manager 中处理，保存失败报错，没有明文回退；前端不接触 Token 或签名下载地址。日志不记录密码、请求体、Token、Cookie 或验证码，不启用 HTTP wire 调试。应用不包含 Mokawa 中转服务器。

[迁移计划与阶段结果](docs/VUE_MIGRATION.md) · [安全说明](SECURITY.md) · [依赖许可证](THIRD_PARTY.md) · [变更记录](CHANGELOG.md)
