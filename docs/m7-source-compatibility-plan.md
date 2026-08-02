# M7 书源兼容性 v2 实施计划

本计划承接 M6.1 的安全导入层，目标是让 Legado 常用书源“导入信息不丢、能力边界可见、执行行为可测试”。本计划不承诺一次性复制 Android 端全部规则和脚本能力。

## 设计决策（Grill 结论）

1. M6.5 Windows 发布闸门不阻塞 M7 代码开发，但在安装/升级/卸载/WebView2 回归完成前不宣称正式发布完成。
2. 兼容性采用三档：可执行、可导入但不执行、明确拒绝；不允许静默丢弃 XPath、JavaScript、Cookie 或认证信息。
3. 先补元数据和内部模型，再做规则执行；脚本运行时必须经过许可证、沙箱、配额和权限审查。
4. 借鉴 Android 项目的数据流、测试思路和交互，不直接复制受许可证约束的代码；本项目许可证仍需单独确定。
5. UI 借鉴 MD3 的层次、色彩和动态反馈，但采用 Windows 的信息架构、键盘和窗口行为。

## M7.0 元数据保真（已完成）

### 代码范围

- BookSource 增加来源 URL、分组、类型、书籍 URL 模式、发现 URL、发现开关、自定义顺序、权重和备注字段。
- 外部 JSON 支持 Legado 常见别名：bookSourceUrl、bookSourceGroup、bookSourceType、bookUrlPattern、exploreUrl、enabledExplore、customOrder、weight、bookSourceComment。
- bookSourceUrl 可作为导入条目的稳定 ID；仍优先使用显式 id/sourceId。
- 只支持文本书源（bookSourceType=0）；音频类型明确报错，不静默当成文本。
- 来源 URL、发现 URL、分组、备注、模式和排序数值均有长度/范围校验；来源主机进入安全审计。

### 验收证据

- Rust 元数据解析、别名映射、音频类型拒绝和来源 URL ID 回归测试。
- GitHub Actions CI：Frontend checks、UI contract、Rust fmt、Cargo check、Rust tests 均通过；本轮 Rust tests 为 38 passed（新增 M7.1 元数据重启持久化测试）。
- 本地不构建、不安装；真实网络只使用授权或合成夹具。

## M7.1 书源管理基础（已完成 M7.1a + M7.1b + M7.1c）

本轮已完成 SQLite 元数据迁移、旧配置回填、按分组/自定义顺序/权重排序，以及书源列表的分组筛选、发现开关和元数据编辑。M7.1b 补齐多选、批量启停/发现开关/删除、同分组上移下移，以及导入预览的新增/更新/无变化和变更字段提示；M7.1c 又补齐批量分组移动、导入冲突策略、导入前快照、快照恢复和失败时的原子替换。远程 CI 已验证前端构建、UI 契约、Rust 检查和 39 个测试；跨分组拖拽、快照保留策略和 Windows 手工体验仍属于后续收尾。

### 数据库

新增迁移，给 book_sources 增加：

- source_url：稳定来源标识/主页 URL；
- group_name：用户分组；
- source_type：当前固定文本类型 0；
- weight：搜索排序权重；
- enabled_explore：是否进入发现；
- custom_order：用户自定义顺序；
- comment：来源备注；
- book_url_pattern、explore_url：后续发现和详情链路使用。

迁移必须保留旧配置 JSON，旧数据库默认值必须与当前行为一致；升级失败必须可诊断，不得删除原有书源。

### Rust 命令

已完成：

- list_sources：返回元数据并按“分组 → customOrder → weight → name”排序。
- save_source：从校验后的 BookSource 一次性写入 JSON 与元数据，避免两套状态漂移。
- set_source_enabled、set_source_explore_enabled：分别控制搜索和发现开关。
- update_source_metadata：以一次事务式更新同步 JSON 别名和 SQLite 元数据。
- export_sources：导出配置时携带元数据字段。
- set_sources_group：批量移动书源分组，并同步 JSON 与 SQLite 元数据。
- list_source_snapshots、restore_source_snapshot：列出最近快照并以原子事务恢复完整书源集合。

已完成本轮：

- set_sources_enabled、set_sources_explore_enabled、delete_sources：批量启停、发现开关和删除。
- reorder_sources：同分组上移/下移并回写 custom_order。
- 导入预览：显示新增/更新/无变化和变更字段；仍由用户确认后才写入。
- 导入冲突策略：更新已有、跳过已有、全部新建；写入前自动保存当前配置快照。
- 快照恢复：恢复前校验完整 bundle，使用 replace-all 事务，失败时保留原数据和快照。

后续收尾：

- 跨分组拖拽排序、批量备注/权重编辑和快照清理/保留策略。
- 真实 Windows 窄窗口下的批量操作、冲突提示和恢复流程手工验收。

### Vue 界面

- 书源列表增加分组、类型、权重、发现开关和备注摘要。
- 增加按分组筛选、启用/停用、拖拽或上下移动排序、批量操作。
- 编辑器显示“可执行/可导入但不执行/拒绝”能力徽标；保存前显示元数据与规则 diff。
- 不把认证头、Cookie 或脚本原文展示到诊断日志中。

### 验收

已完成本轮：

- SQLite 0006/0007 迁移、默认值和旧 JSON 元数据回填。
- 列表排序、分组筛选、搜索/发现开关、权重/顺序/备注编辑和批量分组移动。
- 导入冲突策略、导入前快照、快照列表/恢复和 replace-all 原子事务。
- 保存后重启仍保持元数据；前端 typecheck/build/UI contract、Rust fmt/check/test 全部通过。

远程证据：[GitHub Actions run 30755061447](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/30755061447)（M7.1a）；[GitHub Actions run 30755644977](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/30755644977)（M7.1b 首个切片）；[GitHub Actions run 30757528534](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/30757528534)（M7.1c，前端与 Rust 39 tests）。

后续验收：

- 空库/旧 M5 数据库升级失败回滚、重复来源 URL 冲突和非法枚举专项夹具。
- 跨分组拖拽、批量元数据编辑、快照清理策略和 Windows 手工恢复流程。

## M7.2 规则执行增强（分页诊断、安全回退链、缓存诊断与有限 JSONPath 已完成）

分页/模板变量首个切片已接入真实 pipeline：支持 page、pageNum、pageIndex、page+1、page-1、keyword/key、bookUrl/bookId、chapterId；多页搜索有 20 页硬上限，并在空页或无新增结果时停止。随后补齐了统一的每源诊断模型（扫描页数、解析数量、终止原因、请求失败）和前端展示；调试步骤携带变量名快照（关键词、URL 和 ID 脱敏）与 cache_hit 字段。

当前已完成安全回退链子集：导入器保留 `||` 候选，CSS 链和 JSONPath 链分别最多 8 项，按顺序尝试直到得到结果；混合 CSS/JSONPath、XPath、JavaScript 和不受支持的表达式会明确拒绝，不静默丢弃。字符串规则与对象规则的兼容形态也保持不变。

请求 URL 也支持每个阶段最多 8 个 `||` 候选；失败候选会进入诊断步骤，首个成功 URL 作为后续相对章节链接的基准。URL 链仅做模板渲染和请求重试，不执行脚本。每个阶段共享受限时间预算（最多 60 秒，按客户端请求超时和候选数量计算）；预算耗尽会取消当前请求、记录超时诊断并停止继续尝试。端到端书源调试、远端书籍/章节刷新和多源搜索均支持显式用户取消，整个搜索→详情→目录→正文 pipeline 另有总预算（最多 120 秒）。正文规则已支持受限 `nextUrl/nextPage` 中间结果：只解析一个绝对 HTTP(S) 链接并提示用户，不自动追链；自动追链、深度/响应配额仍待评估。

书籍/章节缓存响应现在返回 cache_hit；过期刷新失败时保留 stale 与错误原因，阅读器明确提示缓存来源并可手动刷新。基础缓存可见化与脱敏诊断快照导出已完成；统一的书籍/章节请求步骤和缓存回退事件已纳入快照，并带有稳定的相对顺序与单调 start_ms，可排序时间线已完成；跨请求失败聚合和更细的重试元数据仍属于后续切片。

有限 JSONPath 已扩展为安全子集：支持对象字段、数组下标、数组/对象通配、连续括号字段别名（`['title']`/`["title"]`）和单字段等值过滤（`[?(@.kind == 'novel')]`）。路径最多 512 字节，字段最多 128 字节，过滤值最多 256 字节，单段最多产生 256 个节点；不等式、逻辑组合、函数、脚本和转义表达式继续拒绝。

书源调试页提供脱敏诊断快照导出：只写入阶段名、脱敏 URL、状态、耗时、响应大小、失败原因、变量名和 cache_hit 汇总，不写入正文、关键词、书籍/章节 ID、Cookie 或请求头；快照最多 256 个步骤且文件不超过 256 KB。远端章节响应也保留请求步骤，导出时按 pipeline、书籍、章节顺序合并，并追加缓存命中或 stale 回退事件；每个步骤带有 order 和相对单调 start_ms，便于离线排序且不暴露系统绝对时间。多源搜索的失败消息、扫描页数、解析数量和停止原因也会按上限合并，形成跨请求失败摘要。

远程证据：[GitHub Actions run 30760475594](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/30760475594)（分页诊断，40 tests）；[GitHub Actions run 30761456593](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/30761456593)（安全回退链，42 tests）；[GitHub Actions run 30761886043](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/30761886043)（缓存命中诊断，42 tests）；[GitHub Actions run 30763032154](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/30763032154)（有限 JSONPath，44 tests）；[GitHub Actions run 30764314398](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/30764314398)（URL 回退链，47 tests）；[GitHub Actions run 30764884266](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/30764884266)（脱敏诊断快照导出，47 tests）；[GitHub Actions run 30765352024](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/30765352024)（书籍/章节统一请求时间线，47 tests）；[GitHub Actions run 30765823280](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/30765823280)（URL 阶段超时预算，48 tests）；[GitHub Actions run 30766330200](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/30766330200)（相对可排序时间线与调试取消，48 tests）；[GitHub Actions run 30766847254](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/30766847254)（pipeline 总耗时预算，49 tests）；[GitHub Actions run 30767351427](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/30767351427)（多源失败聚合，49 tests）；[GitHub Actions run 30767940334](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/30767940334)（远端/多源取消，49 tests）；[GitHub Actions run 30768670672](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/30768670672)（受限 next URL 中间模型，50 tests）。

后续切片：

1. 为已合并的书籍/章节诊断事件补充跨请求失败聚合、重试原因分类和导出版本兼容。
2. 保持复杂 JSONPath（函数、脚本、逻辑组合）拒绝，并根据授权夹具补充有限数字/布尔值过滤。
3. 自动追踪受限 next URL 仍未开启；纯策略闸门已固定候选校验、同源、深度/页面/响应体/总耗时、环路和稳定 stop reason，并用 60 个 Rust 测试覆盖停止原因和无限配额裁剪（run 30772265421）。下一步按 [M7.2 next URL 自动追链策略](m7-next-url-policy.md) 建立授权多页夹具，再决定是否接入请求链。
4. 为书籍/章节和多源请求在 UI 中展示候选命中说明，并将取消状态纳入失败聚合。

## M7.3 XPath 评估闸门（静态识别切片已完成）

本轮先落地导入侧的静态只读闸门，而不是引入 XPath 执行器：

- 导入预览只在规则上下文中识别 `//`、`xpath:`、`xpath=` 和 `@xpath` 等表达式，最多保留 8 条发现、每条最多 512 字节，并展示原始表达式、路径上下文和“仅用于只读兼容性评估，当前不执行”的原因。
- 预览阶段复用真正导入使用的 `validate_source_json`，避免出现“预览可导入、实际导入才失败”的不一致；XPath 条目当前仍标记为不可执行并跳过，不会降级成 CSS。
- 远程证据：[GitHub Actions run 30769462298](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/30769462298)（M7.3a 静态识别与预览契约，51 个 Rust 测试、前端检查通过）。

M7.3b 已加入离线只读解析 PoC（xpath_poc.rs 与 analyze_xpath_offline 命令）：

- 仅接受以 \`/\` 或 \`//\` 开头的路径，支持元素/通配符、子级/后代分隔、单个属性等值谓词、位置谓词和末端属性读取。
- 明确拒绝函数、联合、轴、父节点、复杂逻辑与超出 1,024 字节表达式、16 步、256 字节谓词、64 KiB 合成 HTML、4,096 节点或 65,536 工作量上限的输入。
- 解析器只统计 AST 步数、谓词、节点数、估算工作量和耗时；不把 XPath 翻译为 CSS、不发起网络请求、不接入真实书源执行链。
- 远程证据：[GitHub Actions run 30770030409](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/30770030409)（M7.3b PoC，55 个 Rust 测试、前端检查通过）。

M7.3c 已补充合成 HTML 夹具矩阵：覆盖 6 条受限语法（元素、绝对/相对路径、属性谓词、位置谓词、通配符、末端属性）和 6 条明确拒绝语法（函数、复杂属性谓词、联合、轴、父节点），并断言密集节点输入的估算工作量仍受上限约束。
- 远程证据：[GitHub Actions run 30770432561](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/30770432561)（M7.3c，56 个 Rust 测试、前端检查通过）。

M7.3d 已把离线指标接入导入预览：每条发现同时展示受限语法是否可静态解析、步数和估算工作量；即使静态解析成功，仍明确保持“不执行”状态。
- 远程证据：[GitHub Actions run 30770921591](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/30770921591)（M7.3d，56 个 Rust 测试、前端检查通过）。

下一步是继续扩充授权合成夹具并记录解析耗时分布；任何失败都不得静默改写为 CSS，只有固定节点数、表达式长度、执行时间和网络权限后，才决定是否进入可执行档。

## M7.4 JavaScript 评估闸门

必须先完成：许可证确认、独立沙箱、超时/堆限制、API 白名单、禁止文件/进程/系统命令、日志脱敏、用户逐项授权和回滚开关。未完成前继续明确拒绝 @js、webJs 等脚本。

## M7.5 诊断与维护

- 单书源重试、失败历史、请求/解析/缓存时间线。
- 导入 diff、配置版本迁移、冲突提示和可回滚快照。
- 统计启用源、失败率、缓存命中和规则兼容性；默认不上传遥测。

## GitHub 执行方式

每个子阶段拆为 Issue → 小 PR → CI → 兼容性矩阵更新 → 路线图记录。前端 typecheck/build/UI contract、Rust fmt/check/test 必须通过；涉及安装器的任务额外等待 M6.5 Windows Actions 验收。本地不执行构建或安装。
