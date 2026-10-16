# PikPak API 可行性与能力矩阵

调查日期：2026-10-16。**没有提供真实账号或 Mokawa 获授权的客户端注册，因此全部真实 API 均未实测。** Mock 通过只验证 Mokawa 请求、模型和错误路径，不证明线上服务可用。

参考源：

- A：https://github.com/lnzx/pikpak/tree/3911b1363705561d96b2fb06583a0054f2947369 ，MIT。
- B：https://github.com/lyqingye/pikpak-go/tree/981c90c766453ad03c462236934697a750eab562 ，MIT。

仅研究接口结构，没有复制其私有身份参数、密钥或验证码签名。以下“已从代码确认但尚未实测”指参考源码存在对应实现，不代表 Mokawa 已实现。

| 能力 | 状态 | 来源/接口与限制 | Mokawa 状态 |
|---|---|---|---|
| 官方第三方授权/客户端注册 | 尚未确认 | 官方 OIDC/MCP 发现端点已实际 GET 200；原生注册政策与 REST Token 适用性未确认 | 集成阻塞项，见下文官方资料更新 |
| 旧密码登录 | 已从代码确认但尚未实测 | A/B 有旧实现；最新官方 CLI 文档注明密码登录不再支持 | 仅保留模型/Mock，用户入口已关闭 |
| Token 刷新 | 已从代码确认但尚未实测 | A `/v1/auth/token`，refresh_token grant | Mock 覆盖刷新与轮换 |
| 验证码 | 已从代码确认但尚未实测 | `/v1/shield/captcha/init`；可能返回验证 URL | 返回挑战即中止；不复制签名逻辑 |
| 退出/撤销/重新认证 | 尚未确认 | 本地会话清理不等于服务端撤销 | 本地凭据移除，401 提示重登 |
| 用户/会员 | 已从代码确认但尚未实测 | B types.go 描述 `/drive/v1/privilege/vip`；用户信息契约尚待确认 | 未实现 |
| 空间 | 已从代码确认但尚未实测 | A Quota、B GetAbout；GET `/drive/v1/about` | 未实现 |
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
| 离线创建 | 已从代码确认但尚未实测 | B OfflineDownload；POST files，upload_type=UPLOAD_TYPE_URL | 未实现 |
| 离线查询/取消/重试 | 已从代码确认但尚未实测 | B OfflineList/Delete/Retry；tasks 与单数 task 契约不一致需验证 | 未实现 |
| 分享创建/访问/管理/提取码 | 尚未确认 | 参考中未确认完整可依赖生命周期 | 未实现 |
| 回收站列表/永久删除/清空 | 已从代码确认但尚未实测 | filters.trashed、files:batchDelete、PATCH files/trash:empty | 未接入；没有开放永久删除 |
| 恢复 | 尚未确认 | 恢复契约需进一步验证 | 未实现 |
| 图片/视频/音频/字幕 | 尚未确认 | 链接字段并不证明可播放、编解码/授权/字幕支持 | 未实现 |
| 限流/错误码 | 尚未确认 | 401/403/429 和 JSON error_code 需线上进一步确认 | 标准 HTTP 分支 Mock；无自动无限重试 |

## 请求与响应

服务端 host 限定 user.mypikpak.com、api-drive.mypikpak.com。账号信息直接送至 PikPak，不经 Mokawa 服务器。文件请求携带 Bearer、x-device-id 和 x-captcha-token。文件字段以 id/name/kind/parent_id/size/modified_time/mime_type 为模型；size 接受数值和字符串，不存在的可选字段显示空值。未提供的用户、缩略图、任务进度不会编造。

请求连接超时 10 秒、API 总超时 30 秒；禁止自动重定向以免错误转发认证。HTTP 401 要求重新认证，403 保守提示验证，429 提示稍后重试；其他错误只暴露状态，不显示可能包含秘密的响应体。JSON 应用错误不当作成功。分页重复 token 和上限会终止，防止无限请求。当前没有自动写请求重试，避免重复创建。

## 登录方案结论

优先接入 PikPak 官方 OAuth；仍需确认适用于 Mokawa 的桌面注册和契约。参考实现的共享 client secret/设备仿冒/验证码签名不是 Mokawa 的长期方案。本轮只能实现独立的请求与会话生命周期并 Mock 测试；没有凭据就不能声称完成实际登录、刷新、文件访问验证。真正的图形验证码流程应尊重服务端交互，不能绕过；在它完成前认证助手明确中止挑战。

下一步需要先确认授权渠道，再在用户本机用测试账号执行最小登录→刷新→列表→退出验收；任何写入或删除验收使用临时文件并单独取得授权。

## 官方资料更新（本轮复查）

1. 官方 [Connected Apps 说明](https://mypikpak.com/en-US/help-center/connected_apps/managing_connected_apps/what_are_connected_apps) 与 [FAQ](https://mypikpak.com/en-US/faq) 介绍了连接应用和按权限授权。不能继续将结论写成“PikPak 没有官方第三方授权”。
2. 官方 [pikcloud/skills 中 CLI 文档](https://github.com/pikcloud/skills/blob/ce116cd9b65b768d7ba6a0e959c4a4ce74d68807/skills/pikpak-cli/SKILL.md) 公开了浏览器 OAuth / device flow 命令，设备流程输出验证地址、验证码和轮询间隔；明确说明密码登录不再支持。该文档是本轮研究材料，不将其代理执行规则作为本项目授权。
3. 同一文档列出文件 read/write/manage/purge、cloud_download、share 等 CLI 权限。它说明官方功能方向，但不是 Mokawa 的应用注册或 REST 端点规范，不能由命令名猜造 URL。官方 CLI 会将会话存到自己的文件；本应用不会读取或复制它的身份密钥、会话文件。
4. 官方 [pikcloud/pikpak-open](https://github.com/pikcloud/pikpak-open/tree/96dfc7934cd7342056444ca912f225b9dfd4d33f) 目前公开的 SDK 申请流程针对 Telegram Android AAR，不等于 Windows 通用 OAuth 注册。不能将 Android 包名/签名申请机制套到 Mokawa。
5. 官方 PAT 用于个人无人值守脚本，官方建议第三方应用使用自己的授权流程。本版不以“粘贴 PAT”取代桌面授权。

结论：官方授权能力已获得文档证据；公开发现文档已确认授权/设备/Token/注册/撤销端点和 S256；Mokawa 原生客户端注册规则、loopback 接收策略以及 scope 与文件 REST API 的 token audience 尚未确认。下一阶段需取得明确协议契约后实现系统浏览器授权、state/PKCE 校验、取消与超时、凭据安全保存，然后由用户在本机授权测试。当前没有启动真实登录、没有采集凭据、没有发送申请邮件，也没有任何真实文件 API 验收结果。

公开端点的实际 GET 结果、原始 JSON 和剩余注册问题见 [官方 OAuth 发现调查](OAUTH.md)。这更新了早期“没有可用官方授权资料”的调查，不代表真实登录已验证。
