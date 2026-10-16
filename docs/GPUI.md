# GPUI 技术调查

调查日期：2026-10-16。

- 上游： https://github.com/zed-industries/zed 。调查时 main revision `322cd509dcccaee1f6bc44725d270598ae8171ad`，`crates/gpui/Cargo.toml` 声明 0.2.2，但该 main 并不等同于发行包。
- 实际选用：crates.io GPUI **0.2.2**，精确版本及 Cargo.lock checksum 固定。发行包 `.cargo_vcs_info.json` 指向 `69e2130295c2649963eb639fc70b4f2ee8ea1624`，dirty=true，因此不把它冒充完全相同的干净 Git revision。
- API 依据：发行包 `examples/hello_world.rs`、`input.rs`、`src/app/context.rs`、`src/platform.rs` 和 `src/platform/windows/window.rs`。使用 Application::run、open_window、WindowOptions、Render、Context::spawn、WeakEntity::update。输入基于 Apache-2.0 示例，保留归属与许可证。
- Windows：上游开发说明要求 MSVC x64 构建工具、Windows SDK（至少 10.0.20348）、CMake。上游完整 Zed 的数据库等依赖并非 Mokawa 独立 GPUI 应用需求。需要支持 GPU/D3D 的 Windows 主机验证。
- 工具链：GPUI edition=2024；未从发行 manifest 发现精确 MSRV。本项目固定本环境实际安装的 Rust 1.99.1，并不声称这就是 GPUI 的最低要求。
- 列表、布局、按钮、菜单、窗口及输入均可在 GPUI 内实现。输入不是 GPUI 内置完整高级控件，需要处理 EntityInputHandler/IME、焦点、选择和键盘绑定。当前使用滚动列表，尚未做大目录虚拟列表性能验收。
- 异步：GPUI 前台 executor 处理实体更新；Tokio Runtime 执行 HTTP，JoinHandle 被 GPUI spawn 等待，完成后通过弱实体更新。无同步网络阻塞 render。窗口退出会取消当前任务。

## 材质判断

发行包 `src/platform/windows/window.rs` 的 `set_background_appearance` 对 Opaque/Transparent/Blurred 分别调用 `set_window_composition_attribute`，Blurred 使用 ACCENT_ENABLE_ACRYLICBLURBEHIND。它不是可作为唯一方案依赖的公开 DWM 系统材质契约。

因此 Mokawa 默认使用 **Opaque** 与可读深色配色，当前没有声称实现透明、Acrylic 或 Mica。后续应以公开 `DwmSetWindowAttribute` / `DWMWA_SYSTEMBACKDROP_TYPE` 为 Windows 11 22621+ 评估路径，检测 OS 和返回结果后启用；Windows 10 和不支持主机维持不透明降级。需要先验证 GPUI renderer clear/background 与 HWND 互操作，不能只设置 DWM 属性就宣称成功。圆角、系统阴影、DPI 100/125/150/200% 均待 Windows 实测。

## 维护与升级

上游持续维护，但 GPUI 的公共 API 会变；不得直接浮动跟随 main。升级步骤：另开分支、下载目标发行包、比对窗口/输入/异步 API 和 Windows 后端、更新精确版本及锁文件、运行核心和 Windows 构建，再做 GPU/IME/DPI/材质人工验收。主依赖 GPUI 为 Apache-2.0；完整传递依赖清单从锁文件在 CI 生成。
