# 第三方依赖

本项目采用 Apache-2.0（LICENSE）。

`src/ui/input.rs` 改编自 GPUI 0.2.2 `examples/input.rs`，由 Zed Industries 及贡献者维护，Apache-2.0。修改包括组件导出、深色配色、初始化、Windows 键位、空文本点击边界与 IME 选择范围修正；保留其输入/IME 处理结构。来源：https://github.com/zed-industries/zed ，发行包见 crates.io/crates/gpui/0.2.2。

API 研究参考 lnzx/pikpak 与 lyqingye/pikpak-go（均 MIT），未复制其实现代码或身份密钥。源码 revision 见 docs/PIKPAK_API.md。

主要依赖：GPUI（Apache-2.0）、Tokio（MIT）、reqwest（MIT OR Apache-2.0）、serde/serde_json（MIT OR Apache-2.0）、tracing（MIT）、secrecy（MIT OR Apache-2.0）、keyring（MIT OR Apache-2.0）、rpassword（Apache-2.0）、embed-resource（MIT，构建依赖）、wiremock（MIT OR Apache-2.0，开发依赖）。

这不是完整传递依赖法律审计。CI 使用 cargo metadata 输出精确版本/许可证清单到安装包 `dependency-licenses.json`，发布前需检查所有非空许可证及传递依赖随附通知，尤其 GPUI 的平台、图像和字体组件；不能仅凭主依赖许可证推断全部许可。


Vue 桌面入口新增：Vue（MIT）、Vite（MIT）、TypeScript（Apache-2.0）、Tauri 2 / 单实例插件（Apache-2.0 OR MIT）、Lucide 图标（ISC）、image（MIT OR Apache-2.0）、rfd（MIT）、webbrowser（MIT OR Apache-2.0）。精确版本见 package-lock.json 与 src-tauri/Cargo.lock。

Noto Sans SC Variable 由 @fontsource-variable/noto-sans-sc 5.3.1 本地分发，SIL Open Font License 1.1，完整通知位于 resources/NOTO-OFL.txt，并随安装包分发。字体与静态前端总产物约 4.8 MiB（当前生产构建），字体按 Unicode 子集加载；不请求 Google 字体服务器。Mokawa 几何 M 图标为项目原创。
