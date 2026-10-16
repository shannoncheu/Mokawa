# 官方 OAuth 发现调查

2026-10-16，本轮仅执行公开只读 GET，没有注册应用、发送账号凭据、请求设备验证码或兑换 Token。

| 请求 | 实际响应 |
|---|---|
| `https://pikpak.ai/.well-known/oauth-protected-resource/mcp` | HTTP 200；resource 为 https://pikpak.ai/mcp，授权服务器为 https://user.mypikpak.com，列出文件/离线/分享等 scopes |
| `https://pikpak.ai/mcp`（无认证 GET） | HTTP 401，WWW-Authenticate 指向上述资源元数据 |
| `https://user.mypikpak.com/.well-known/oauth-authorization-server` | HTTP 404；不能将这个失败当作“没有官方 OAuth” |
| `https://user.mypikpak.com/.well-known/openid-configuration` | HTTP 200；公开 OIDC 发现信息，见下表 |

公开原始响应：[OIDC](research/pikpak-openid-configuration.json)、[MCP 资源](research/pikpak-resource-metadata.json)。这些文件不包含账号信息或秘密。

| 字段 | 官方响应 |
|---|---|
| issuer | https://user.mypikpak.com |
| authorization_endpoint | https://mypikpak.com/drive/auth-center |
| device_authorization_endpoint | https://user.mypikpak.com/v1/auth/device/code |
| token_endpoint | https://user.mypikpak.com/v1/auth/token |
| revocation_endpoint | https://user.mypikpak.com/v1/auth/revoke |
| userinfo_endpoint | https://user.mypikpak.com/v1/user/me |
| registration_endpoint | https://user.mypikpak.com/v1/clients-registrations/openid-connect |
| code_challenge_methods_supported | S256、plain；Mokawa 只应选择 S256 |
| grant_types_supported | authorization_code、refresh_token、client_credentials、device_code；没有 password |
| token_endpoint_auth_methods_supported | client_secret_post、client_secret_basic；没有列出 none |
| scopes_supported（OIDC） | openid、profile、email；与 MCP 资源的 scopes 含义不同 |

## 需要确认的注册与适用性

公开注册 URL 的存在不等于任意原生客户端都能注册成功。本轮没有 POST 注册请求，不能报告已获得 Mokawa client_id。必须确认：

- 原生桌面 public client 是否允许 `token_endpoint_auth_method=none`，或是否需要官方批准的其他方式。发现文档目前只声明需要 secret 的认证方式。
- `application_type=native`、loopback redirect URI、动态端口及 PKCE 的服务端接收规则。
- 每次安装的独立动态注册与应用级预注册分别适用于哪些客户端，是否有条款或人工审批要求。
- MCP resource 的 Token audience 是否只限 MCP；不能直接把它发送到旧文件 REST API，并把 200/401 当作完整接口契约。
- OAuth 返回的 scope、expires_in、刷新轮换、撤销行为和权限不足错误语义，需实际授权后验证。

## 下一步实现约束

优先系统浏览器 + 授权码 + S256；state 使用安全随机数，监听器仅绑定本机 loopback，校验准确回调路径和 state，设置短期超时、一次性消费和取消。Token 只存 Windows Credential Manager，不导出用户密码、PAT 或官方 CLI 会话。不得把 CLI 或 Android 客户端的身份参数当作 Mokawa 身份。

应先取得适用于 Mokawa 的注册契约，准备可审阅的客户端元数据，再由用户决定是否向 PikPak 注册或批准账号连接。真实授权、创建测试文件与删除验收均未执行。目前账号页明确显示授权集成尚未完成；旧密码调用仅作为历史 Mock 研究保留。

来源边界：user.mypikpak.com 的 OIDC 发现来自 PikPak 已知认证域名；pikpak.ai 的 MCP 地址最初来自外部公开资料，本轮未在官方公开页面找到可核对该地址的直接链接。因此 MCP 响应仅作为候选资源的只读观察，不把它的域名或 Token audience 当作已获官方确认的生产契约。Mokawa 没有向该候选地址发送凭据。
