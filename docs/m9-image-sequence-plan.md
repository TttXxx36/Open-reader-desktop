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
6. 目录扫描现在有 4096 文件、512 MiB 总量和 15 秒耗时上限；重新关联扫描已支持协作式取消令牌，前端可在扫描期间主动取消，取消点位于目录/文件条目安全检查点。

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

### 9.2.5 首切片：扫描超时与路径安全反馈

- 统一 Windows 驱动器、UNC 和 POSIX 路径拼接规则；
- 对根目录、页相对路径做规范化和越界拒绝；
- 只读取受控元数据：存在性、文件大小、修改时间和按需 SHA-256；
- 目录扫描设置页数、总字节数和总耗时配额；当前已实现文件数/总字节数/15 秒耗时上限，并在目录/文件条目循环中检查协作式取消令牌；
- 重新关联预览把“超过时间上限”与普通读取失败分开提示，超时或失败都不会修改旧目录和旧缓存；
- 重新关联扫描通过 `operation_id` 注册取消令牌；前端显示“取消扫描”，取消、超时或普通失败都只返回错误，不触碰旧根目录、旧页路径和缩略图缓存；
- 超时首切片提交 [eb520ff4fd88a992f5b3ccdd10db1443c90a95eb](https://github.com/TttXxx36/Open-reader-desktop/commit/eb520ff4fd88a992f5b3ccdd10db1443c90a95eb) 对应 CI run [31378331544](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31378331544) 已通过前端、Rust fmt/check/tests、TXT 性能证据和 Windows 编译/峰值 RSS 采样；取消令牌最终提交 [8e234f37290ec42f048702064270159011a36904](https://github.com/TttXxx36/Open-reader-desktop/commit/8e234f37290ec42f048702064270159011a36904) 对应 CI run [31380314725](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31380314725) 已通过同一组检查。

## M9.3 首切片：书架元数据与查询

M9.3 首切片已完成，并由 main CI run [31385269180](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31385269180) 验证通过：

- 新增迁移 `0014_book_shelf_metadata.sql`，为书籍增加 `shelf_group`、`tags_json` 和 `custom_order`，保留现有封面路径字段并建立分组/顺序索引；
- 数据库层新增可选分组、标题/作者/标签搜索、固定白名单排序和升降序查询；元数据写入会校验分组、去重标签、限制数量与长度，并在事务后返回最新书籍摘要；
- 前端书架新增搜索、分组筛选、排序和升降序切换；书籍卡片展示分组/标签，并支持单本分组、标签和自定义顺序编辑；
- 首页最近阅读卡片同步展示分组和标签，便于确认元数据已进入统一查询模型。

本切片只覆盖可自动化验证的查询与单本编辑基础；真实 Windows 安装版/便携版中的中文路径、UNC 路径、目录重新关联取消交互和断目录恢复仍属于独立手工验收项，不能用 CI 代替。

### M9.3 批量元数据首切片

批量书架治理的首个可验收切片已完成，最终提交 [a15683ede69e4f408dc3b217d7f8bd09be572722](https://github.com/TttXxx36/Open-reader-desktop/commit/a15683ede69e4f408dc3b217d7f8bd09be572722) 对应 main CI run [31388184266](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31388184266)：

- Rust 数据库层新增 `BookMetadataBatchWrite` 和事务化批量更新；书籍 ID 去重并限制单次最多 256 本，分组/标签沿用单本编辑的长度、数量和去重校验；
- 批量更新支持只改分组或只改标签，空分组/空标签可主动清空；任意书籍不存在或写入失败都会回滚整批事务；
- Tauri 新增 `update_books_metadata` 命令；前端书架支持多选、当前筛选结果全选/取消全选、批量分组和标签编辑，并在刷新列表后保留可见选择；
- 前端 typecheck/build、UI 契约、Rust fmt/check/tests 和 Windows 编译/峰值内存采样均已通过。

基于该提交的 Windows Release run [31388689695](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31388689695) 已生成安装版、便携版和 SHA-256 清单，artifact [9063016672](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31388689695#artifacts)，artifact 摘要为 `sha256:be69fe9375574afa7e4db529039fae0699e2a3497200e6408d38ebadd2d11232`；installer smoke run [31389443472](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31389443472) 已通过产物校验、便携版启动、NSIS/MSI 安装卸载和数据保留自动化检查。真实 Windows 手工升级、WebView2、中文/UNC 路径和目录恢复交互仍需用户环境补验。

### M9.3 重复候选预览首切片

重复书治理的只读预览已完成，提交 [789efa3ca44bc5bb42c6a274a814fe64c866b776](https://github.com/TttXxx36/Open-reader-desktop/commit/789efa3ca44bc5bb42c6a274a814fe64c866b776) 对应 main CI run [31390991820](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31390991820)：

- 数据库按规范化书名、作者和格式分组，最多返回 128 组、每组最多 256 条记录；只读查询不写入书架；
- Tauri 提供 `find_duplicate_books`；书架增加“检查重复书”面板，列出候选记录、章节数、进度和分组；
- UI 明确当前只做候选预览，不自动删除、覆盖或选择保留项；
- 最新 `main` 的 Windows Release run [31404728684](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31404728684)（提交 `bfbeb409e76b1332e933ef5c525744909126bf7d`）已成功生成安装版、便携版和 SHA-256 清单；artifact [9069420519](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31404728684#artifacts)，摘要为 `sha256:1a851633a49bb83005d6b1ac0a8e8b93bcc743634fa1548481ccebfed633e1a6`。随后 installer smoke run [31405543069](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31405543069) 已通过产物/校验和、便携版启动、NSIS/MSI 安装卸载和数据保留自动化检查。

真正的合并操作仍需先确定：保留书籍的章节和阅读进度来源、标签/分组合并规则、图片序列及根目录冲突、重复书回滚/撤销能力，以及合并后旧 ID 的跳转兼容；在这些规则写入新的数据模型和迁移前，不执行不可逆删除。

### M9.3.1 设计阶段：封面缓存与可撤销重复书合并

M9.3 的查询、单本编辑、批量元数据和重复候选只读预览已经完成。下一步设计已单独记录在 [M9.3.1 封面缓存与可撤销重复书合并设计](m9.3.1-cover-merge-plan.md)，9.3.1-a/b/c/d1 安全首切片已通过，当前进入 9.3.1-d2 的迁移前评审：

- 封面采用“来源元数据 + 本机缓存”两层模型，区分本地路径、显式刷新远程 URL 和占位图；默认不抓取远程封面，不保存凭据或敏感请求头；
- 缓存键使用版本化内容/文件指纹，临时文件写入后原子安装，并设置单文件 8 MiB、总量 256 MiB 的配额；失效时回退到占位图，不阻塞正文阅读；
- 重复书合并要求用户显式选择保留项，先生成带输入指纹和过期时间的预览；任何章节、进度、分组/标签、封面或图片序列冲突默认阻止提交；
- 后续迁移将保留原始书籍和章节，通过归档状态、别名和操作快照实现事务及 7 天撤销；在模型和纯读取预览通过前，不执行删除或自动覆盖；
- 9.3.1-a 已完成首个纯函数切片：`src-tauri/src/cover.rs` 负责本地/远程封面来源规范化和版本化缓存键，提交 [91bc285d654568b942fbcc59e779c6efb43ea79d](https://github.com/TttXxx36/Open-reader-desktop/commit/91bc285d654568b942fbcc59e779c6efb43ea79d) 对应 CI run [31394712988](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31394712988) 已通过；
- 9.3.1-b 已完成封面缓存文件层与 `book_covers` 元数据首切片：`src-tauri/src/cover_cache.rs` 提供 8 MiB 单文件、256 MiB 总量、有界读取、临时文件清理、原子安装和最旧项清理；迁移 `0015_book_covers.sql`、`get_book_cover`/`save_book_cover` 和 `BookSummary.cover_state` 已接入书架与最近阅读状态展示，提交 [a91d1af7ffb1c5c8ccc91187615550a65cca2647](https://github.com/TttXxx36/Open-reader-desktop/commit/a91d1af7ffb1c5c8ccc91187615550a65cca2647) 对应 CI run [31402819722](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31402819722) 已通过；远程封面仍默认关闭，实际解码/刷新后续实现；
- 9.3.1-c 的纯只读重复合并预览已完成：`preview_book_merge` 限制 2—8 本、显式 canonical，返回章节/进度/元数据/封面候选、冲突和图片序列阻断原因，并生成 5 分钟 preview_id 与输入指纹；书架面板只读展示，提交 [a91d1af7ffb1c5c8ccc91187615550a65cca2647](https://github.com/TttXxx36/Open-reader-desktop/commit/a91d1af7ffb1c5c8ccc91187615550a65cca2647) 对应 CI run [31402819722](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31402819722) 已通过；过期/输入变化二次校验和实际事务仍留在 9.3.1-d。
- 9.3.1-d1 已完成首切片：`revalidate_book_merge_preview` 重新计算当前输入指纹和 5 分钟有效期，过期或数据变化拒绝继续；书架预览增加“重新验证”按钮，仍保持只读。提交 [f18f9ccf539a1059544df9657308127a983689ad](https://github.com/TttXxx36/Open-reader-desktop/commit/f18f9ccf539a1059544df9657308127a983689ad) 对应 CI run [31409256256](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31409256256) 已通过前端、Rust fmt/check/tests/TXT 性能证据和 Windows 编译/峰值 RSS 采样。

## 后续顺序

M9.2 的自动化切片已完成；最新 Windows release run [31391524135](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31391524135) 已在 `main` 生成包含 M9.3 重复候选预览首切片的安装版、便携版和 `release-sha256.txt`，artifact [9064180342](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31391524135#artifacts) 摘要为 `sha256:473ff8c0717e82ae7fc2268e5fe15775083f5786670890138dfa0a2ea235e616`；installer smoke run [31392423285](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31392423285) 已通过自动化安装/卸载和启动检查。 最新 `main` 的 Windows Release run [31404728684](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31404728684)（提交 `bfbeb409e76b1332e933ef5c525744909126bf7d`）已成功生成安装版、便携版和 SHA-256 清单；artifact [9069420519](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31404728684#artifacts)，摘要为 `sha256:1a851633a49bb83005d6b1ac0a8e8b93bcc743634fa1548481ccebfed633e1a6`。随后 installer smoke run [31405543069](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31405543069) 已通过产物/校验和、便携版启动、NSIS/MSI 安装卸载和数据保留自动化检查。M9.3.1-a/b/c/d1 已通过 main CI run [31409256256](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31409256256)，真实 Windows 手工验收（安装包/便携版、中文与 UNC 路径、升级、取消交互、断目录后的状态恢复）仍待用户在目标环境完成并保存记录。当前暂停在 9.3.1-d2 的迁移前评审：先冻结 `0016` 字段、归档/别名/快照和撤销冲突策略，再决定是否进入事务合并；任何需要原生目录选择器或 Windows 手工 UI 验收的工作，都先补充对应 CI 产物和验收记录。
