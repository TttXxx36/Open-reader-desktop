# M9 图片序列数据库化与书架恢复计划

## 目标与边界

M9 将图片序列从“只读预览 + 本机缩略图缓存”推进为可查询、可恢复、可检测失效的书架记录。图片原始文件仍保留在用户指定的本机目录，不复制进 SQLite；SQLite 只保存书籍元数据、序列布局、页级路径和可用于变更检测的指纹。

本计划不把图片序列强行映射为文本章节，也不引入 PDF 渲染或脚本执行；目录访问、文件校验、缩略图缓存和阅读进度必须继续受配额约束。

## 已完成切片

### M9.0：SQLite 模型与迁移

迁移 '0013_image_sequences.sql' 已加入：

- 'books.content_kind'，区分 'text' 与 'image_sequence'；
- 'library_roots'，记录绝对根路径、可用性和最近校验时间；
- 'image_sequences'，记录缓存键、页数、总像素、方向、排版、当前页、缩放、状态和进度；
- 'image_sequence_pages'，记录相对路径、文件大小、修改时间、可选内容摘要、摘要版本、MIME、尺寸和页状态。

数据库连接现在显式启用外键约束；旧书籍通过默认值继续按文本书籍读取。相对路径拒绝绝对路径、遍历段、重复分隔符、控制字符和 ADS 风格片段；根目录必须是 POSIX、Windows 驱动器或 UNC 绝对路径。

### M9.1：书架保存、重启恢复和进度回写

Rust 数据库层已提供：

- 'save_image_sequence'：事务化 upsert 根目录、书籍、序列和页记录；
- 'list_image_sequences' / 'get_image_sequence'：书架查询和完整页记录恢复；
- 'save_image_sequence_progress'：页码、缩放、方向和排版回写，并同步书架进度；
- 书架点击图片序列时，前端从 SQLite 恢复预览和缓存页，文本书籍仍走原阅读器。

前端已提供：

- 图片序列标题和绝对根目录输入；
- 保存到书架按钮，保存前继续执行原有图片解码和缓存配额；
- 文件选择器的 'webkitRelativePath' 保留为页相对路径；
- 页码、缩放、方向、排版的本机位置保存和已入库记录回写；
- 重启后通过书架记录恢复当前页及相邻缩略图；
- 缺失/过期状态的提示文案，明确告知重新关联能力尚未进入本切片。

验证：提交 '177f54905fc61d9f7a4e93a5d970c1686c1002e8' 对应 CI run [31363237878](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31363237878) 已通过 Frontend checks、Rust fmt/check/tests、TXT 性能证据和 Windows Rust 编译/峰值 RSS 采样。此前 M9.0 数据库闭环由 CI run [31362116283](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31362116283) 验证。

## 当前明确限制

1. M9.2.3 已接入 Tauri dialog 原生目录选择器；新建序列可以直接填充绝对根目录，已入库序列通过选择新目录进入差异扫描。
2. 保存时页级修改时间可能为空；首次打开书架会以文件大小和当前修改时间建立快速指纹基线。
3. 当前状态检测先读取存在性、文件大小和修改时间；只有用户点击“复核变化页”时，才对 stale 页按需计算 SHA-256，不在每次打开书架时读取全部图片。
4. 'missing'、'stale'、'needs_relink' 已可自动判定并显示；M9.2.3 已完成相对路径/文件名+大小候选、差异预览和事务化换根目录；后续切片已加入按需 SHA-256 内容确认，摘要一致的 stale 页可恢复为 ready。
5. 重新关联只更新 SQLite 中的根目录和页相对路径，不复制原始图片；未匹配页保留为 missing，旧根目录记录不删除，但目前还没有单独的回滚按钮。

## M9.2：文件变更检测与重新关联（第一切片已完成，后续工作仍在本阶段）

### 9.2.1/9.2.2 已完成：路径适配、快速指纹与状态判定

- resolve_image_page_path 在文件系统边界再次规范化根目录和相对页路径，拒绝越界路径；
- 打开书架记录时调用 refresh_image_sequence_state，检查根目录是否可用，并逐页读取存在性、文件大小和修改时间；
- 首次保存的空修改时间会在文件大小一致时建立基线；文件变化进入 stale，文件消失进入 missing，根目录不可用进入 needs_relink；
- 刷新结果以事务方式写回 library_roots、image_sequences 和页状态；前端展示可用、变化和缺失页数量；
- 数据库夹具覆盖首次基线、单页变化、单页删除、根目录删除和重新关联提示，路径夹具覆盖 Windows 分隔符与越界拒绝。

提交 [a14795a45f6e0127e4a791fc47a716f48a87d3ff](https://github.com/TttXxx36/Open-reader-desktop/commit/a14795a45f6e0127e4a791fc47a716f48a87d3ff) 对应 CI run [31368081755](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31368081755) 已通过前端 typecheck/build/UI 契约、Rust fmt/check/tests、TXT 性能证据和 Windows 编译/峰值 RSS 采样。

### 9.2.3 已完成首切片：原生目录选择与差异预览

- Tauri dialog 插件提供 Windows 原生目录选择；扫描跳过符号链接，只收集 PNG/JPEG/GIF/WebP；
- 先按相对路径匹配，再按“文件名 + 文件大小”寻找移动候选；候选上限 4096 个文件、总大小 512 MiB；
- 差异预览展示 matched、changed、missing、added 和 reordered，并列出前 8 页候选；
- 用户确认后由 Rust 事务化更新 library_roots、books.path、image_sequences 和页相对路径；未匹配页保留为 missing，待复核页保留为 stale；
- 重新关联期间文件消失、大小变化或预览过期会直接失败，旧根目录、页路径和缩略图记录不会被改动；
- 提交 [3f20acd68ac4890d24d2992fc7fcf766729cafa2](https://github.com/TttXxx36/Open-reader-desktop/commit/3f20acd68ac4890d24d2992fc7fcf766729cafa2) 对应 CI run [31370651374](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31370651374) 已通过前端、Rust fmt/check/tests、TXT 性能证据和 Windows 编译/峰值 RSS 采样。

### 9.2.3 后续切片已完成：按需 SHA-256 复核与 stale 恢复

- `image_relink::sha256_file` 使用 64 KiB 分块读取；单页最多复核 64 MiB，本次最多复核 256 MiB，超过任一上限会在事务开始前失败；
- `verify_image_sequence_digests` 先复用快速状态检测，再只处理当前标记为 `stale` 的页；目录根不存在时保持 `needs_relink`，缺页转为 `missing`；
- 有已保存摘要且摘要一致的页转回 `ready` 并更新观察到的修改时间；摘要不一致继续保持 `stale`；没有历史摘要的旧页不会被“大小相同”误报为正常；
- 数据库更新仍是单事务，读取错误、单页/总量超限或路径越界不会写入半成品状态；前端仅在存在 stale 页时显示“复核变化页”按钮，并在完成后刷新当前页和相邻缩略图；
- 提交 [1eeeaf02702c83648273890662587cc034df9321](https://github.com/TttXxx36/Open-reader-desktop/commit/1eeeaf02702c83648273890662587cc034df9321) 对应 CI run [31375034783](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31375034783) 已通过前端 typecheck/build/UI 契约、Rust fmt/check/tests、TXT 性能证据和 Windows 编译/峰值 RSS 采样。

### 9.2.4 阅读器与书架反馈（已完成首轮）

- `BookSummary` 现在带有图片序列状态、缺页数和待复核页数；首页继续阅读、最近阅读和书架卡片显示“图片正常 / 待复核 / 缺页 / 目录需重新关联”状态；
- 打开已入库图片序列时会先执行快速状态检测，并把最新状态同步回书架内存；书架展示不依赖“缓存文件存在”来判断原始图片可用；
- 阅读器当前页缺失时隐藏可能过期的缩略图，显示“重新关联目录”操作；stale 页显示原文件变化提示和“复核变化页”操作；
- SHA-256 复核完成后重新加载当前页及相邻页缩略图；进度回写仍受有效页范围和状态提示约束；
- 提交 [5f5b69121c6824df529b53cbed849edfc1a3ff7f](https://github.com/TttXxx36/Open-reader-desktop/commit/5f5b69121c6824df529b53cbed849edfc1a3ff7f) 对应 CI run [31377102734](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31377102734) 已通过前端、Rust fmt/check/tests、TXT 性能证据和 Windows 编译/峰值 RSS 采样。

### 9.2.5 验收与性能

- 统一 Windows 驱动器、UNC 和 POSIX 路径拼接规则；
- 对根目录、页相对路径做规范化和越界拒绝；
- 只读取受控元数据：存在性、文件大小、修改时间和按需 SHA-256；
- 目录扫描设置页数、总字节数和总耗时配额，并支持取消。

### 9.2.2 指纹与状态机

- 首次保存后记录 'file_size + modified_at_ns' 快速指纹；
- 快速指纹变化时才按需计算内容摘要，避免每次启动读取全部文件；
- 页面状态转换：'ready -> stale'、'ready -> missing'、局部恢复后的 'stale -> ready'；
- 根目录不可用时序列进入 'needs_relink'，不得把“缓存可读”误报为“原文件可用”；
- 记录检测时间、变更原因和受影响页数量，供诊断展示。

### 9.2.3 重新关联工作流

- 原生目录选择器选择新根目录；
- 先按相对路径匹配，再按文件名+大小寻找候选；应用后由按需 SHA-256 复核确认内容；
- 产生预览差异：匹配、缺失、新增、内容变化、顺序变化；
- 用户确认后事务化更新 'library_roots' 和页记录，保留当前页可见性；
- 重新关联失败时保留旧路径和旧缓存，不破坏可回滚状态。

### 9.2.4 阅读器与书架反馈

- 书架卡片显示可用、部分失效、目录缺失状态；
- 阅读器遇到缺页时显示可操作提示，不让缓存缩略图掩盖原文件不可用；
- 状态恢复后自动刷新当前页和相邻页缓存；
- 进度回写只允许有效页范围，目录变更后进行页码夹紧和提示。

### 9.2.5 验收与性能

- Windows 驱动器/UNC、POSIX、中文和空格路径夹具；
- 目录移动、单页删除、单页替换、批量重命名和顺序变化夹具；
- 快速指纹命中、摘要重算、取消和超时夹具；
- 1/128/2,048 页扫描矩阵，记录耗时、读取字节和峰值 RSS；
- 数据库升级、事务回滚、缓存仍可读但原文件缺失等恢复测试；
- GitHub Actions 继续执行前端检查、Rust 测试和 Windows 采样；本地不构建、不安装。

## 后续顺序

完成 M9.2 后，再进入 M9.3 书架分组/标签/排序与封面策略；随后按路线图进入 M10 备份恢复。任何需要原生目录选择器或 Windows 手工 UI 验收的工作，都先补充对应 CI 产物和验收记录。
