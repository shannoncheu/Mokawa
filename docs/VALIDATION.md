# Mokawa 阶段交付与真实验证记录

日期：2026-10-16。所有结果区分核心/Mock、Windows 编译、Windows 窗口、真实 PikPak。Mock 通过不代表真实网盘功能可用。

## 阶段一：保留成果与平台修复

- 初始仓库 main 没有 commit，30 个工程/文档文件未跟踪。没有 reset、清理或重新初始化仓库；以 `154a755` 保存已有工作。远程仍为 https://github.com/shannoncheu/Mokawa.git 。
- 修改：Cargo.toml、src/main.rs、src/ui/input.rs、.github/workflows/windows.yml。
- 命令：`git status --short`、`git log`、`cargo tree --locked --features desktop -i xattr`、`cargo tree --locked --features desktop --target x86_64-pc-windows-msvc -i xattr`、`cargo check --locked --all-features --all-targets`。
- 定位：GPUI → gpui_http_client → zed-async-tar → cfg(unix) xattr。Windows 无 xattr；修复为 target Windows 专用 GPUI 与 cfg UI 模块。保留 libc 0.2.190，没有降级依赖树。
- 实际结果：Linux 共享核心检查通过，两目标树不再引入 xattr。Linux all-features 不验证 Windows UI。
- 下一步：实际 Windows CI。

## 阶段二：Windows Runner 编译闭环

命令：`git push`、`gh run list/view --repo shannoncheu/Mokawa`、`gh run view <id> --log-failed`。工作流执行 fmt、core tests、Windows native check、desktop Clippy、release build、NSIS、安装/卸载、独立窗口诊断。

| Run | 代码 | 已实际观察结果 |
|---|---|---|
| [38979353343](https://github.com/shannoncheu/Mokawa/actions/runs/38979353343) | 4f37253 | 失败：UI 鼠标事件类型推断、输入组件 size 导入；已修复。 |
| [38979824894](https://github.com/shannoncheu/Mokawa/actions/runs/38979824894) | 45f6858 | Windows 核心测试通过；native check 失败：Context::on_app_quit 需要两个参数；已按发行源码修复。 |
| [38980197660](https://github.com/shannoncheu/Mokawa/actions/runs/38980197660) | d002fa8 | Windows 核心 13 测试通过，UI check 通过；严格 Clippy 失败：两个赋值空格诊断；已修复。安装包未执行。 |
| [38980394899](https://github.com/shannoncheu/Mokawa/actions/runs/38980394899) | c35603e | 13 核心测试、UI check、严格 Clippy、release、PE 元数据/导入、安装包、安装/卸载、五场景窗口启动均通过。但人工看截图发现大窗口超出 Runner 屏幕，已继续修复默认尺寸并加强边界断言。 |

附加修改：.cargo/config.toml 为 MSVC 静态 CRT；build.rs/resources/mokawa.rc 添加 Mokawa 产品/版本元数据。CI 检查 PE 导入并拒绝意外动态 VCRUNTIME/MSVCP 依赖；不能仅因安装脚本生成就宣布兼容干净 Windows。

## 阶段三：页面、异步与任务

- 修改：src/ui/mod.rs、src/ui/pages.rs、src/ui/input.rs、src/transfers.rs、src/download.rs、src/settings.rs、src/credentials.rs、tests/core.rs。
- 账号页：清晰显示授权未接入，可打开官方资料、尝试恢复本应用凭据库已有会话。没有伪造“登录成功”。
- 文件页：保留导航、搜索、排序、多选、详情、新建目录、重命名、删除确认。明确鼠标事件类型；长名称截断；较窄窗口隐藏右侧详情。失败写操作保留原列表，导航清理过期删除弹层。
- 任务中心：真实文件详情链接、原生目录选择、最多两任务、进度/平均速度、取消、记录保存；重启后 Queued/Running 标记 Interrupted，不声称支持 Range。没有上传/离线假队列。
- 设置页：行密度与详情栏偏好异步保存，不导出凭据。
- 状态修复：异步结果有 generation 检查；退出账号使待保存的旧会话失效；窗口退出取消传输并有限等待清理；持久化写入串行化。
- 本地命令：`cargo fmt --check`、`cargo test --locked --all-targets`、`cargo clippy --locked --all-targets -- -D warnings`。
- 已实际通过：13 项测试，含原 10 项及设置往返/损坏、失败下载记录无签名 URL、退出会话旧写入拦截。此处没有调用真实 PikPak。之后针对 IME 中文/emoji 组合范围加入第 14 项回归并通过；为退出账号早于下载线程注册的竞争再加入第 15 项回归并通过；最终 Windows CI 另有隔离的真实 Credential Manager 合成凭据往返测试，不访问应用账号凭据。
- Windows 视觉验证：scripts/windows-smoke.ps1 配置五个真实窗口场景，结果必须单看 smoke-results.json；Runner 无 GPU 桌面会明确失败。这不替代本机 IME、DPI、多显示器、材质验收。

## 阶段四：官方认证与剩余能力

- 修改：README.md、docs/PIKPAK_API.md、docs/ARCHITECTURE.md、docs/GPUI.md、docs/WINDOWS.md、SECURITY.md、src/bin/auth.rs、src/error.rs。
- 实际调查：读取官方 FAQ / Connected Apps / pikcloud/skills CLI / pikcloud/pikpak-open 文档。发现官方已支持浏览器和设备授权，官方 CLI 不再支持密码登录。已停止用户入口收集密码，旧协议仅留作 Mock 研究。
- 阻塞：公开 OIDC/MCP 端点已实际读取 HTTP 200，确认授权、设备、Token、注册、撤销和 S256。Mokawa 原生注册政策、loopback 回调、权限和 REST token audience 契约仍未确认。没有真实账号、没有申请/授权、没有验证码处理实测；不能以复制 CLI 私有身份或读取其会话文件取代。
- 真实登录、刷新、文件列表、创建目录、重命名、删除、下载：**全部尚未执行真实验收**。
- 上传、分享、离线下载、回收站完整页面与媒体：**未实现**。官方 CLI 资料确认了部分服务能力，但没有由此猜造 REST 端点。
- 下一步：先完成 Windows 产物与本机运行验收；取得官方适用协议后实现浏览器授权并安排用户本机测试，再按临时测试目录逐项验证 P0。此后继续 P1/P2。

## 截图复查与最终复验

第一个完整流水线成功后，已下载安装包、PE 导入清单和五张真实窗口截图。DLL 导入仅为 Windows 系统库，没有动态 VCRUNTIME/MSVCP。窗口进程检查通过不等于画面无问题：大窗口截图实际被屏幕裁切，窄窗口完整。已根据 GPUI primary_display 的逻辑边界限制初始尺寸、同步降低最小尺寸，并在烟雾脚本用 Windows WorkingArea 检查可用桌面边界，等待最终 Windows 复验。
