# PikPak 登录阻塞报告

复核日期：2026-10-17。原始问题是 Tauri OAuth 命令尚未接入。本轮已新增 PAT 的本机输入与真实 Drive 只读验证路径；没有可用 PAT，因此没有宣称线上认证成功。没有提交登录、动态注册、设备码、Token 兑换或账号凭据请求；没有真实账号或已获批的 Mokawa OAuth 客户端身份。

## 结论

OAuth 路径仍然不能进行新账号登录：Tauri 的 `begin_authorization` 命令无条件返回 `AuthorizationPending`，没有启动系统浏览器、请求授权码/设备码、接收回调或兑换 Token。登录页现在优先提供 PAT 路径：输入后会对真实 Drive API 发起一条根目录读取请求，只有成功才保存凭据。该实现尚未用真实 PAT 验证，因此仍不能声称用户目前一定能登录。

官方公开资料确认 PikPak 有 OAuth 登录能力，官方 CLI 资料列出浏览器和设备码流程，并明确密码登录已不再支持。但这些资料没有给出可直接用于 Mokawa 的已批准客户端凭据、动态注册请求契约、桌面回调规则，也没有确认所获 Token 能否访问本应用所用的 Drive REST API。发现文档列出的 Token 端点认证方式只有 `client_secret_post` 和 `client_secret_basic`，没有 `none`。不能把客户端密钥编译进桌面程序，也不能借用 CLI 的私有身份。

PAT 对第三方应用的使用与 FAQ 的通常建议有张力，Mokawa 已将此路径明确标为个人令牌兼容尝试，并在页面说明限制。其直接 Bearer 格式仅由官方 CLI 对 PAT 的接收方式及 Mokawa 现有 Drive API 请求形态推导，公开 API 文档没有给出线级 PAT 规范；服务端成功读取根目录才会确认本机这一令牌可用。OAuth 路径仍需 PikPak 对 Mokawa 的客户端注册/授权及原生客户端 Token 兑换方式明确支持。

## 当前认证链路

| 环节 | 当前实现 | 结果 |
|---|---|---|
| PAT 登录页 | `web/src/App.vue` 本地输入 PAT，调用 `connect_personal_access_token` | 令牌经 Tauri IPC 送到本机 Rust；输入在调用前清空 |
| PAT 验证 | `src-tauri/src/app.rs` 创建 PAT 会话并调用 `Api::verify_personal_access_token` | 仅向 PikPak Drive API 读取根目录 1 项；成功后存入 Windows Credential Manager；失败不保存 |
| OAuth 登录页 | Tauri `begin_authorization()` 仍返回 `Error::AuthorizationPending` | 无浏览器或设备授权请求 |
| 启动时恢复 | `bootstrap()` 只检查 Windows Credential Manager 中是否有会话；`authorization_ready` 固定为 `false` | 有旧会话时 UI 仍须请求文件列表才能确认会话有效；无会话时无法登录 |
| 文件列表 | `session()` 从 Windows Credential Manager 取会话，过期后用 `MOKAWA_PIKPAK_CLIENT_ID` 和 `MOKAWA_PIKPAK_CLIENT_SECRET` 刷新，再请求 Drive API | 过期会话在没有这两个注册变量时无法刷新；当前没有成功建立该会话的 UI 流程 |
| 旧密码实现 | `src/auth.rs` 发送 captcha 初始化及 `grant_type=password` 的旧请求 | 没有被 Tauri 登录命令调用；官方 CLI 文档称密码登录不再支持，不能将其作为当前方案 |
| 用户信息 | OIDC discovery 暴露 userinfo 地址，但 Mokawa 没有用户信息调用/模型 | 未实现，真实账号资料未读取 |
| 退出 | 本地凭据删除 | 不能据此声称已调用 PikPak 服务端 Token 撤销 |

`src/api.rs` 将 Drive 请求发往 `api-drive.mypikpak.com`，且要求 Bearer Token、设备 ID 和 captcha token。未来接入 OAuth 时必须确认这些旧式请求头的必要性、Token audience、scope 和设备标识规则；不可把 MCP 或 CLI Token 未经验证地用于此 REST API。

## 官方流程与参考实现核实

1. PikPak OIDC discovery：`https://user.mypikpak.com/.well-known/openid-configuration`，本次只读 GET 实测 HTTP 200。它列出浏览器授权端点、device authorization 端点、Token/撤销/userinfo 端点、动态注册端点及 S256 PKCE；grant types 包含 `authorization_code`、`refresh_token`、`client_credentials` 和 device code，不含 password。
2. Token discovery 声明 `client_secret_post`、`client_secret_basic`，未列出原生 public client 常用的 `none`。元数据没有说明 Mokawa 是否可动态注册、是否要求人工批准、device flow 如何认证客户端、redirect URI 允许规则，以及 REST API 需要的 Token audience/scope。
3. PikPak 公共 `pikcloud/skills` 项目中的 CLI 使用文档提供 `PIKPAK_TOKEN` 环境变量以及 `pikpak auth token <token>`，并列出使用 CLI 的浏览器 OAuth、设备码和文件命令：<https://github.com/pikcloud/skills/blob/main/skills/pikpak-cli/SKILL.md>。这确认官方 CLI 接受 PAT 作为凭据并具备文件操作命令；文档没有公布 PAT 对应的 REST 主机、HTTP 头或服务端 audience，也不能证明 CLI 客户端身份或其凭据获准由 Mokawa 重用。没有复制其代码、密钥、Token 或本地会话。
4. PikPak Connected Apps 帮助页本次 GET 返回 HTTP 200：<https://mypikpak.com/en-US/help-center/connected_apps/managing_connected_apps/what_are_connected_apps>。这确认存在已连接应用的用户授权概念，不构成 Mokawa 注册已获批准的证据。
5. 公开 MIT 参考项目 `lnzx/pikpak` 与 `lyqingye/pikpak-go` 可用于理解历史接口，但旧密码/设备/验证码实现不能证明当前接口仍有效或获 PikPak 允许。它们不提供 Mokawa 的客户端身份。

未向注册端点 POST。对 `https://user.mypikpak.com/v1/clients-registrations/openid-connect` 的只读 GET 返回 HTTP 501，响应为 `error=unimplemented`、`Method Not Allowed`；OPTIONS 返回 204。此结果不能确定正确 POST schema、注册是否开放或是否需要审批。没有用探测性 POST 创建客户端或触发外部状态变化。

## 本次实际请求证据

以下均为不带账号凭据、Token、Cookie 的只读请求。PAT 的实际 Drive 请求只有在 Windows 用户自行输入真实 PAT 后才会发生，本轮没有此令牌：

| 请求 | 实测结果 | 说明 |
|---|---|---|
| OIDC discovery GET | HTTP 200 | 返回公开 OIDC 元数据；不是登录测试 |
| Connected Apps 帮助页 GET | HTTP 200 | 官方帮助页可访问 |
| 动态注册 URL GET | HTTP 501，`Method Not Allowed` | 未发送 POST，注册契约未确认 |
| 动态注册 URL OPTIONS | HTTP 204 | 未披露方法/注册 schema |
| Drive `GET /drive/v1/files`，无 Authorization | HTTP 401，`error_code=16`，`invalid auth header` / `Unauthenticated, please re-login` | 证明无凭据请求被拒绝，不是某个 PikPak 账号的登录失败响应 |
| Userinfo `GET /v1/user/me`，无 Authorization | HTTP 401，`error_code=16`，提示需 Bearer Token | 无真实用户信息可读取 |

错误响应没有包含用户凭据。应用当前 `begin_authorization` 路径自身没有 HTTP 状态码，因为它在本地立即返回；不能把上面的未认证 401 描述成登录接口实测失败。

## 测试与环境边界

- 真实 PAT 登录、账号信息、真实文件列表、重启恢复、PAT 过期/撤销及服务端撤销：均未执行。没有测试令牌或授权条件，也没有 Mokawa 客户端身份。
- `tests/core.rs` 的登录验证码、Token 刷新以及新增 PAT Bearer/401/403/不刷新测试均为 Wiremock Mock 测试；它们只验证本地协议处理，不验证 PikPak 线上 PAT 兼容性或 Tauri 的 Windows Credential Manager。本轮使用仓库固定 Rust 1.99.1 执行 `cargo test --locked`：7 个库测试、25 个核心集成测试通过；Windows 专属凭据测试在 Linux 上显示 0 tests。
- 前端 `npm test -- --run`：5 个文件、10 项测试通过；`npm run build` 与 `npm run format:check` 通过。Linux 结果不代表 Windows Tauri 编译或桌面运行。
- Windows 目标交叉 `cargo check --locked --manifest-path src-tauri/Cargo.toml --target x86_64-pc-windows-msvc` 未通过：当前 Linux 环境没有 Visual Studio/MSVC `lib.exe`，`ring` build script 停止。未将其描述为 Windows 构建成功；需要 Windows Runner 验证新增 Tauri command 与凭据保存。
- 当前工作区基线为 `main`；此前未提交的本报告保留。本轮增改认证代码、登录表单和测试，未删除已有测试。

## 后续验证

先在 Mokawa Windows 构建中由账号持有人本地输入一个短期、只读 PAT，确认 PAT→真实根目录读取→Credential Manager 保存→重启后真实列表→下载。仅分享脱敏的 HTTP 状态/错误类别；不要发送令牌。如果 PAT 直连 REST API 得到 401/403，则保留失败证据并停止声称 PAT 可用，询问 PikPak 是否允许该方式/是否有官方令牌 audience 或认证代理。

上传当前未实现；账号资料 API 未接入。不能把 OAuth 客户端注册以外的 PAT 路径说成已完全替代 OAuth。

## OAuth 解阻所需条件

需要 PikPak 官方或获授权的应用注册渠道提供以下非账号秘密信息：

1. Mokawa 的 `client_id` 或获准的每安装动态注册流程；如必须提供客户端密钥，须明确原生桌面客户端如何合法使用它，不能要求将共享秘密嵌入安装包。
2. 获准的授权方式（浏览器 authorization code + PKCE 或 device code）、scope、回调 URI 规则、state/PKCE 要求、取消/过期处理及 Token 端点认证要求。
3. Token 是否适用于 `api-drive.mypikpak.com`，以及 Drive 请求所需的设备/captcha 头、用户信息接口和刷新/撤销规则。
4. 由 Mokawa 操作者在 Windows 桌面端完成的真实测试账号交互。密码、验证码、Cookie、Token 不应发到聊天或提交到仓库；可使用脱敏日志报告结果。

获得上述契约后，再决定实现系统浏览器/设备码流程、随机 `state` 与 S256 PKCE 校验、短时回调/轮询、可取消授权、错误映射、Credential Manager 持久化及过期刷新。若 PikPak 不批准第三方 Windows 客户端注册，应明确记录 OAuth 登录限制；PAT 是否继续可用取决于真实 API 验证结果。
