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
- GitHub Actions CI：Frontend checks、UI contract、Rust fmt、Cargo check、Rust tests 均通过；本轮 Rust tests 为 37 passed。
- 本地不构建、不安装；真实网络只使用授权或合成夹具。

## M7.1 书源管理基础（下一步）

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

- list_sources：返回元数据并按“分组 → customOrder → weight → name”排序。
- save_source：从校验后的 BookSource 一次性写入 JSON 与元数据，避免两套状态漂移。
- set_source_enabled、set_source_explore_enabled：分别控制搜索和发现开关。
- reorder_sources、set_source_group：支持单项和批量操作。
- export_sources：导出版本号、元数据和配置；导入预览显示字段差异。

### Vue 界面

- 书源列表增加分组、类型、权重、发现开关和备注摘要。
- 增加按分组筛选、启用/停用、拖拽或上下移动排序、批量操作。
- 编辑器显示“可执行/可导入但不执行/拒绝”能力徽标；保存前显示元数据与规则 diff。
- 不把认证头、Cookie 或脚本原文展示到诊断日志中。

### 验收

- 迁移测试：空库、旧 M5 数据库、重复来源 URL、非法枚举和回滚失败。
- UI 契约：列表排序、分组筛选、启用/发现开关、键盘 Tab 和窄窗口。
- Rust 测试：保存后重启仍保持元数据，导出再导入不丢字段。

## M7.2 规则执行增强

1. 定义分页/变量中间模型，覆盖 page、page+1、page-1、关键词、书籍 ID 和章节 ID。
2. 支持安全模板变量、链式规则和有限 JSONPath 表达式；每项能力都要有大小、递归深度和超时配额。
3. 增加规则调试快照：请求 URL（脱敏）、阶段、解析数量、耗时、缓存命中和失败原因。
4. 为分页循环设置最大页数和终止条件，禁止无限请求。

## M7.3 XPath 评估闸门

- 先做离线只读解析 PoC，只对合成 HTML 运行。
- 统计常见 XPath 语法覆盖率、解析耗时和资源占用。
- 只有明确限制节点数、表达式长度、执行时间和网络权限后，才决定是否进入可执行档。
- 不能把 XPath 失败静默改写为 CSS；导入预览必须保留原规则和“不执行”原因。

## M7.4 JavaScript 评估闸门

必须先完成：许可证确认、独立沙箱、超时/堆限制、API 白名单、禁止文件/进程/系统命令、日志脱敏、用户逐项授权和回滚开关。未完成前继续明确拒绝 @js、webJs 等脚本。

## M7.5 诊断与维护

- 单书源重试、失败历史、请求/解析/缓存时间线。
- 导入 diff、配置版本迁移、冲突提示和可回滚快照。
- 统计启用源、失败率、缓存命中和规则兼容性；默认不上传遥测。

## GitHub 执行方式

每个子阶段拆为 Issue → 小 PR → CI → 兼容性矩阵更新 → 路线图记录。前端 typecheck/build/UI contract、Rust fmt/check/test 必须通过；涉及安装器的任务额外等待 M6.5 Windows Actions 验收。本地不执行构建或安装。
