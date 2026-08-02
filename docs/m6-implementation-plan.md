# M6 Windows 体验与阅读质量计划

本计划记录 2026-08-02 安装包回归后确认的缺陷、设计拷问结论、实施顺序和验收标准。M6 以“先恢复可用性，再扩展兼容性，最后完善内容排版”为原则。

## 一、缺陷证据

| 编号 | 现象 | 当前代码证据 | 优先级 |
| --- | --- | --- | --- |
| M6-UI-001 | 设置按钮点击无反应 | \`src/App.vue\` 的设置导航仍带有 \`disabled\` | P0 |
| M6-UI-002 | 书源导入按钮点击无反应 | 文件选择框位于书架条件分支，书源页面中 ref 为空 | P0 |
| M6-SOURCE-001 | 外部书源 JSON 无法直接导入 | 后端只接受 \`{version:1,sources:[...]}\` 项目导出包 | P0 |
| M6-READER-001 | EPUB 样式、标题、图片和段落语义丢失 | EPUB 导入使用 \`strip_html\`，章节只保存纯文本 | P1 |
| M6-READER-002 | 阅读设置不足 | 当前只有字号、行距和三种主题 | P1 |
| M6-BRAND-001 | 界面图标与安装包图标不一致 | 侧栏使用文字 \`O\`，未使用书籍图标资产 | P1 |
| M6-BRAND-002 | 中文字体回退不稳定 | 全局字体栈以 Inter 为首，未定义中文优先栈 | P1 |

## 二、设计拷问结论

### 1. 是否先重做首页再修功能？

不先做视觉重构。死按钮和无法导入会直接阻断用户流程，必须先修复；首页重构放在 M6.4，并复用已经稳定的设置、书源和书架状态。

### 2. 是否一次性兼容全部 Legado 书源？

不承诺。先支持项目协议、单个兼容对象、Legado 3.0 的安全子集；对 XPath、JavaScript、登录态、Cookie 和特殊脚本明确报告“不支持”，不静默导入。

### 3. 是否立刻把 EPUB 原始 HTML 用 v-html 渲染？

不允许。外部 EPUB 和书源正文都属于不可信输入，必须先转为白名单内容块或经过严格清洗的 HTML，禁止脚本、事件属性和危险 URL。

### 4. 阅读设置是否需要立即写入 SQLite？

第一阶段使用版本化 localStorage 保存全局设置，避免为了 UI 设置引入不必要的数据库迁移；按书籍覆盖、同步和恢复策略在阅读数据模型稳定后再加入。

### 5. 是否现在拆分整个 App.vue？

M6.0 先做小步修复并建立回归测试；随后把 2000 多行的单文件拆成 views/components/composables，避免继续产生条件渲染和 ref 生命周期错误。

## 三、实施里程碑

### M6.0 可用性修复（当前）

- [x] 设置导航可用并显示基础设置页
- [x] 书源文件选择框始终挂载，导入按钮可触发选择
- [x] 保持项目导出包兼容
- [x] 支持单个当前协议 JSON 和安全的 Legado 字段映射
- [x] 导入成功显示数量；不兼容规则拒绝并显示原因（逐条跳过将在 M6.1）
- [x] 侧栏使用真实书籍 SVG 图标
- [x] 使用中文优先字体栈
- [x] 阅读页增加字体、版心、段间距和首行缩进设置
- [x] 增加前端契约检查和 Rust 导入适配器测试

### M6.1 书源导入兼容层

- [x] 支持项目导出包、单对象和数组三种外部 JSON 形态，并兼容 UTF-8 BOM 及 `sources`、`bookSources`、`items`、`data` 包装
- [x] 映射 `bookSourceName`、`ruleSearch`、`ruleBookInfo`、`ruleToc`、`ruleContent`，并兼容 `bookName`、`bookAuthor`、`coverUrl` 等常见别名
- [x] 对不支持的 XPath/JavaScript/认证头在预览中逐项标明并跳过；严格保存命令仍会拒绝不兼容项
- [x] 支持从 HTTP(S) URL 获取 JSON，并复用本地文件的同一解析、校验和持久化流程
- [x] 对 URL 长度（2 KB）和响应体大小（2 MB）设置上限，不携带 Cookie 或认证头
- [x] 导入前显示预览，自动跳过不兼容条目；仅保存通过校验的条目
- [x] 使用合成夹具覆盖原生包、数组、包装对象、字符串包装和常见 Legado 别名；授权来源夹具仍需后续补充
- [x] 支持安全 JSONPath/API 规则：搜索、书籍信息、目录和正文均可按 `$.items[*]`、数组下标和 `json:` 前缀提取

### M6.2 阅读内容模型

- [x] TXT 保留段落、缩进和章节标题
- [x] EPUB 使用 `blocks-v1` 保留标题、段落、引用和基础强调
- [x] EPUB 过滤 script/style/noscript，并将不可信图片转为占位文本
- [x] 仅嵌入受限的本地栅格图片（单张 2 MB、总计 8 MB），外链和 SVG 保持占位
- [x] 新增 `content_format` 字段与 `blocks-v1` JSON 内容块
- [x] 旧数据库章节默认 `text`，前端保留纯文本读取回退

### M6.3 阅读设置

- [x] 字体、字号、行距、字间距
- [x] 版心宽度、左右边距、段间距、首行缩进
- [x] 文本对齐和滚动/分页模式
- [x] 夜间、纸张、暖色和自定义颜色主题
- [x] 设置重置、版本迁移和重启持久化

### M6.4 首页与组件化

- [x] 最近阅读、继续阅读、书架统计和空状态引导
- [x] 将书架概览、书源、本地阅读器、远端阅读器和阅读设置抽成独立 Vue 组件；Tauri 状态与命令仍由 App 编排
- [x] 统一图标资源、颜色 token、中文字体栈和键盘焦点样式
- [x] 移除过期的 M2/M3 展示文案与 macOS `⌘` 快捷键
- [x] 增加中文 Windows 环境下的可读性验收基线（窄窗口、中文字体、Tab 焦点）

## 三点五、M6.5 发布闸门与后续交接

M6.5 是 Windows 发布验收闸门，不再阻塞 M7 代码开发。发布工作只接受 GitHub Actions Windows runner 产物，不在本地构建或安装。

- [ ] 在 main 分支手动运行 Windows release，upload_artifacts=true。
- [ ] 证明安装版、便携版、SHA-256 清单来自同一提交，并且清单文件存在。
- [ ] 在干净 Windows runner/验收机执行安装、启动、导入授权夹具、设置读写、退出重启和卸载检查。
- [ ] 验证升级路径、WebView2 缺失提示、中文字体、1024px 左右窗口、Tab 焦点和数据目录迁移。
- [ ] 保存 Actions 运行链接、产物名称、提交 SHA 和手工验收记录。
- [ ] 签名方案继续暂缓，待用户提供证书与安全存储方案后单独立项。

M7 交接：

- M7.0 已转入“书源元数据保真”：导入并校验来源 URL、分组、类型、权重、发现开关、自定义顺序、备注和书籍 URL 模式；音频书源明确拒绝。
- M7.1 先做 SQLite 元数据迁移和书源列表分组/排序，再扩展分页、变量和调试快照。
- M7.3/M7.4 的 XPath/JavaScript 不得因为导入字段存在就自动启用，必须通过独立安全闸门。

## 三点五、实施记录

- 2026-08-02：提交 UI 可用性修复、真实翻书 SVG、中文优先字体栈与全局阅读设置（设置页、字体、字号、行距、版心、段间距、首行缩进）。
- 2026-08-02：新增安全书源适配器，接受项目导出包、单对象和数组；映射 Legado 常用 CSS 规则，并拒绝 XPath、JavaScript 和未实现的认证规则。
- 2026-08-02：M6.1 扩展 JSON 解析形态（BOM、包装对象）并新增 URL 导入命令；URL 仅允许 HTTP(S)，限制 2 KB 地址和 2 MB 响应体，前端书源页已接入 URL 输入与 UI 契约检查。
- 2026-08-02：新增导入预览和逐项结果：本地文件与 URL 都先展示可导入/跳过原因，确认后只保存通过校验的条目；原始脚本、XPath 和敏感认证头仍不会执行或带入请求。
- 2026-08-02：补齐常见 Legado 别名（`bookName`、`bookAuthor`、`coverUrl`）、多行/JSON 字符串 `header` 解析及 `pageNum/page±1` 占位符回归测试；敏感认证头仍由校验拒绝。
- 2026-08-02：新增 `scripts/verify-ui-contract.mjs` 并接入 CI；Rust 适配器包含原生包、Legado CSS 子集、数组、无效文档、不安全规则和未知属性测试。
- 2026-08-02：新增安全 JSONPath 解析层，支持字符串/对象规则、`jsonPath`/`path` 别名、对象字段、通配数组和数组下标；HTML/CSS 书源路径保持兼容。
- 2026-08-02：TXT 导入剥离 UTF-8 BOM，并在章节内容裁剪时保留首行缩进与空行段落；新增 BOM/缩进回归测试。
- 2026-08-02：EPUB 文本提取改为安全块级转换：过滤 script/style/noscript，保留段落、标题和引用边界，图片转为本地占位文本，并扩展常见 HTML 实体；不执行外部资源，图片映射与缓存仍待后续完成。
- 2026-08-02：EPUB 引入 `blocks-v1` 内容文档和 `content_format` 数据库迁移，前端按白名单渲染标题、段落、引用和 strong/em 文本；旧 text 章节继续走原有纯文本回退。
- 2026-08-02：EPUB 图片仅解析包内安全栅格资源，转为受大小上限保护的本地 data URI；外链、SVG 和超限资源不嵌入，前端拒绝非白名单 URI。
- 2026-08-02：M6.3 完成阅读设置 v2：新增字间距、左右边距、文本对齐、连续/分页滚动、自定义颜色；设置写入 `{version:2,settings}`，启动时兼容旧扁平结构并限幅数值、校验十六进制颜色，恢复默认可立即生效。
- 2026-08-02：M6.4 完成首页概览组件：继续阅读、最近阅读、书籍/在读/已读统计和空状态双入口；新增 `LibraryOverview.vue` 与 `ReaderSettingsPanel.vue`，保留 App 统一编排 Tauri 命令，降低视图条件耦合。
- 2026-08-02：M6.4 继续完成视图拆分：新增 `SourceView.vue`、`RemoteReaderView.vue`、`LocalReaderView.vue`；通过注入运行时上下文保留原有书源导入、搜索和阅读命令，父级仍保留全局 `sourceImportInput` 以避免切换视图后 ref 丢失。
- 2026-08-02：统一阅读器 CSS 变量、分页滚动容器、自定义主题样式、:focus-visible 键盘焦点；移除 M2/M6/⌘ 过期文案，Windows 窄窗口降级为单列布局并保留中文优先字体栈。
- 当前边界：界面导入使用预览和逐条跳过，原有 `import_sources` 命令仍保留整包严格模式供兼容调用；JSONPath 仅支持安全字段/数组遍历，不执行过滤表达式、XPath、JavaScript 或认证逻辑；更多编码和授权夹具仍需后续评估。本轮不在本地构建或安装。
- CI 验证：运行 [30742621733](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/30742621733) 通过 Frontend checks、UI contract check、Rust 格式检查、Cargo check 与 30 个 Rust tests；JSONPath 导入、搜索解析和 TXT BOM/缩进回归测试均包含在本次验证中。
- CI 验证：运行 [30743927928](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/30743927928) 通过 Frontend checks、UI contract check、Rust 格式检查、Cargo check 与 32 个 Rust tests；EPUB 脚本/样式过滤、引用边界和图片占位回归测试均包含在本次验证中。
- CI 验证：运行 [30744561337](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/30744561337) 通过内容块格式的前端/Rust 检查与 33 个 Rust tests。
- CI 验证：运行 [30744789147](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/30744789147) 通过 Frontend checks、UI contract check、Rust 格式检查、Cargo check 与 34 个 Rust tests；内容块、图片本地映射、大小上限和外链拒绝回归测试均包含在本次验证中。
- M6.3/M6.4 验证基线：前端构建必须通过新增组件导入、设置迁移、阅读 CSS 变量和首页契约检查；Windows 安装包需手工确认设置实时生效、重启保持、重置回退、Tab 焦点和 1024px 左右窗口不横向溢出。
- CI 验证：运行 [30746099373](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/30746099373) 通过 Frontend checks（typecheck、Vite build、UI contract）与 Rust checks（fmt、cargo check、Rust tests）；视图组件拆分和设置 v2 契约均已覆盖。

## 四、测试与验收

### 前端

- 设置按钮能打开设置页并保存修改
- 书源按钮能打开系统文件选择器
- 取消选择不会清空当前配置
- 导入成功后列表刷新并选中首个导入项
- 无效 JSON、超限文件和不兼容规则显示具体原因
- 阅读设置修改后实时生效，重启后仍保留
- 自定义主题颜色、字间距、左右边距、对齐和分页滚动在本地与远端阅读器一致
- 书源、本地阅读器和远端阅读器视图切换后，导入 input、键盘 Tab 焦点和中文文案仍可用

### Rust

- 项目导出包、单对象、数组导入
- 字段映射和规则转换
- 无效 JSON、空数组、超限输入
- 不支持能力被拒绝并给出明确错误
- TXT/EPUB 解析回归和旧数据库读取

### Windows

- 安装后设置、书源、书架和阅读器入口均可点击
- 导入授权书源并完成校验
- 导入 TXT/EPUB 后调整字体、背景和版心
- 退出再启动后设置与阅读进度保留
- 不在本地构建或安装；优先通过 GitHub Actions Windows runner 验证

## 五、完成标准

M6.0 只有在 P0 缺陷全部关闭、前端契约检查和 Rust 测试通过、并在 Windows 安装包上完成手工验收后，才进入 M6.1。M6.1 不以“能解析 JSON”为通过标准，而以“用户能看到兼容性边界并安全导入”为通过标准。M6.3/M6.4 的代码任务及远程 CI 验证已完成；在 Windows 安装包手工验收前，不宣称 M6 整体完成。M6.5 仍需下载 GitHub Actions 生成的 Windows 安装包/便携版，按本节验收清单进行手工回归；该发布闸门等待 Actions 权限恢复，但不阻塞 M7 代码切片。全程不在本地构建或安装。
