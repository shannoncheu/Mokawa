# 性能与资源生命周期

本文件是持续验收要求，不代表所有条目已通过。Rust 内存安全不保证资源不会泄漏。

## 当前审查与优先级

| 优先级 | 风险 | 本次处理/状态 |
|---|---|---|
| P0 | future 被中止后下载登记不释放，退出等待残留 | RAII 登记释放；中止回归测试 |
| P0 | 排队任务和历史、错误表无限增长；重复 ID 覆盖取消句柄 | 活动含排队最多 32，历史最近 200；对应错误同时淘汰；拒绝重复 ID并返回错误 |
| P0 | 下载被强制 abort 时临时文件可能残留 | 阻塞创建操作内部立即建立 RAII 路径所有者；中止/正常退出删除临时文件，清理失败输出不含路径的警告；新增中止与文件冲突测试，Windows 待验证 |
| P1 | 空闲任务页每 250ms 重绘，所有页面持续计时 | watch 事件通知；有变化时才启动 100ms 合并计时；窗口释放取消订阅等待 |
| P1 | 每次渲染构建所有文件行和克隆完整文件 | GPUI 0.2.2 官方 uniform_list，仅为可见范围建行；排序改为索引和预计算小写键，搜索 180ms 防抖，旧防抖任务随新输入取消；排序缓存仍待加入 |
| P1 | 默认后台线程数随 CPU 数增长 | Tokio 2 个工作线程、最多 8 个阻塞线程 |
| P1 | 慢连接长期占用下载槽 | 每次流读取 30 秒超时；总超时仍为 24 小时，连接 10 秒；无自动无限重试 |
| P1 | 进度逐网络 chunk 通知过密 | 最多约每 100ms 通知，完成始终发送最终值 |
| P1 | 磁盘历史文件无限制读入 | 最多读取 2MiB+1，超限明确报错；恢复最多 200 条 |
| P1 | 下载链接准备任务在进入队列前尚无统一准入 | 目录选择开始即取得 32 个总准入名额之一，准备阶段额外最多 4 个并发；令牌随任务释放，会话退出取消等待 |
| P1 | API 响应及文件分页聚合占用较大 | 尚待加入字节和列表总量预算；当前分页有循环检测和页数上限 |
| P2 | 页面切换不自动取消所有页面请求 | 窗口销毁/退出已有取消；需要细分只读页面请求和不可随意中止的写操作 |

UI Context 的异步回调使用弱 Entity；订阅保存在视图字段中。新增通知等待仅捕获弱视图、receiver 和窗口取消令牌；窗口 Drop 发出取消。下载持续跨页面运行，账号退出取消下载，不因普通页面导航取消。尚未通过 Windows Entity 反复创建销毁测试证实全部上游对象释放。

当前无缩略图或媒体缓存，不应声称已验证其容量。当前日志输出到控制台，没有无限增长的磁盘日志文件。队列只持久化非机密元数据；中止时内存标为 Interrupted，磁盘若仍为 Running，下次恢复同样改为 Interrupted。

## 可重复命令

```
cargo fmt --check
cargo test --locked --all-targets
cargo clippy --locked --features desktop --all-targets -- -D warnings
cargo build --locked --release --features desktop --bins
./scripts/windows-performance.ps1 -Executable ./target/release/mokawa.exe -Seconds 60 -Cycles 3
```

Linux 不编译 GPUI，不能代替 Windows 验证。脚本打开真实空闲任务页，测量进程 Working Set、Private Bytes、单核百分比 CPU、线程、句柄；每轮请求关闭窗口并等待退出，超时失败。CI 采用每轮 15 秒、3 轮短采样，导出 CSV/JSON，不能代替长期测试或证明组件无泄漏。每轮是新进程，因此不能用跨轮差异证明同进程内存回收。

正式预算必须来自同版本 Windows 10/11、固定 DPI/显示器、GPU/驱动、电源策略和文件规模测量；目前未设臆测的内存阈值。记录构建 SHA、系统版本和上述环境，预热后分别测试空闲 1 小时及 24 小时、1千/1万/10万文件、目录反复导航、任务完成/取消/失败、断网和休眠唤醒。真实认证尚未具备时，相关场景标为未验证；合成数据须为明确的开发测试，不进入产品展示。

使用 PerfMon 每秒采集 Process 实例的 Working Set、Private Bytes、% Processor Time、Thread Count、Handle Count，注意 PID 对应。异常持续增长时用 WPR/WPA 记录 CPU、文件 I/O、分配和句柄；比较多轮相同负载后趋于平台的缓存与持续增长。Working Set 上升可能是驻留变化；Private Bytes 不回落也可能是分配器保留，需要分配栈、活跃任务/缓存计数和句柄趋势进一步定位。不能只凭任务管理器判断泄漏。

## 测试覆盖边界

已新增：32 任务准入压力、重复 ID 拒绝、协作取消并等待空闲、future 中止后登记回收、历史和错误表容量、空闲无状态通知、超大持久化文件拒绝。

尚待执行/实现：大文件流式下载内存预算、网络断开与读取超时的实测、强制中止临时文件清理的 Windows 验证、GPUI Entity 生命周期压力、文件列表渲染耗时及高 DPI、休眠唤醒、真实 Token 过期和重新认证、24 小时 soak、上传及媒体资源生命周期。现有下载使用流式响应而非整体读入，但尚无证明单连接最大内存预算的压力数据。

存在可复现资源残留或持续异常增长时阻止正式发布；当前仍是功能不完整的预览版本。

## 2026-10-17 实际验收记录

代码：`df86508f41e3a2201a6d4ffe7d2410b2753a4030`。
Windows Server 2022 GitHub Runner：[39006709784](https://github.com/shannoncheu/Mokawa/actions/runs/39006709784)，全部成功。此环境不是 Windows 10/11 实机。

修改文件：`src/transfers.rs`（准入、RAII、事件通知和测试）、`src/download.rs`（临时文件所有权、超时、节流、持久化读上限和测试）、`src/state.rs`（排序索引）、`src/ui/mod.rs`（虚拟列表、防抖、运行时预算）、`src/ui/pages.rs`（通知订阅、准备并发）、`tests/core.rs`、`scripts/windows-performance.ps1`、`.github/workflows/windows.yml`、本文件及基线产物。

实际执行：`cargo fmt --check`；`cargo test --locked --all-targets`；`cargo check --locked --features desktop --all-targets`；`cargo clippy --locked --features desktop --all-targets -- -D warnings`；`cargo build --locked --release --features desktop --bins`；NSIS 构建、静默安装、`scripts/windows-smoke.ps1`、`scripts/windows-performance.ps1 -Seconds 15 -Cycles 3`、静默卸载。Windows **22 项测试通过**（4 单元 + 17 核心/Mock + 1 系统凭据测试）。本地 Linux 21 项通过，Windows 凭据测试按平台不运行；不以此替代 Windows UI 编译。

五个真实原生窗口场景通过：登录、文件、任务、设置及小窗口；含工作区尺寸检查。真实账号仍未接入，空文件页面测试不能验证大量文件实际渲染。三轮任务页空闲采样均能正常关闭退出。

| 轮次 | 最大工作集 MiB | Private Bytes 首→尾 MiB | 最大线程数 | 句柄首→尾 |
|---|---:|---:|---:|---:|
| 1 | 33.40 | 27.13 → 27.08 | 15 | 297 → 297 |
| 2 | 33.43 | 27.30 → 27.22 | 16 | 298 → 297 |
| 3 | 33.32 | 26.97 → 26.88 | 16 | 298 → 297 |

三轮每秒进程 CPU 时间差均测得 0，受短采样和计时分辨率限制，不能解释为绝对零 CPU。没有观察到本次短时采样内 Private Bytes 或句柄持续增长；不构成零泄漏证明。原始 CSV/JSON：`docs/performance/windows-2022-idle-df86508.*`。正式 Windows 10/11 内存/CPU/响应预算仍待长时间和实际负载测量后确定，不将这些空载值用作所有任务的硬性上限。

本地 10 万文件索引筛选/排序实测约 19.5ms（debug 构建、单次、Linux；不是 GPUI 帧耗时）。新增测试曾触发 `field_reassign_with_default` 严格 lint，已修复并重新通过。两次中间 CI 为节省资源主动取消，不计成功；最终上述 run 是最新代码的完整验证。

下一阶段：有界 API 响应与分页总量、文件索引缓存、同进程 Entity 压力测试、流式大文件和网络中断压力测试、Windows 10/11 一小时及 24 小时资源曲线。真实认证、休眠唤醒、高 DPI、媒体/上传生命周期和完整 P0/P1 仍未验证，不进入正式发布。

## Vue / Tauri 资源审查

优先级与处理：

| 优先级 | 风险 | 本轮处理 |
|---|---|---|
| 高 | 事件、图片 URL、Worker 或定时器跨组件残留 | 统一卸载清理；异步订阅注册竞态也释放；图片按类型 AbortController/代次；最多一个退休背景 URL，250ms 撤销；Worker 清空/销毁终止 |
| 高 | 页面旧请求覆盖新目录或退出状态 | 前后端代次、页面取消与会话凭据 epoch；退出取消下载；前端失效立即清空列表和危险对话框 |
| 高 | 图片解码和 API 元数据无界 | 图片 12 MiB/1600 万像素/分配限制；输出头像 256²/背景 2560×1600；API 响应 4 MiB、目录 10 万项/64 MiB 元数据上限及 4 KiB 分页 Token 限制 |
| 高 | 大列表排序和渲染阻塞 | Worker 单目录排序索引，转移 Uint32Array；请求合并；160ms 搜索防抖；列表/网格只挂载可见行 |
| 高 | 重复下载、连接和队列增长 | 保留核心 2 并发、32 准入、200 历史上限及流式临时文件 RAII；Rust 2 工作线程/8 阻塞线程 |
| 中 | 滑块连续写盘、空闲轮询 | 450ms 防抖、串行最新快照、关闭等待保存；任务 watch 事件更新，无空闲轮询 |
| 中 | 透明度影响文本、低端 GPU 模糊开销 | 内容面板至少 80%，文字和窗口不透明；尊重减少动态效果，可关闭模糊和背景 |
| 待实测 | Windows 10/11 长期运行、睡眠唤醒、真实网络和媒体 | 尚未验证，不承诺零泄漏或正式资源预算 |

`npm test` 包括 10 轮虚拟列表创建/销毁，20 次图片更换后旧 URL 撤销，Worker 请求合并/终止，保存防抖/无无限失败重试。`npm run test:ui` 的十万条数据仅为明确的性能测试，不进入产品云盘展示。Linux Chromium 单次索引约 120ms，期间主线程计时仍继续；不是 Windows UI 响应预算。

Windows 新入口必须统计整个 Mokawa + WebView2 进程树。`scripts/windows-tauri-performance.ps1` 采集工作集、Private Bytes、CPU 单核百分比、线程、句柄与进程数，进程成员变化时不计算该次 CPU 差值。脚本默认 3 轮独立启动、预热 3 秒、每轮 15 秒；短时采样不是同进程长时间 soak。不要将旧 GPUI 的约 33 MiB 数据套用到 WebView2 新版本。正式预算仍需 Windows 10/11、固定 DPI/硬件与同版本 1 小时/24 小时、真实文件和下载负载测试。
