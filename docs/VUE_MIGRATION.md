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
