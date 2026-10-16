# Windows 构建、安装和验收

目标：Windows 10/11 x64，GPUI 原生 GPU 窗口。不支持其他平台发行。

构建依赖及命令见 README；GitHub Actions 采用 windows-2022，固定 Rust，先测试核心、类型检查 UI、编译 release，然后 NSIS 打包，执行静默安装与卸载检查。工作流是否成功必须以实际 run 为准，配置文件本身不代表通过。Runner 不保证有可用的交互式 GPU 桌面，因此窗口实测属于本机验收。

本机构建安装器：安装 NSIS 后先 `cargo build --locked --release --features desktop --bins`，创建 dist 目录，执行 `cargo metadata --locked --format-version 1` 将输出以 UTF-8 保存到 dist/dependency-licenses.json，再运行 `makensis installer/mokawa.nsi`。安装器不下载执行第三方脚本，安装到当前用户 LOCALAPPDATA/Programs/Mokawa，无需管理员权限。

卸载不会删除账号系统凭据（避免在退出失败时误认为远程会话已撤销）；卸载前使用 UI Sign out 或 `mokawa-auth.exe --logout` 明确移除。当前未配置代码签名，SmartScreen 可能提示未识别发布者，不能声称已签名。

## 必须人工验收，尚未通过

- [ ] Windows 10 和 Windows 11 x64 启动无 DLL 缺失，窗口标题 Mokawa。
- [ ] `mokawa.exe --smoke-test` 打开真实窗口并在五秒后退出，退出码 0；缺少 GPU 支持时记录失败。
- [ ] 文本框中文 IME、复制粘贴、选择、焦点；F5、Ctrl 多选、双击导航。
- [ ] 100/125/150/200% DPI、多显示器、缩放、最大化、关闭。
- [ ] 系统阴影/圆角；不透明降级可读。材质未实现，不将半透明冒充模糊。
- [ ] Windows Credential Manager 实际保存/恢复/删除，过期、重启、保存失败。
- [ ] 获授权真实账号的登录、刷新、分页读取和临时文件写入。
- [ ] 安装、升级、正在运行时重装、卸载、非 ASCII 用户名路径。
- [ ] 应用退出和网络故障时不残留活跃任务。
- [ ] 发布前核验全部传递依赖通知、运行库分发和签名状态。

## 故障排查

无窗口：检查 Windows 图形驱动和 GPUI GPU 支持，在交互式桌面运行 smoke test；云端 runner 构建成功并不保证远程无 GPU 环境能开窗。构建失败：核对 MSVC x64、SDK、CMake 和工具链。会话错误：先确认已使用 Mokawa 获授权的身份注册；不要复制其他客户端密钥。403/验证码：停止自动尝试，按 PikPak 官方要求完成验证。429：等待后手动重试。下载恢复未启用；队列中 Interrupted 状态不表示具备断点能力。

自动窗口检查使用 scripts/windows-smoke.ps1，分别启动账号、文件、任务、设置与窄窗口场景，检查本机窗口标题、退出码并尝试截图。它作为允许失败的独立诊断步骤运行，因为托管 Runner 可能没有可用 GPU 桌面；必须单独查看该步骤结果及 smoke-results.json，不能把整条工作流绿色等同于视觉验收通过。截图也不能代替 IME、拖放、多显示器、DPI 和材质人工验收。

## 已实际通过的 Runner 验证

最新代码 ce9a945 的 [Windows 运行](https://github.com/shannoncheu/Mokawa/actions/runs/38981753842) 已通过 16 项测试、原生编译、严格 lint、release、PE 品牌/静态 CRT 检查、NSIS、安装/卸载和五个窗口场景。截图已人工复查。Runner 是 Windows Server 2022；不能据此将上方 Windows 10/11 本机与 DPI 清单勾选为通过。

GPUI 0.2.2 的 release 着色器编译需要 Windows SDK 的 fxc.exe。若不在 PATH 且 SDK 不位于上游默认 10.0.26100.0 路径，应在构建前将 GPUI_FXC_PATH 设置为本机 SDK x64/fxc.exe 的完整路径。
