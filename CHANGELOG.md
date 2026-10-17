# Changelog

## Vue / Tauri 迁移 — 开发中

保留 GPUI 和 Rust 核心，新增 Vue 3 / TypeScript / Tauri 2 Windows 入口。中文、自定义标题栏、浅深/系统主题、离线 Noto Sans SC、本地头像及背景个性化、独立透明度/模糊/亮度、设置持久化。文件列表虚拟化，搜索排序 Worker，复用文件/下载 API；官方授权注册仍阻塞，新用户暂不能登录。

新增图片与 API 元数据上限、Blob URL/Worker/事件清理和前端回归测试；新增 Windows 原生 WebView2 安装后验收、进程树资源采样及依赖许可证清单。当前并非完整客户端正式发布。

## 0.1.0 — GPUI 开发基线

初始 GPUI 窗口、PikPak API 基础、系统凭据层、Mock 测试及 Windows 构建/安装脚本。尚未达到完整客户端验收标准；真实账号与 Windows 本机验收未完成。

本轮：将 GPUI 隔离到 Windows 目标，修复实际 Windows UI 编译诊断，新增账号/任务中心/设置页与两任务下载、偏好持久化和退出会话竞争保护。加入安装后原生窗口诊断、产品版本资源和 MSVC 静态运行库检查。官方 OAuth 发现信息已读取并记录；旧密码登录用户入口已关闭，正式授权接入仍待完成。
