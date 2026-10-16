# PikPak API 可行性与能力矩阵

调查日期：2026-10-16。**没有提供真实账号或 Mokawa 获授权的客户端注册，因此全部真实 API 均未实测。** Mock 通过只验证 Mokawa 请求、模型和错误路径，不证明线上服务可用。

参考源：

- A：https://github.com/lnzx/pikpak/tree/3911b1363705561d96b2fb06583a0054f2947369 ，MIT。
- B：https://github.com/lyqingye/pikpak-go/tree/981c90c766453ad03c462236934697a750eab562 ，MIT。

仅研究接口结构，没有复制其私有身份参数、密钥或验证码签名。以下“已从代码确认但尚未实测”指参考源码存在对应实现，不代表 Mokawa 已实现。

| 能力 | 状态 | 来源/接口与限制 | Mokawa 状态 |
|---|---|---|---|
| 官方第三方授权/客户端注册 | 尚未确认 | 未找到已确认可用于 Mokawa 的注册和授权契约 | 阻塞项 |
| 密码登录 | 已从代码确认但尚未实测 | A client.go，B api.go；user.mypikpak.com `/v1/auth/signin` | 模型与 Mock；仅接受获授权注册 |
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
| 下载链接 | 已从代码确认但尚未实测 | GET `/drive/v1/files/{id}?usage=FETCH`，web_content_link | 详情模型与 API；下载 UI 未接入 |
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

优先等待 PikPak 官方授权或确认适用于 Mokawa 的注册。参考实现的共享 client secret/设备仿冒/验证码签名不是 Mokawa 的长期方案。本轮只能实现独立的请求与会话生命周期并 Mock 测试；没有凭据就不能声称完成实际登录、刷新、文件访问验证。真正的图形验证码流程应尊重服务端交互，不能绕过；在它完成前认证助手明确中止挑战。

下一步需要先确认授权渠道，再在用户本机用测试账号执行最小登录→刷新→列表→退出验收；任何写入或删除验收使用临时文件并单独取得授权。
