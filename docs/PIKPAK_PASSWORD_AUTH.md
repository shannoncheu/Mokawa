# PikPak 账号密码登录调查与实现

更新日期：2026-10-17

## 参考源码审查

本次审查的是以下仓库的实际源码，而非仅阅读 README：

| 项目 | 审查版本 | 许可证 | 认证实现 |
| --- | --- | --- | --- |
| OpenList | `3b28270a4066ea52943a69ecddd850d83f7e8bdc` | AGPL-3.0 | `drivers/pikpak/driver.go`、`drivers/pikpak/util.go`、`drivers/pikpak/meta.go`、`drivers/pikpak/types.go` |
| pikpaktui | `81d7621c66a6f747b60618a4bf8b336476071e66` | Apache-2.0 | `src/pikpak/mod.rs`、`src/pikpak/auth.rs`、`src/pikpak/models.rs` |

OpenList 的 `util.go` 通过 `POST /v1/shield/captcha/init` 初始化登录验证码，再向 `POST /v1/auth/signin?client_id=…` 提交用户名、密码、客户端 ID/Secret 和 captcha token。`refreshToken` 通过 `POST /v1/auth/token` 使用 refresh-token grant 并保存轮换后的 refresh token。请求还会携带设备 ID 与 captcha token。`RefreshCaptchaTokenInLogin` 按邮箱、手机号或用户名设置 meta，并包含其平台签名实现；`driver.go` 根据所选平台配置对应的客户端身份。

pikpaktui 的 `src/pikpak/mod.rs` 中 `login`、`init_captcha`、`refresh_session` 分别实现密码登录、验证码初始化和 token 刷新；登录会保存 access token、refresh token、设备 ID 和 captcha 状态。`src/pikpak/auth.rs` 定义相关认证响应类型。其 session 文件方案不适用于 Mokawa，因此 Mokawa 使用 Windows Credential Manager。

两项目均表明公开客户端确实在使用 PikPak 的密码 grant；这不能证明任意第三方客户端均获授权、服务端当前仍接受该 grant，或其平台身份可以移交给 Mokawa。参考源码包含平台客户端身份及 CAPTCHA 签名材料。本项目没有复制这些身份、Secret、签名盐、用户代理伪装或验证码绕过逻辑，也没有移植 AGPL 代码；只按公开请求结构独立实现了注册凭据可配置的认证流程。参考许可证和源码均未并入本仓库。

## Mokawa 实现

- `src/auth.rs` 实现验证码初始化、`password` grant、设备头、会话构造与 refresh-token grant。验证码 meta 会按输入区分 `email`、`phone_number` 与 `username`。
- `src/api.rs` 的认证请求携带注册 `client_id` 查询参数及 `x-device-id`；登录成功后调用已有真实文件列表 API 验证会话。
- `src-tauri/src/app.rs` 提供 `save_password_client_registration` 和 `login_with_password` 命令。密码只传给本机 Rust 后端、通过 HTTPS 发往 PikPak，不持久化、不记录日志。成功后只存会话凭据。
- `src/credentials.rs` 将用户提供的客户端注册 ID/Secret 与会话分别存入 Windows Credential Manager；普通配置文件不保存认证秘密。
- 登录页提供账号/邮箱和密码表单，以及可选的官方客户端注册凭据配置。未配置时不会发送密码。PAT 仍作为兼容选项保留。
- 验证码返回的外部 URL 只有在 HTTPS 且属于 PikPak 域名时才交给系统浏览器打开。Mokawa 不尝试代做、跳过或伪造验证码。

## 边界与验证状态

Mock 集成测试验证验证码初始化请求、账号类型 meta、注册客户端参数、设备头、密码 grant、真实文件列表调用以及 token 刷新请求结构。前端测试验证密码通过 Rust 命令提交，失败后输入框清空且 UI 不会伪造登录成功。这些测试不代表 PikPak 真实服务已接受登录。

本次没有可用于测试的真实 PikPak 账号，也没有 PikPak 官方签发给 Mokawa 的客户端注册 ID/Secret；因此未发送真实登录请求，未观察到服务端 HTTP 状态码或响应错误码，也没有真实验证文件详情、下载、refresh token 轮换和重启恢复。要完成线上验收，用户需先向 PikPak 获取可用于该密码 grant 的客户端注册信息，并在 Mokawa 登录页配置；随后使用允许密码登录的 PikPak 账号进行端到端验证。若账号仅通过 Google 等第三方注册，需先在 PikPak 官方渠道确认或设置独立密码。

当前认证请求不实现参考项目中的平台 CAPTCHA 签名材料。若 PikPak 返回人工验证 URL，Mokawa 会打开可信的 PikPak 验证页面并停止登录；用户完成页面验证后可重新提交。验证是否能使服务端随后返回可用 token，仍需真实账号和官方客户端身份确认。PikPak 官方可能限制密码 grant 或要求其官方客户端身份；本地 mock 不能证明该限制已解除。

## 许可证说明

参考仓库仅用于协议与行为调查，没有复制或链接其源码。OpenList 为 AGPL-3.0，pikpaktui 为 Apache-2.0。Mokawa 对照其公开请求结构独立实现；如未来实际移植任一项目代码，必须先完成相应许可证义务审查并保留声明。
