# PikPak API 可行性与能力矩阵

调查日期：2026-10-16。**没有提供真实账号或 Mokawa 获授权的客户端注册，因此全部真实 API 均未实测。** Mock 通过只验证 Mokawa 请求、模型和错误路径，不证明线上服务可用。

本轮离线 API 交叉研究源：Bengerthelorf/pikpaktui `81d7621`（Apache-2.0，`src/pikpak/offline.rs`）、OpenListTeam/OpenList `3b28270`（AGPL-3.0，`drivers/pikpak/driver.go` 与 `types.go`）、rclone/rclone `4c52ae0`（MIT，`backend/pikpak/pikpak.go`）、lnzx/pikpak `3911b13`（MIT，`internal/pikpak/task.go`）。仅核对公开端点与字段，不复制其代码、密钥或身份材料；OpenList 的 AGPL 代码没有复制或链接进 Mokawa。

参考源：

- A：https://github.com/lnzx/pikpak/tree/3911b1363705561d96b2fb06583a0054f2947369 ，MIT。
- B：https://github.com/lyqingye/pikpak-go/tree/981c90c766453ad03c462236934697a750eab562 ，MIT。

仅研究接口结构，没有复制其私有身份参数、密钥或验证码签名。以下“已从代码确认但尚未实测”指参考源码存在对应实现，不代表 Mokawa 已实现。

| 能力 | 状态 | 来源/接口与限制 | Mokawa 状态 |
|---|---|---|---|
| PAT 本地连接 | 已从官方文档确认 PAT 可供官方 CLI 使用；REST 直接认证尚未实测 | FAQ 建议第三方应用通常走自身授权；没有公开 PAT 的 header/audience 规范 | 新增本机 Bearer 只读探测，真实验证成功后存 Windows Credential Manager；真实兼容性待测，见 [PAT 调查](PAT_AUTH.md) |
| 官方第三方授权/客户端注册 | 尚未确认 | 官方 OIDC/MCP 发现端点已实际 GET 200；原生注册政策与 REST Token 适用性未确认 | 集成阻塞项，见下文官方资料更新 |
| 旧密码登录 | 已从代码确认但尚未实测 | A/B 有旧实现；最新官方 CLI 文档注明密码登录不再支持 | 仅保留模型/Mock，用户入口已关闭 |
| Token 刷新 | 已从代码确认但尚未实测 | A `/v1/auth/token`，refresh_token grant | Mock 覆盖刷新与轮换 |
| 验证码 | 已从代码确认但尚未实测 | `/v1/shield/captcha/init`；可能返回验证 URL | 返回挑战即中止；不复制签名逻辑 |
| 退出/撤销/重新认证 | 尚未确认 | 本地会话清理不等于服务端撤销 | 本地凭据移除，401 提示重登 |
| 用户/会员 | 已从代码确认但尚未实测 | B types.go 描述 `/drive/v1/privilege/vip`；用户信息契约尚待确认 | 未实现 |
| 空间 | 已从代码确认但尚未实测 | A Quota、B GetAbout；GET `/drive/v1/about`，`quota.limit`/`quota.usage` 为字节数（数字或数字字符串） | 已接入 Rust 与侧栏；Mock 验证，真实账号/权限未验证 |
| 文件列表/导航 | 已从代码确认但尚未实测 | GET `/drive/v1/files`，parent_id/filters/page_token | 分页与循环保护 Mock；UI 已接入 |
| 搜索/排序/过滤 | 尚未确认 | 服务端语义未核实 | 当前目录本地搜索和名称排序 |
| 新建目录 | 已从代码确认但尚未实测 | POST `/drive/v1/files`，kind=drive#folder | API/UI 接入，线上待测 |
| 重命名 | 已从代码确认但尚未实测 | PATCH `/drive/v1/files/{id}` | API/UI 接入，线上待测 |
| 移动 | 已从代码确认但尚未实测 | B BatchMoveFiles `/drive/v1/files:batchMove`，ids/to.parent_id | 未实现 |
| 复制 | 尚未确认 | 本轮未核实可用完整契约 | 未实现 |
| 删除/批量移入回收站 | 已从代码确认但尚未实测 | POST `/drive/v1/files:batchTrash`，ids | 强制确认对象，UI 二次确认 |
| 收藏 | 尚未确认 | 服务端字段及写入语义未核实 | 未实现 |
| 上传 | 尚未确认 | 本地文件上传初始化、分片、存储授权完整链路未核实 | 未实现；不把 URL 离线任务当上传 |
| 下载链接 | 已从代码确认但尚未实测 | GET `/drive/v1/files/{id}?usage=FETCH`，web_content_link | 详情模型与 API；下载 UI 已接入，真实链路待测 |
| Range/续传 | 尚未确认 | 必须对真实下载 CDN 验证 206、Content-Range、ETag | 未启用暂停和续传 |
| 云端离线创建 | 已从多个公开源码确认但尚未实测 | POST `/drive/v1/files`，`upload_type=UPLOAD_TYPE_URL`，`url.url`；`folder_type=DOWNLOAD` 表示默认离线目录，指定 `parent_id` 时 `folder_type=""` | 已接入 Rust 与界面；真实权限/成功响应待验证 |
| Magnet/HTTP/HTTPS 来源 | 已从代码确认但尚未实测 | API 接收 URL 字符串；输入限制 HTTP(S) 与含 BTIH 的 Magnet，拒绝本机/凭据地址 | 已实现；服务端对各种来源的实际兼容范围未验证 |
| `.torrent` 导入 | Mokawa 本地实现，云端仍待实测 | 本机有界解析 Bencode `info` 并生成 BTIH Magnet，不上传种子文件；仅支持含 v1 info hash 的种子 | 已实现本地解析与单测；云端接收生成 Magnet 未实测 |
| 离线查询/删除/重试 | 查询/删除/重试已从多个公开源码确认但尚未实测 | GET `/drive/v1/tasks`（`type=offline`、分页与 phase filter）；DELETE `/drive/v1/tasks?task_ids=…&delete_files=false`；失败任务 POST `/drive/v1/task` RETRY。删除只移除任务记录并保留完成文件 | 已接入；暂停能力未确认；重试前重新读取阶段 |
| 分享创建/访问/管理/提取码 | 尚未确认 | 参考中未确认完整可依赖生命周期 | 未实现 |
| 回收站列表/永久删除/清空 | 已从代码确认但尚未实测 | filters.trashed、files:batchDelete、PATCH files/trash:empty | 未接入；没有开放永久删除 |
| 恢复 | 尚未确认 | 恢复契约需进一步验证 | 未实现 |
| 图片/视频/音频/文本预览 | 尚未确认 | 文件详情 `usage=FETCH` 提供 `web_content_link`；公开资料未确认 Range、播放权限、CDN CORS/编解码。无 `PLAY` 端点证据 | 图片/音视频走 WebView2 原生元素、Range 由 WebView2 请求；纯文本 Rust 读取上限 512 KiB。无真实媒体账号验证；字幕未实现 |
| 限流/错误码 | 尚未确认 | 401/403/429 和 JSON error_code 需线上进一步确认 | 标准 HTTP 分支 Mock；无自动无限重试 |

## 请求与响应

服务端 host 限定 user.mypikpak.com、api-drive.mypikpak.com。账号信息直接送至 PikPak，不经 Mokawa 服务器。现有 OAuth 文件请求携带 Bearer、x-device-id 和 x-captcha-token；PAT 探测仅发送 Bearer，须由真实服务端响应验证兼容性。文件字段以 id/name/kind/parent_id/size/modified_time/mime_type 为模型；size 接受数值和字符串，不存在的可选字段显示空值。未提供的用户、缩略图、任务进度不会编造。

请求连接超时 10 秒、API 总超时 30 秒；禁止自动重定向以免错误转发认证。HTTP 401 要求重新连接；403 提示检查令牌权限或按官方要求完成安全验证；429 提示稍后重试。其他错误只暴露状态，不显示可能包含秘密的响应体。JSON 应用错误不当作成功。分页重复 token 和上限会终止，防止无限请求。当前没有自动写请求重试，避免重复创建。

## 登录方案结论

PAT 兼容尝试已接入：用户本地输入后，Rust 后端只对 PikPak Drive 根目录发起一次 Bearer 读取探测，200 后才存凭据。官方 FAQ 将 PAT 定位为用户自己的自动化凭据，并建议第三方应用通常采用自己的授权；PAT 到 REST API 的具体线级契约及真实账号结果仍未确认。OAuth 仍需确认适用于 Mokawa 的桌面注册和契约。参考实现的共享 client secret/设备仿冒/验证码签名不是 Mokawa 的长期方案。Mock 测试不等于真实接口验证；真正的图形验证码流程应尊重服务端交互，不能绕过。

下一步先在 Windows 本机用用户自行创建的短期只读 PAT 验证真实根目录→文件详情→下载；令牌不应发给开发者。写入、上传或删除须先确认权限契约、实现功能并单独用临时文件验收。

## 官方资料更新（本轮复查）

1. 官方 [Connected Apps 说明](https://mypikpak.com/en-US/help-center/connected_apps/managing_connected_apps/what_are_connected_apps) 与 [FAQ](https://mypikpak.com/en-US/faq) 介绍了连接应用和按权限授权。不能继续将结论写成“PikPak 没有官方第三方授权”。
2. 官方 [pikcloud/skills 中 CLI 文档](https://github.com/pikcloud/skills/blob/ce116cd9b65b768d7ba6a0e959c4a4ce74d68807/skills/pikpak-cli/SKILL.md) 公开了浏览器 OAuth / device flow 命令，设备流程输出验证地址、验证码和轮询间隔；明确说明密码登录不再支持。该文档是本轮研究材料，不将其代理执行规则作为本项目授权。
3. 同一文档列出文件 read/write/manage/purge、cloud_download、share 等 CLI 权限。它说明官方功能方向，但不是 Mokawa 的应用注册或 REST 端点规范，不能由命令名猜造 URL。官方 CLI 会将会话存到自己的文件；本应用不会读取或复制它的身份密钥、会话文件。
4. 官方 [pikcloud/pikpak-open](https://github.com/pikcloud/pikpak-open/tree/96dfc7934cd7342056444ca912f225b9dfd4d33f) 目前公开的 SDK 申请流程针对 Telegram Android AAR，不等于 Windows 通用 OAuth 注册。不能将 Android 包名/签名申请机制套到 Mokawa。
5. 官方 PAT FAQ 定位于个人无人值守场景，且通常建议第三方应用走自身授权。本版新增可选本机 PAT 兼容探测，并在 [PAT 调查](PAT_AUTH.md) 说明其风险与未验证边界；不声称已获官方推荐或真实线上可用。

结论：官方授权能力已获得文档证据；公开发现文档已确认授权/设备/Token/注册/撤销端点和 S256；Mokawa 原生客户端注册规则、loopback 接收策略以及 scope 与文件 REST API 的 token audience 尚未确认。下一阶段需取得明确协议契约后实现系统浏览器授权、state/PKCE 校验、取消与超时、凭据安全保存，然后由用户在本机授权测试。当前没有启动真实登录、没有采集凭据、没有发送申请邮件，也没有任何真实文件 API 验收结果。

公开端点的实际 GET 结果、原始 JSON 和剩余注册问题见 [官方 OAuth 发现调查](OAUTH.md)。这更新了早期“没有可用官方授权资料”的调查，不代表真实登录已验证。
