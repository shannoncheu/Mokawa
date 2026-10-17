# PikPak Personal Access Token 接入调查

调查日期：2026-10-17。此文将官方文档明确内容与 Mokawa 的兼容性假设分开记录。

## 官方资料确认的内容

官方 FAQ：<https://mypikpak.com/zh-CN/faq>，问题“什么是个人访问令牌？”及其创建/管理问题说明：

- 用户在“账号与安全 → 已连接的应用 → 个人访问令牌”自行创建；创建时选择有效期和权限，完整令牌仅显示一次。
- 有效期为 30 天至 1 年，默认 90 天，没有永不过期选项。
- PAT 适用于用户自己的脚本、定时任务、NAS 和其他无人值守场景；FAQ 明确建议第三方应用通常使用自己的登录与授权流程，而不是要求用户粘贴 PAT。因此 Mokawa 将其标为**用户自持令牌的兼容尝试**，不声称它是 PikPak 推荐的第三方应用授权方式。
- 默认权限是读取文件；FAQ 明确指出 PAT 不能获得永久删除和邀请权限。FAQ 没有给出其完整 API scope 名称列表。
- 令牌可以重新生成或移除；旧令牌立即失效。移除属于用户在 PikPak 账号安全页面完成的服务端撤销。
- 生成令牌会触发 PikPak 邮件通知。连接应用另有传输额度规则，FAQ 提及免费账户有每日连接应用下行配额，以及连接应用共享月度下行配额。

官方 CLI 使用文档：<https://github.com/pikcloud/skills/blob/main/skills/pikpak-cli/SKILL.md>，给出 `PIKPAK_TOKEN` 环境变量和 `pikpak auth token <token>`，并列出 CLI 文件列表、下载、上传等命令。它也说明密码登录不再支持。其文档的 `files:read/write/manage/purge`、`cloud_download`、`share`、`invite`、`account` scopes 是 CLI OAuth 权限标识；不能据此宣称 PAT 使用相同 scope。

**官方公开资料没有给出 PAT 的线级 HTTP 规范**：没有明确声明 PAT 应发往哪个资源 audience、是否直接用 `Authorization: Bearer <PAT>`、是否需要 `x-device-id` 或 captcha header、哪些 PAT 权限控制每个 REST endpoint。官方 CLI 能接受 PAT 并操作 PikPak，证明它在官方 CLI 里可用，不足以证明 Mokawa 直接把 PAT 放进既有 Drive REST 请求就兼容。

## Mokawa 当前实现

登录页新增个人访问令牌输入。令牌只通过本机 Tauri IPC 传到 Rust；Rust 以 Bearer Token 方式向固定 PikPak Drive 主机 `https://api-drive.mypikpak.com` 发送一次 `GET /drive/v1/files`（根目录、`limit=1`、排除回收站）。PAT 请求不附带旧 OAuth 会话的设备/captcha headers，也不跟随 HTTP 重定向。

只有 API 返回成功且 JSON 可解析，应用才把令牌写入 Windows Credential Manager。验证失败不保存。令牌不写日志、配置文件或传到 Mokawa 服务；Web 输入在调用前清空，组件销毁时再清理。Credential Manager 保存的旧会话若缺少 `auth_method`，仍按 OAuth 解析。

- HTTP 200：说明该令牌在该安装和当时权限下通过一次真实文件读取验证，才可视为已连接。
- HTTP 401：按令牌无效、过期或已撤销处理；重新创建/连接令牌。PAT 不使用 OAuth refresh token。
- HTTP 403：显示权限/安全验证提示，不保存令牌。当前没有已验证的服务端错误码映射，不能精确区分缺少权限与安全验证。
- PAT 有效期没有安全可依赖的声明字段，Mokawa 不推算到期时间；由 PikPak API 响应判断，已撤销令牌也须由用户在 PikPak 页面撤销。

当前已连接的文件列表和下载流程都使用已有 API。它们在 PAT 模式下会继续发送 Bearer 请求，但**尚无真实 PAT 验收**。Mokawa 目前没有上传 API/界面，不能声称支持上传；新建文件夹、重命名、移入回收站虽已有 OAuth API 代码，其 PAT 权限要求也未验证。账号信息/userinfo 未实现。

## 兼容性与授权风险

| 客户端 | 公开实现概况 | Mokawa 不复用的原因 |
|---|---|---|
| PikPak CLI | 官方文档接受 PAT 环境变量/命令参数；支持浏览器和设备码 OAuth | PAT 的 REST header/audience 没有公开契约；CLI 客户端身份和本机凭据文件不由 Mokawa 冒用 |
| rclone PikPak backend（MIT） | 当前公开源码使用旧用户名/密码登录与 refresh-token，含 PikPak 客户端 ID、设备 ID 和 captcha tokener | 不支持/不证明 PAT；源码中的客户端身份不是 Mokawa 身份；复制身份/密码流程会产生未授权冒用与风控风险 |
| OpenList PikPak driver（AGPL-3.0） | 当前公开源码配置平台客户端参数、用户名/密码与 refresh token，并处理 captcha | 不证明 PAT；含 Android/Web/PC 客户端身份配置；复制秘密或伪装平台可能违反平台政策，且 AGPL 有分发义务 |

以上项目只用于确认公开实现路线，不复制其私有/平台身份密钥，也不复制实现代码。

## 验证状态与限制

- 自动化 Mock 覆盖：PAT 会话构造和 Debug 脱敏；以 Bearer 头发起单次根目录读取；PAT 不带旧 OAuth 设备/captcha headers；401/403 不作为连接成功；旧 Credential Manager 数据默认仍识别为 OAuth。
- 真实 PikPak PAT：尚未验证。本地环境没有测试 PAT，也没有执行任何 PAT 授权请求。Mock 成功不能证明线上兼容。
- 真实文件读取/下载：待用户在 Windows Mokawa 中输入自己生成的 PAT 并看到 API 验证成功。不要在聊天、issue、日志或 Git 中发送令牌。
- 上传：当前项目未实现，且 PAT 对写入 API 的权限契约未确认。

下一步需要 Mokawa Windows 操作者在个人账号的 PAT 管理页面创建至少“读取文件”权限的短期测试令牌，在本机 UI 进行真实验证；若 401/403，收集仅含 HTTP 状态码和已脱敏错误类别的诊断。只有读取通过后，再分别验证详情/下载及允许的写操作。不要先用生产令牌测试未确认的写入、删除或上传操作。
