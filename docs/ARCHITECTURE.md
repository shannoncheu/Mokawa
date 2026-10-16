# 架构与分阶段实施计划

UI（GPUI entities / input）→ 业务（auth/files/state/download）→ API（reqwest）→ PikPak。
只有 PikPak，没有 Provider trait 或多网盘适配层。

- `src/auth.rs`：获授权客户端参数、登录挑战、Token 生命周期；Session Debug 脱敏。
- `src/api.rs`：固定服务端、超时、HTTP/JSON 错误分类，不记录 body。
- `src/models.rs`：API 文件响应与本地传输记录。
- `src/files.rs`：输入校验、分页、新建/重命名、确认后的批量删除。
- `src/credentials.rs`：Windows Credential Manager；非 Windows 明确失败，不写明文兜底。
- `src/state.rs`：列表过滤和排序、选择、路径、异步请求 generation 隔离。
- `src/download.rs`：单次下载/取消/进度/冲突与队列保存基础，未接 UI、未支持 Range。
- `src/ui`：原生布局、输入、交互。Tokio 运行网络工作，GPUI 弱实体异步更新。
- `src/bin/auth.rs`：Windows 终端隐藏输入认证助手，尚未替代完整图形登录。

没有实现的 upload/offline/shares/media 不创建空壳模块伪装功能完整。计划在端点与生命周期确认后独立实现。

## 实施门槛

1. 调查/基础工程：记录 GPUI 与 PikPak 的可行性、固定版本、核心 Mock 与安全边界。
2. 窗口基础：Windows CI 编译和生成安装包；Windows 本机实际运行确认输入、异步、DPI、窗口关闭。云端 UI 源码检查不能替代此步骤。
3. P0：合法授权与挑战处理、真实会话/空间/文件操作、下载队列 UI、本地目录选择与通知、右键/快捷键/焦点；对临时文件真实验收。当前尚未达到该门槛。
4. P1：先验证上传协议和 CDN Range，再实现并发、持久化、恢复、离线任务、分享、回收站完整交互，逐项补 Mock/真实测试。
5. P2：评估媒体方案。候选 Media Foundation（系统 API，需 GPUI 视频帧与 GPU 互操作）和 libmpv（需评估动态链接许可、DLL 分发、体积）。未选定或声称可播放。之后托盘、通知、主题、国际化等。
6. 发布：安装/卸载实测、依赖许可证审计、签名证书（如可用）、验收清单；门槛未通过不得宣布正式发布。

退出时取消后台请求；不会继续静默执行未确认的危险任务。修改请求若已到达服务端，取消不能保证撤销，因此需刷新核对。当前 UI 只允许一个网络操作进行中。
