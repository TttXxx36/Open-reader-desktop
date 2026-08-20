# 开发路线图

> 维护复核（2026-08-20）：main 提交 [2b5d973](https://github.com/TttXxx36/Open-reader-desktop/commit/2b5d97314caaed2be119606623c3f4358c721064) 为当前基线；PR10 [分离搜索工作区并重构书架交互](https://github.com/TttXxx36/Open-reader-desktop/pull/10) 的 head 2ac53b9 已通过 CI [31590842650](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31590842650)，但尚未合并。正式发布仍受合并后 Release/smoke、目标 Windows 手工验收和签名暂缓约束。完整未完成清单见 [2026-08-20 开发状态审计](development-status-2026-08-20.md)。

路线图按“可独立验收、可回滚、可持续兼容”的里程碑推进。每个里程碑都必须在 GitHub Issue、PR、自动化检查和变更记录中留下证据；“已实现”只表示代码与远程 CI 通过，不替代 Windows 安装包手工验收。

## 总体原则

1. **先恢复可用性，再扩展能力**：死按钮、导入失败和数据丢失属于 P0，优先于视觉和高级功能。
2. **安全分层兼容**：书源能力分为“可执行”“可导入但不执行”“明确拒绝”三类，不对 XPath、脚本、认证态做静默降级。
3. **授权优先**：只使用用户自有、授权、公开测试或公版内容；不内置侵权书源，不绕过登录、付费墙、验证码、DRM 或访问控制。
4. **Windows 优先、跨平台可演进**：Tauri + Vue 负责桌面体验，Rust 负责解析、网络边界和 SQLite；Windows Actions 是发布验证入口。
5. **发布闸门与代码开发解耦**：Windows Release 的权限、签名和安装器回归会阻塞“正式发布完成”，但不阻塞不依赖安装包的代码切片。

## 里程碑

### M0 — 范围与治理（已完成基础版）

确定产品定位、贡献规范、安全报告方式、内容合法性和隐私边界；记录 Legado 3.0 兼容目标与“安全子集优先”原则。仓库根目录已有 MIT License，但 README、CONTRIBUTING、SECURITY 和兼容性矩阵的一致性仍由 M0 issue #1 跟踪；脚本运行时和同步范围仍是后续治理决策项。

### M1 — 工程骨架（已完成）

建立 Tauri v2、Vue 3、TypeScript、Rust、SQLite、格式化、Rust/前端检查、GitHub Actions 和 Windows/WebView2 检查。

### M2 — 本地阅读 MVP（已完成基础版）

实现 TXT/EPUB 导入、解析、目录、阅读视图、书架、阅读进度、主题、字体、行距设置和离线章节数据。

### M3 — 书源协议（已完成基础版）

统一搜索、书籍详情、目录、正文数据模型；实现 HTTP 客户端、HTML/CSS、正则、JSONPath 安全解析、超时/重定向/响应上限和合成测试夹具。

### M4 — 单书源端到端（已完成）

实现书源导入、校验、调试日志和“搜索 → 详情 → 目录 → 正文”链路；仅使用本机合成授权夹具，不连接未确认授权的真实站点。

### M5 — 多源与可维护性（代码已完成）

- M5.0：启用书源并发搜索、统一结果模型、标题/作者归一化去重和单源失败隔离。
- M5.1：远程详情、目录、章节阅读与 SQLite TTL 缓存。
- M5.2：书源 JSON 导入/导出、预览和手动强制刷新。
- M5.3：正文 replaceRules、规则上限、缓存容量治理。
- M5.4：目录指纹、章节差异统计、stale 缓存回退。
- M5.5：permission 元数据、安全审计、敏感请求头审计、CSP、重定向限制和缓存可观测性。
- M5.6：翻开书籍图标、版本/产物门禁、标签发布工作流、安装器/便携包/SHA-256 清单；签名暂缓。

### M6 — Windows 体验与阅读质量（代码切片已完成，安装包回归待完成）

- M6.0：设置入口、书源文件选择、外部 JSON 形态、图标/字体和基础排版。
- M6.1：Legado 3.0 安全子集导入层，支持 BOM、包装对象、别名、URL 预览和 JSONPath。
- M6.2：TXT 段落与 EPUB blocks-v1 内容块，安全过滤图片、脚本和外链。
- M6.3：字体、字号、字间距、版心、边距、段距、首行缩进、对齐、连续/分页和自定义主题。
- M6.4：首页概览、空状态、组件拆分、焦点样式和中文 Windows 窄窗口基线。

### M6.5 — Windows 发布闸门（自动化验收已完成，人工收尾待执行）

这是发布闸门，不是后续代码开发的硬阻塞项：

- main 上的 Windows release run [31574767135](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31574767135) 已完成严格预检、Tauri 安装器构建、便携 ZIP 打包和 SHA-256 清单生成。
- installer smoke run [31575465554](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31575465554) 已通过产物校验、便携版启动、NSIS/MSI 安装/启动/卸载和数据保留检查。
- 仍需在目标 Windows 环境手工补验：双版本原地升级、WebView2 缺失提示、离线/网络错误、回滚/撤回策略、中文字体、窄窗口、键盘 Tab 焦点、高对比度和书源导入体验。
- 签名方案在用户可提供证书前继续暂缓；本轮及后续验证不在本地构建或安装。



### M6.6 — 阅读工作区视觉系统（已完成）

统一导航、书架、书源工作区、阅读工具栏、阅读页面和设置面板的墨色/暖金视觉语言；补充焦点状态、悬停反馈、响应式窄窗口和设置引导。只改变展示层，不改变解析、缓存、阅读进度或 SQLite 数据模型。

### M7 — 书源兼容性 v2（核心代码已完成，兼容收尾与人工验收待完成）

详细执行清单见 [M7 书源兼容性 v2 实施计划](m7-source-compatibility-plan.md)。 当前阶段缺口与后续顺序见 [2026-08-11 开发缺口审计](development-gap-audit-2026-08-11.md)。

目标是“导入信息不丢、能力边界可见、执行行为可测试”，而不是立即复制全部 Android 行为。

- **M7.0 元数据保真（本轮）**：映射并校验 bookSourceUrl、bookSourceGroup、bookSourceType、bookUrlPattern、exploreUrl、enabledExplore、customOrder、weight、bookSourceComment；文本书源以外类型明确拒绝。
- **M7.1 书源管理基础（M7.1a/b/c/d 已完成）**：SQLite 已保存分组、类型、权重、发现开关、自定义顺序和备注；列表已支持按分组/自定义顺序/权重排序、分组筛选、多选、批量启停/发现开关/删除、同组拖拽与上下移动、批量分组/备注/权重编辑、导入新增/更新/无变化差异提示。M7.1c 已加入更新已有/跳过已有/全部新建冲突策略、导入前快照、快照恢复和失败时的 replace-all 原子事务；M7.1d 的批量元数据修改先完成全量预校验，再以单个 SQLite 事务写入，避免半批次更新。M7.1d 的 CI、Windows Release 和 installer smoke 均已通过；当前统一验证基线为 CI [31574147034](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31574147034)、Windows Release [31574767135](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31574767135) 和 installer smoke [31575465554](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31575465554)；快照保留/清理策略和真实 Windows 手工体验仍待补齐。
- **M7.2 规则执行增强（核心、边界夹具、可取消链与首轮 UI 已完成）**：已接入 page/pageNum/pageIndex/page+1/page-1、keyword/key、bookUrl/bookId/chapterId 模板上下文；多页搜索最多 20 页并在空页/无新增结果时停止；调试面板展示脱敏变量快照。纯 next URL 策略闸门、累计配额、显式 opt-in 三页 HTTP 链和部分成功/环路/跨源/超时/超体积夹具已通过（run 30774860219，67 个 Rust 测试）；`fetch_source_chapter` 已接受可选 policy 并复用取消 token（run 30775360305）；设置页首轮追链开关、配额控件、同源限制、远端停止原因与缓存绕过已通过（run 31302166671），默认仍关闭；诊断导出已补充分页策略的安全配额、同源开关、停止原因和状态说明（run 31302984553），Windows Release 与 installer smoke 自动化验收已完成，下一步补充真实 Windows 环境手工验收。
- **M7.3 XPath 评估（静态识别、离线 PoC、授权夹具与耗时指标预览已完成）**：导入预览在规则上下文中识别 `//`、`xpath:`、`xpath=`、`@xpath` 等表达式，最多保留 8 条、每条 512 字节并展示原始值、不执行原因、静态解析状态、步数和估算工作量；离线 PoC 只解析受限路径/谓词并统计合成 HTML 节点，首轮夹具覆盖 6 条受限语法和 6 条拒绝语法，明确拒绝函数、联合、轴、父节点和超限输入，不翻译为 CSS、不访问网络。授权合成夹具、解析耗时分布和边界/回归夹具已完成（70 个 Rust 测试，run 31307700513）；继续保持不执行真实网络。
- **M7.4 JavaScript 评估闸门**：设计条件已记录在 [M7.4 JavaScript 评估安全闸门](m7-js-evaluation-gate.md)；许可证/供应链、独立隔离、资源配额、API 白名单、脱敏审计、逐源授权、回滚和 Windows 合成夹具全部通过前，不引入运行时，默认仍拒绝脚本。
- **M7.5 书源诊断**：单源重试、搜索/书籍/章节失败历史、本地保留与统计、跨请求 `operation_id`、本地失败报告、有限原因分类、旧库升级夹具、报告 schema_version 兼容约定、`source_metrics` 请求/缓存观测摘要和规则执行指标已完成；SQLite 0010 按书源/阶段统计网络请求，SQLite 0011/0012 按来源/阶段/字段规则键统计 attempts、success/no-match/failure/skipped，其中 skipped 不进入分母；书源页和报告展示明确比例、按规则分解与 `observed` 状态。规则指标边界详见 [规则执行指标边界](source-rule-metrics.md)；CI run [31322235077](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31322235077) 已通过前端检查、Rust fmt/check 和 76 个 Rust 测试。M7.5j 完成，下一步进入 M8 内容格式 v2。

### M8 — 阅读内容与本地格式 v2

详细执行清单见 [M8 内容格式 v2 计划](m8-content-format-plan.md)。M8.1 EPUB/TXT 导入边界与 M8.2 TXT 解析可配置化均已完成；M8.3 已完成安全链接索引、块锚点、跨章节片段跳转、有限 CSS 白名单和损坏 EPUB 恢复边界。M8.4 首个 TXT 行累积切片已完成，移除每章行字符串缓存并通过约 1 MiB/512 章夹具；随后完成单次字符扫描的 CRLF/CR 与全角空格归一化，并在无替换规则路径上改为逐行处理、有效 UTF-8 借用解码，CI run [31330389289](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31330389289) 通过，Rust 95 tests；新增 1/16/64 MiB 尺寸矩阵和每档 20 秒时间上限，run [31330961076](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31330961076) 通过；默认无替换规则路径已完成 UTF-16LE/BE 与 GB18030 的 64 KiB 有状态分块解码，run [31332336289](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31332336289) 通过；run [31334298865](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31334298865) 通过真实的 1/16/64 MiB 基线，Linux Rust 99 tests，artifact 9043889018，Ubuntu 记录 37/560/2241 ms 与 13,369,344/63,385,600/239,243,264 bytes 峰值 RSS；同一 run 的 Windows 测试编译与峰值 RSS 采样测试也通过，替换规则已采用 64 KiB 有界滚动缓冲并覆盖跨块匹配。M8.4 收口，M8.5 其他格式评估已完成图片单页、多图片安全预览和序列模型；M8.5.5d1 已完成内容摘要缓存键和本机位置恢复，M8.5.5e1-e4 已完成持久化 PNG 缩略图缓存、原子安装、容量/LRU 清理、取消/重试、当前页/相邻页恢复和崩溃临时文件清理，并继续保持 PDF 只读探测；M9.0/M9.1 已将图片序列接入 SQLite 书架并支持恢复，下一步进入 M9.2 文件变更检测与重新关联。

- EPUB：完善目录、章节标题、字体、图片、链接和基础 CSS 白名单，继续禁止脚本、事件属性、危险 URL 和无限资源。
- TXT：编码探测、章节规则配置、全角空格/缩进、替换词和大文件流式解析。
- M8.5.1 已完成只读格式探测：`BookFormatProbe`/`probe_book_format` 覆盖 TXT、EPUB、MOBI/AZW/AZW3、PDF 与常见图片签名；run [31334929174](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31334929174) 通过 100 个 Rust 测试和 Windows 测试编译/采样。M8.5.2 增加有界 PDF 版本、图片尺寸/MIME、MOBI 记录偏移/头长度元数据探测，run [31335820920](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31335820920) 通过 100 个 Rust 测试和 Windows 编译/采样；M8.5.3 将 `require_importable_format` 接入导入与预览，已知签名冲突明确拒绝，MOBI/AZW/AZW3 明确保持只读且不绕过 DRM，run [31336890258](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31336890258) 通过前端、Rust fmt/check、100 个 Rust 测试、性能证据和 Windows 编译/采样；MOBI/AZW 后续仍须审查许可证、解析器维护性、内存上限和 DRM 拒绝策略，并评估 PDF/图片独立阅读模型。
- M8.5.4a 已将格式探测接入本地文件导入前置流程，run [31337644268](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31337644268) 的前端、Rust checks 和 Windows sampler 全部通过；M8.5.4b 的 PDF/图片独立阅读模型契约已记录在 [m8-reader-model.md](m8-reader-model.md)。M8.5.5 依赖评估已记录在 [m8.5.5-dependency-evaluation.md](m8.5.5-dependency-evaluation.md)：图片采用受限 `image 0.25.9` 解码链，PDF 继续探测。
- M8.5.5a 已完成 PNG/JPEG/GIF/WebP 单页受限预览，run [31350842759](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31350842759) 的前端、Rust、Windows 编译/峰值 RSS 采样全部通过；M8.5.5b 已完成图片序列方向/双页/长图模型和 2,048 页、128,000,000 像素、512 MiB 解码字节配额，run [31351541210](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31351541210) 的前端、Rust 105 项测试、TXT 性能证据和 Windows 编译/峰值 RSS 采样全部通过；M8.5.5c 已完成多图片批量逐页解码、256 MiB 原始输入总量闸门、最多 24 个临时缩略图、方向/排版控制和只读预览，run [31352828101](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31352828101) 的前端、Rust、Windows 编译/峰值 RSS 采样全部通过。
- M8.5.5d1 已完成：前端为每个图片页生成内容摘要，组合为不含绝对路径的序列缓存键，并在本机恢复页码、缩放、方向和排版；当前只保存位置元数据，不写入书架或磁盘缩略图。run [31353873233](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31353873233) 的前端、Rust、Windows 编译/峰值 RSS 采样全部通过。
- M8.5.5e1-e4 已完成：Rust 在 Tauri 应用缓存目录下按内容摘要键写入 PNG 缩略图，单页最多 8 MiB、总缓存最多 512 MiB；临时文件 sync_all 后 rename 安装，损坏条目自动重建，超额按最早修改时间清理；缓存键保留文件选择顺序，缓存命令支持协作式取消和失败重试，读取命令一次最多恢复当前页及相邻页，并清理上一次进程遗留的临时文件；前端显示命中/写入/占用摘要。run [31359196211](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31359196211) 的前端、Rust fmt/check/tests、TXT 性能证据和 Windows 编译/峰值 RSS 采样全部通过。图片序列数据库化已转入 M9。
- PDF：单独评估 PDF 渲染/搜索/目录模型；不把 PDF 当作普通文本章节。
- 漫画/图片：M8.5.5d1 已完成内容摘要键和本机位置恢复，M8.5.5e1-e4 已完成持久化缩略图文件缓存、原子安装、容量/LRU 清理、取消/重试、页序一致性和当前页/相邻页恢复；M9.0/M9.1 已完成 SQLite 书架记录、重启恢复和阅读进度回写；M9.2 继续补齐文件变更检测与重新关联，不混入文本阅读器。
- 所有格式都需旧数据库迁移、损坏文件错误恢复和 64 MB 默认导入上限测试。
### M9 — 书架、元数据与阅读历史

详细执行清单见 [M9 图片序列数据库化与书架恢复计划](m9-image-sequence-plan.md)。M9.0 SQLite 模型、迁移、路径安全约束和外键恢复已完成；M9.1 已完成图片序列保存到书架、重启恢复、缩略图恢复和页码/缩放/方向/排版进度回写，CI run [31363237878](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31363237878) 通过；M9.2.1/9.2.2 已完成安全路径解析、快速文件指纹、状态判定、数据库回写和书架恢复时的状态提示，CI run [31368081755](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31368081755) 通过；M9.2.3 已加入 Tauri 原生目录选择、受限目录扫描、候选匹配、差异预览和事务化重新关联，CI run [31370651374](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31370651374) 通过；按需 SHA-256 复核与 stale 恢复已完成，CI run [31375034783](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31375034783) 通过；M9.2.4 首轮书架/阅读器状态反馈已完成，CI run [31377102734](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31377102734) 通过；M9.2.5 已加入 15 秒目录扫描超时、区分式错误提示和协作式取消任务令牌，CI run [31380314725](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31380314725) 通过。最新 Windows release run [31410657419](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31410657419) 已在 `main` 生成安装版、便携版和 `release-sha256.txt`，artifact [9071723663](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31410657419#artifacts) 摘要为 `sha256:0055d98696597f5a2c1f0dd17f33a08281bb9c42316b7e9a6361d26df9ad4ddf`；installer smoke run [31411486079](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31411486079) 已通过产物/校验和、便携版启动、NSIS/MSI 安装卸载和数据保留自动化检查。真实 Windows 手工验收仍待在目标环境完成。

- M9.2.3/M9.2.4/M9.2.5（目录恢复、状态反馈、超时边界与取消反馈已完成首切片）：原生目录选择、相对路径/文件名+大小候选、missing/stale/needs_relink 差异预览、事务化重新关联，仅针对 stale 页的有界 SHA-256 复核、书架/阅读器缺页提示，以及 15 秒扫描超时和可取消任务令牌；摘要一致可恢复为 ready，无摘要旧页继续保持 stale。真实 Windows 手工验收仍待完成，不能由 CI 代替。
- M9.3 查询/单本编辑、批量元数据与重复候选预览首切片均已完成：分组、标签、筛选、排序、单本编辑、多选、批量分组/标签更新和只读重复候选检查，main CI run [31385269180](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31385269180)、[31388184266](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31388184266)、[31390991820](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31390991820) 通过；Windows Release/installer smoke run [31388689695](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31388689695)、[31389443472](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31389443472)、[31391524135](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31391524135)、[31392423285](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31392423285) 通过。M9.3.1-a 已完成首个纯只读代码切片：`src-tauri/src/cover.rs` 提供封面来源规范化和版本化缓存键，提交 [91bc285d654568b942fbcc59e779c6efb43ea79d](https://github.com/TttXxx36/Open-reader-desktop/commit/91bc285d654568b942fbcc59e779c6efb43ea79d) 对应 CI run [31394712988](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31394712988) 已通过；9.3.1-b 已完成 `cover_cache` 文件层、`0015_book_covers.sql`、封面状态查询和书架提示；9.3.1-c 已完成 `preview_book_merge` 只读合并预览（2—8 本、显式 canonical、输入指纹、章节/元数据/封面候选与图片序列阻断），提交 [a91d1af7ffb1c5c8ccc91187615550a65cca2647](https://github.com/TttXxx36/Open-reader-desktop/commit/a91d1af7ffb1c5c8ccc91187615550a65cca2647) 对应 CI run [31402819722](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31402819722) 已通过；9.3.1-d1 已完成只读预览二次校验、过期/指纹变化拒绝和前端重新验证入口，最新 d1 提交 [f18f9ccf539a1059544df9657308127a983689ad](https://github.com/TttXxx36/Open-reader-desktop/commit/f18f9ccf539a1059544df9657308127a983689ad) 对应 CI run [31409256256](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31409256256) 已通过前端、Rust fmt/check/tests/TXT 性能证据和 Windows 编译/峰值 RSS 采样。 9.3.1-d1 已完成只读预览二次校验和前端重新验证入口，最新 d1 提交 [f18f9ccf539a1059544df9657308127a983689ad](https://github.com/TttXxx36/Open-reader-desktop/commit/f18f9ccf539a1059544df9657308127a983689ad) 对应 CI run [31409256256](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31409256256) 已通过前端、Rust fmt/check/tests/TXT 性能证据和 Windows 编译/峰值 RSS 采样。 下一步进入 9.3.1-d2 迁移前评审，先冻结 `0016` 字段和撤销冲突策略，确认前不执行不可逆删除。 Windows 发布闸门已由 run [31410657419](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31410657419) 与 installer smoke run [31411486079](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31411486079) 通过；d2 仍保持迁移前评审边界，不执行物理删除或静默覆盖。
- 最新 `main` 的 Windows Release run [31410657419](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31410657419)（提交 `5b71fc9dfe64181a9091dd066dca273cac754819`）已成功生成安装版、便携版和 SHA-256 清单；artifact [9071723663](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31410657419#artifacts)，摘要为 `sha256:0055d98696597f5a2c1f0dd17f33a08281bb9c42316b7e9a6361d26df9ad4ddf`。随后 installer smoke run [31411486079](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31411486079) 已通过产物/校验和、便携版启动、NSIS/MSI 安装卸载和数据保留自动化检查。
- 阅读历史时间线、章节统计、书签、笔记、划线和“已读/未读”状态。
- 书籍元数据编辑、封面来源策略、最近阅读和继续阅读统一为可查询数据模型。
- 设计搜索索引与迁移策略，避免把大型正文直接存入前端状态。

## 2026-08-12 维护复核

- 书源配置导入本地 JSON/在线 URL 统一使用 16 MiB bundle 上限，在线拉取超时 30 秒；普通书源请求仍维持 15 秒、2 MiB、最多 5 次重定向边界。
- M7/M8/M9 文档中的“已实现”只表示代码与远程自动化通过；安装升级、WebView2、键盘/字体/窄窗口、书源导入和迁移仍需目标 Windows 记录。
- 维护决策与 #1–#5 issue 状态见 [2026-08-12 维护审计](maintenance-audit-2026-08-12.md)。

### M10 — 备份、恢复与同步

- 版本化 JSON/ZIP 备份：书架、进度、设置、书源、书签和替换规则可选择导出。
- 本地备份轮换、校验和、损坏恢复与导入预览。
- WebDAV 先做显式手动上传/下载，再做冲突检测；凭据存储使用 Windows 安全存储，绝不写入日志。
- 同步属于可选能力，默认不上传书籍正文和未授权内容。

### M11 — 发现页、RSS 与 OPDS

- RSS/Atom 订阅模型、刷新频率、缓存和去重。
- OPDS 目录只读接入与授权提示。
- 发现页与书源搜索共用结果卡片，但隔离网络权限、刷新任务和错误提示。
- 不提供未经授权的聚合内容列表；示例仅使用公开测试源。

### M12 — TTS、漫画与音频能力评估

- TTS：系统语音接口、队列、暂停/续播、段落定位和隐私提示。
- 漫画：图片缓存、预加载、双页/长图和阅读方向。
- 音频书源：与 bookSourceType=1 对齐前先做播放器、缓存、许可和 UI 评估；在此之前保持明确拒绝。
- 只有形成独立数据模型和性能基线后，才进入正式里程碑。

## 横向质量线

- **界面**：MD3 的层次、色彩和动态反馈可借鉴，但采用 Windows 信息架构；补齐响应式、键盘、缩放、中文字体和高对比度验收。
- **性能**：大文件流式/分块解析、搜索并发上限、缓存命中率、首屏和章节切换耗时必须有可重复基线。
- **安全**：URL scheme、重定向、响应体、缓存、HTML/图片、脚本、请求头、凭据和文件路径都要有拒绝用例。
- **可观测性**：错误消息可操作、日志脱敏、诊断导出可选，默认不收集遥测。
- **CI/发布**：前端 typecheck/build、UI 契约、Rust fmt/check/test、Windows 安装器 smoke、便携包和 SHA-256 都进入 GitHub Actions；本地不构建、不安装。

## 当前执行顺序

1. **P0 目标 Windows 人工验收**：按模板记录安装器覆盖升级、WebView2 缺失、离线/网络错误、中文字体、窄窗口、键盘 Tab、高对比度，以及本地 JSON/在线 URL 书源导入、16 MiB 边界、冲突策略和快照恢复。
2. **M9.3.1-d2 迁移前收口**：在 P0 记录完成后冻结 0016 schema、预览二次校验、纯文本/无冲突范围、单事务回滚和旧库夹具；未经确认不执行迁移、物理删除或静默覆盖。
3. **M7.1/M8 横向收尾**：补快照保留/清理、缺失资源提示、窄窗口/键盘可用性和兼容矩阵回归；继续明确拒绝 XPath、JavaScript、Cookie、Authorization 和音频执行。
4. **M10 备份/恢复**：在 d2 可回滚边界稳定后，再实现版本化备份、导入预览、校验和、损坏恢复和冲突策略；WebDAV/RSS/OPDS/TTS 继续排在后续阶段。

### 已完成历史（保留证据）

1. M7.0 元数据保真（已完成，CI 通过）。
2. M7.1a 书源管理基础切片（已完成，CI run 30755061447 通过）。
3. M7.1b 批量操作、同分组上下移动、导入 diff（已完成首个切片，CI run 30755644977 通过）。
4. M7.1c 冲突策略、导入快照、恢复和失败原子性（已完成，CI run 30757528534 通过）。
5. M7.2 分页/变量/调试快照首个切片、分页诊断、安全回退链子集、缓存命中提示、有限 JSONPath、每阶段最多 8 项候选的 URL 回退链、脱敏诊断快照导出、书籍/章节统一请求步骤、URL 阶段超时预算、pipeline 总耗时预算、相对可排序时间线、端到端/远端/多源取消、多源失败聚合与受限 next URL 中间模型（已完成，CI run 30760156272、30760475594、30761456593、30761886043、30763032154、30764314398、30764884266、30765352024、30765823280、30766330200、30766847254、30767351427、30767940334、30768670672 通过）；纯策略闸门及 stop-reason 矩阵已完成（CI run 30771892118、30772265421），累计多页夹具证明预算不按页重置、深度优先级稳定（CI run 30772819136），显式 opt-in 三页请求夹具也已通过（CI run 30773702655，62 个 Rust 测试），并完成部分成功、环路、跨源、超时和超体积边界夹具（CI run 30774860219，67 个 Rust 测试）；默认路径保持单页；可选 policy 已接入 Tauri 章节命令并复用取消 token（CI run 30775360305），设置页与远端阅读状态首轮 UI 已通过（CI run 31302166671），诊断导出已补充分页策略的安全配额、同源开关、停止原因和状态说明（run 31302984553）。
6. M7.3 XPath 静态识别、离线解析 PoC、首轮语法夹具与指标预览（已完成，70 个 Rust 测试，CI run 31307700513 通过）；继续禁止真实网络执行。
7. M6.5 Windows Release 与 installer smoke 自动化验收已完成（run 31308312340、31308654635）；待真实 Windows 环境补验升级/WebView2/离线错误/回滚及 UI 可用性。
8. M7.5 单源重试、搜索/书籍/章节失败历史、本地保留与统计、跨请求关联 ID、本地失败报告、有限原因分类、旧库升级、报告 schema_version 兼容、source_metrics 请求/缓存统计、规则执行首个实现和字段级/skipped 收尾均已完成（SQLite 0010/0011/0012，CI run [31322235077](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31322235077)，76 个 Rust 测试）；转入 M8 内容格式 v2，授权合成边界夹具继续作为横向质量线。
9. M8.5.5a/b/c/d1/e1-e4 已完成（CI run 31359196211 通过）；M9.0/M9.1 图片序列 SQLite 模型、书架保存、重启恢复和进度回写已完成（CI run [31363237878](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31363237878) 通过）；M9.2.1/9.2.2 文件状态检测第一切片已完成（CI run [31368081755](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31368081755)），M9.2.3 目录选择、差异预览和按需 SHA-256 stale 恢复已完成（CI run [31375034783](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31375034783)），M9.2.4 书架/阅读器状态反馈已完成（CI run [31377102734](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31377102734)），M9.2.5 15 秒目录扫描超时与协作式取消首切片已完成（CI run [31380314725](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31380314725)）；M9.3 查询/单本编辑、批量元数据与重复候选预览首切片已完成（CI run [31385269180](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31385269180)、[31388184266](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31388184266)、[31390991820](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31390991820)），最新 Windows Release/installer smoke 已通过（run [31391524135](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31391524135)、[31392423285](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31392423285)）；真实 Windows 手工验收仍待完成；最新 `main` 的 Windows Release run [31404728684](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31404728684)（提交 `bfbeb409e76b1332e933ef5c525744909126bf7d`）已成功生成安装版、便携版和 SHA-256 清单；artifact [9069420519](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31404728684#artifacts)，摘要为 `sha256:1a851633a49bb83005d6b1ac0a8e8b93bcc743634fa1548481ccebfed633e1a6`。随后 installer smoke run [31405543069](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31405543069) 已通过产物/校验和、便携版启动、NSIS/MSI 安装卸载和数据保留自动化检查。 下一步先补充封面缓存策略和可回滚的重复书合并设计，再进入 M10、M11、M12，并在每阶段完成一次路线图复盘。

## 里程碑完成定义

代码提交、远程 CI 和自动化测试全部通过只是必要条件；涉及 Windows 安装、升级、卸载、WebView2、字体和焦点的项目，还必须附 GitHub Actions 产物和人工验收记录。任何未实现的 Legado 能力都必须在兼容性矩阵中显式标注，不以“导入成功”代替“执行成功”。
