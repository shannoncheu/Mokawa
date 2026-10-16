# Mokawa 安全说明

- 仅将账号凭据发送给 PikPak 固定认证服务；不包含 Mokawa 中转服务器。
- 不内置第三方客户端密钥、验证码签名或访问限制绕过逻辑。
- 密码隐藏输入且不持久化。Token 由 Windows Credential Manager 保护；没有明文回退。系统凭据库不防御已控制当前用户会话的恶意软件。
- Session Debug 脱敏，应用错误不携带 reqwest URL/body，日志不用 wire tracing。仍需 Windows 凭据尺寸限制及日志人工验收。
- 文件 ID 和名称校验；批量移入回收站必须确认。永久删除、覆盖尚未开放。
- 下载仅 HTTPS，不发送 PikPak Bearer 到下载 host；独占临时文件、拒绝 Windows 保留名、拒绝覆盖。下载基础尚未接入 UI。签名 URL 不进队列。HTTP Range 未验证、未启用。
- API 有连接/总超时；分页有循环保护；无无限自动重试。不对非幂等操作盲目重试。
- 非官方 API 存在账号风控、兼容性、会员能力和服务条款风险。需要授权和真实测试才能正式交付。

报告漏洞时不要附密码、Token、Cookie、验证码、签名 URL 或未经脱敏的日志。
