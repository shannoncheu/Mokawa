# Vue / Tauri 迁移计划

恢复基线：`4be782413a558e810ce1aee96aebde3cb13f13fc`，工作区检查时干净。旧 GPUI 源码、Cargo feature、测试与 Windows 工作流保留；新入口为 `web/`、`src-tauri/`，独立锁文件，不替换 Rust 核心库。

本次采用 Vue 3 + TypeScript + Vite + Tauri 2 稳定系列。原因是个性化图片分层、CSS 设计 Token、滑块和减少动态效果能以较少平台耦合实现。新增风险为 WebView2 运行时、IPC、CSP、图片解码与前端事件生命周期；不是仅生成网页。只发布 Windows x64。

1. 审查与保留：复用 `mokawa` crate API/auth/models/credentials/transfers；不迁移密钥到 JS。参考用户附图，指定 design 图片不存在时继续工作。
2. 安全桥接：只注册具体业务命令，不开放任意 shell、文件系统、URL、HTTP 或令牌读取。前端只得到非敏感文件 DTO。取消页面请求与持续下载分别管理。
3. Vue 设计系统：中文、自定义窗口控制、统一浅/深/系统 Token，系统监听和动画尊重 reduced-motion，列表/网格虚拟化。
4. 个性化：本地选择器由 Rust 打开，限制文件字节/像素/分配，解码重采样为固定管理文件；头像和背景各一份，UI URL 换图/销毁时撤销；前端滑块实时，写盘防抖并串行保留最新设置。
5. 登录/文件：现有真实 API 调用继续；官方客户端注册仍阻塞时明确提示，禁止假登录或密码方案。无账号不填充演示文件/容量。
6. 验证：Rust 核心与图片限制测试、前端测试、浏览器 UI 测试（仅浏览器范围）、Windows Tauri 编译/安装包和原生窗口验证。测试状态分别记录，不把浏览器测试当 Windows 成功。

图片透明度、面板不透明度、遮罩不透明度和应用内 CSS 模糊独立设置。窗口保持不透明；不声称实现 Mica/Acrylic。内容面板保留最低安全不透明度，文字不跟着背景一起透明。

Noto Sans SC 从 Fontsource 本地随包提供，OFL-1.1；无在线字体请求。系统字体回退 Microsoft YaHei UI / Segoe UI / sans-serif。记录实际打包体积及字体通知。

## 阶段记录（2026-10-17）

### 仓库检查与迁移评估

执行 `git status --short`、`git log`、`git remote -v`、检查 Cargo 清单及 `src/ui/`、认证与传输代码。开始时工作区干净，远程不变。保留 GPUI 基线及工作流，并以独立 Tauri crate 复用根 Rust 库。指定参考图不在仓库，按任务附图和规范实施，没有生成演示网盘数据。GPUI 的 Windows 专用依赖隔离保留，不引入 Linux UI，也没有为 xattr 降级整个依赖树。

### 安全桥接与设计系统

新增 `src-tauri/{Cargo.toml,Cargo.lock,tauri.conf.json,capabilities/main.json,src/app.rs,src/store.rs}`；复用 core API、系统凭据、下载中心、取消与持久化。前端 `web/src/` 使用统一 CSS Token、中文导航、自定义标题栏、浅色/深色/系统主题、模态焦点管理及键盘交互。新入口不依赖 GPUI，不删除旧入口。

### 个性化中心

`src/personalization.rs` 负责输入字节、解码尺寸、分配限制、方向修正及重采样；管理图片各一份。设置写入串行，450ms 防抖，保存失败可手动重试；关闭前等待保存。`web/src/useAppearance.ts` 管理系统主题监听、可取消图片加载、Blob URL 撤销和有界背景交叉淡入。背景、遮罩、面板透明度独立，窗口和文字不透明。字体离线分发，带 OFL 文本。

### 文件管理与真实业务

`App.vue`、`VirtualFiles.vue` 接入真实 Rust 文件请求、面包屑、新建、重命名、确认删除和下载；异步代次防止旧结果覆盖。列表/网格虚拟化，搜索防抖 160ms；Worker 缓存单目录排序索引，最多一个执行中请求加一个最新待处理请求，退出/清空释放。修复筛选后旧滚动位置产生空白的问题。

会话只有在真实文件请求成功后才进入文件页。没有 Mokawa 官方客户端注册和回调契约，授权按钮明确报未接入，不提供密码收集、Token 导入或模拟成功。空间用量、账号会员信息、上传、移动/复制、收藏、离线任务、分享、完整回收站和媒体预览没有伪装成可用。

### 本地实际执行

- `npm ci`、`npm run build`、`npm run format:check`：前端生产编译和格式检查通过。
- `npm test`：10 项通过，包括真实业务交互的 Mock、删除确认、会话失效、设置恢复/防抖/失败重试、图片 URL 回收、Worker 合并和终止，以及虚拟列表生命周期/滚动回归。
- `npm run test:ui`：5 项浏览器测试通过，包括中文登录、两主题和系统主题变化、重载保存、窄窗口、极端背景对比度、十万元数据 Worker 测试。这是 Linux Chromium 范围，不代表 Windows 原生测试。
- `cargo test --locked --all-targets`：Linux 26 项通过（7 单元 + 19 核心/Mock）；Windows 专用测试不在 Linux 执行。
- `cargo fmt --check`、核心 `cargo clippy --locked --all-targets -- -D warnings` 和 Tauri 清单格式检查单独执行。Linux 不编译 Tauri Windows 模块，不据此声称原生构建通过。

中间问题均保留记录：TypeScript 7 与当前 vue-tsc 不兼容，固定到实际通过的 5.9.3；Windows 编译发现异步命令持借用 State 时必须返回 Result，已修正；关闭窗口最初缺少受限 `allow-destroy` 权限，实测发现后补齐；生成的许可证资源须在 Rust 原生测试前生成；物理拖动测试需等待 IPC 进入原生拖动循环，不能按下后立即瞬移鼠标。失败的中间 CI 不计成功。

Windows 最终结果与剩余人工验收在下方记录。完整客户端仍受真实授权阻塞；没有宣称完成所有 P0/P1/P2 或正式发布。
