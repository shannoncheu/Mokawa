# 第三方依赖

本项目采用 Apache-2.0（LICENSE）。

`src/ui/input.rs` 改编自 GPUI 0.2.2 `examples/input.rs`，由 Zed Industries 及贡献者维护，Apache-2.0。修改包括组件导出、深色配色、初始化和 Windows 键位；保留其输入/IME 处理结构。来源：https://github.com/zed-industries/zed ，发行包见 crates.io/crates/gpui/0.2.2。

API 研究参考 lnzx/pikpak 与 lyqingye/pikpak-go（均 MIT），未复制其实现代码或身份密钥。源码 revision 见 docs/PIKPAK_API.md。

主要依赖：GPUI（Apache-2.0）、Tokio（MIT）、reqwest（MIT OR Apache-2.0）、serde/serde_json（MIT OR Apache-2.0）、tracing（MIT）、secrecy（MIT OR Apache-2.0）、keyring（MIT OR Apache-2.0）、rpassword（Apache-2.0）、wiremock（MIT OR Apache-2.0，开发依赖）。

这不是完整传递依赖法律审计。CI 使用 cargo metadata 输出精确版本/许可证清单到安装包 `dependency-licenses.json`，发布前需检查所有非空许可证及传递依赖随附通知，尤其 GPUI 的平台、图像和字体组件；不能仅凭主依赖许可证推断全部许可。
