# M8 内容格式 v2 计划

M8 负责本地内容导入与阅读模型，不把 EPUB、TXT、MOBI、PDF 或图片漫画混成同一种章节文本。所有格式继续遵守本地文件范围、资源配额和可恢复错误边界。

> 维护复核（2026-08-12）：M8.1–M8.5.5e 的自动化证据已同步到路线图；M9.0–M9.2.5 已承接图片序列的 SQLite 书架、恢复和变更检测。PDF/MOBI/AZW 仍只读探测，缺失资源提示和 PDF 独立渲染评估属于后续阶段。

## M8.1 EPUB/TXT 导入边界（已完成）

- EPUB ZIP 入口限制为最多 2,048 个条目、单条解压后最多 16 MiB、解压后总大小最多 64 MiB。
- ZIP 条目拒绝绝对路径、空路径和 `..` 路径段，避免路径穿越；外链图片不写入本地内容块。
- 现有 EPUB blocks-v1 继续过滤脚本、样式和危险图片资源；本轮只增加配额和路径闸门，不改变已支持的章节数据格式。
- GitHub Actions run 31322842975：前端检查、Rust fmt/check 和 78 个 Rust 测试通过。

## M8.2 TXT 解析可配置化（已完成）

- 已新增 `TxtParseOptions`：自动识别（保持原行为）、不拆分章节、自定义正则三种模式；自定义表达式限制 256 字节，并拒绝空表达式和无效表达式。
- 新增 `preview_book_import` 远程命令，在真正写入书架前返回标题、格式、编码、章节数、首章标题和异常提示；TXT 导入界面可更新预览、确认或取消。
- `import_book_with_options` 只在用户确认后写入 SQLite；EPUB 继续走已有安全边界，超过 64 MiB 的文件仍拒绝。
- 已补齐中文内置章节规则（章/节/回/卷/篇、序章/番外/后记等）、全角空格归一化和最多 32 条文本替换规则，并对每条替换限制长度。
- Rust 夹具覆盖自定义正则、不拆分章节、无效正则、全角空格/替换和 UTF-16LE/BE、GB18030 编码；GitHub Actions run [31325019515](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31325019515)：前端类型检查/构建/UI 合约、Rust fmt/check 和 85 个 Rust 测试通过。
- 后续在 M8.4 增加 TXT 流式解析和性能阈值。

## M8.3 EPUB 阅读结构完善（跨章节跳转、CSS 白名单、损坏恢复与导入诊断已完成）

- `ContentDocument` 已增加 `links` 元数据，收集片段链接和相对文档链接的可读文本；只保留安全的相对/片段目标，并解析到可阅读章节索引。
- `ContentBlock` 已保留经过校验的安全 `anchor` 与有限 `style`，拒绝空白、控制字符、路径分隔符、标记字符和未列入白名单的 CSS；前端再次执行同一组属性和值校验。
- 脚本协议、data/http/https 外链、协议相对链接、绝对路径和包含 `..` 的穿越路径均不会进入链接模型，也不会触发自动跳转。
- 本地阅读器已显示经过同一安全过滤的“本章内部链接”索引；同章片段和安全的跨章节片段链接可加载目标章节并平滑滚动，未解析目标仍只展示索引。
- 损坏 EPUB 现在区分 ZIP 容器、container.xml、OPF 缺失与单个 spine 章节缺失：前者返回明确导入错误，后者跳过坏章节并保留可读章节，最终无可读章节才拒绝。
- GitHub Actions runs [31327448497](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31327448497)、[31327671723](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31327671723)、[31327687242](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31327687242) 和 [31328288413](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31328288413)：前端检查、Rust fmt/check 和 92 个 Rust 测试通过。
- EPUB 导入预览现在保留 h1-h6 标题层级；对缺失或无法读取的 spine 章节、图片与 stylesheet 生成最多 32 条去重提示，并对超出图片单文件/总量配额的资源说明跳过原因；提示只影响预览，不阻止仍可读内容导入。
- 新增损坏/缺失资源 EPUB 夹具，覆盖 h4 章节标题、图片缺失、stylesheet 缺失和空章节恢复；本切片完成后转入 M8.4 流式解析和性能阈值。

## M8.4 大文件与性能基线

- 首个性能切片已让 TXT 按行累积单个章节正文，移除每章 `Vec<String>` 行缓存，并通过字节范围裁剪首尾空行，保留缩进与段落空行。
- `normalize_txt_text` 已改为单次字符扫描，同时归一化 CRLF/CR 换行和全角空格，避免先复制整段文本再做多轮换行替换。
- 无替换规则时，TXT 拆章改为逐行归一化并直接累积章节；有效 UTF-8 使用借用解码路径，避免额外的整段解码副本。
- CI 合成夹具覆盖 512 章约 1 MiB 文本；run [31328789321](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31328789321) 验证行累积，run [31329875168](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31329875168) 验证混合换行归一化，run [31330389289](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31330389289) 验证流式行处理与 UTF-8 借用；最新运行的前端检查、Rust fmt/check 和 95 个 Rust 测试通过。
- 新增 1/16/64 MiB 单章节 TXT 尺寸矩阵，每档解析时间上限 20 秒；run [31330961076](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31330961076) 通过。CI 已在 run [31331505049](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31331505049) 中重复执行基线并上传 artifact [9043104393](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31331505049#artifacts)，包含 `txt-performance-output.log` 与 `txt-performance-time.log`。
- 默认无替换规则路径已接入 64 KiB 分块解码：UTF-16LE/BE 与 GB18030 使用有状态 decoder，跨块多字节字符、CRLF 和尾部 flush 均有夹具；run [31332336289](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31332336289) 的前端检查、Rust fmt/check 和 97 个 Rust 测试通过，性能日志 artifact [9043330887](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31332336289#artifacts)。测试夹具现在记录平台峰值 RSS：Linux 读取 `VmHWM`，Windows 测试路径调用 `GetProcessMemoryInfo`；run [31334298865](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31334298865) 的前端检查、Linux Rust fmt/check、99 个 Rust 测试、真实性能筛选和 Windows 测试编译/采样均通过，artifact [9043889018](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31334298865#artifacts) 已包含日志。Ubuntu 基线记录 1/16/64 MiB 分别为 37/560/2241 ms，采样峰值分别为 13,369,344/63,385,600/239,243,264 bytes。替换规则现使用 64 KiB 有界滚动缓冲，跨块匹配、规则顺序和换行语义均有夹具覆盖。
- M8.4 收口：TXT 默认路径和带替换规则路径均已采用分块/流式处理，64 MiB 默认导入上限继续作为回归边界；EPUB 继续使用 ZIP 条目配额。下一里程碑转入 M8.5 其他格式评估。
- 记录导入耗时、峰值章节数和失败原因的本地测试摘要，不上传遥测；以 1 MiB、16 MiB、64 MiB 合成文件建立 CI 时间和内存回归阈值。

## M8.5 其他格式评估

- M8.5.1 只读格式探测已完成：新增 `BookFormatProbe` 与 `probe_book_format` Tauri 命令，按扩展名和内容签名识别 TXT、EPUB、MOBI/AZW/AZW3、PDF 与 PNG/JPEG/GIF/WebP；MOBI/AZW 使用 PalmDB 记录偏移和 `MOBI` 标记校验，PDF/图片只返回探测结果，不进入文本导入。扩展名与签名不一致会明确拒绝，未知扩展名可按安全魔数提示候选格式。
- M8.5.1 夹具覆盖有效/伪造 MOBI、PDF、图片、TXT、重命名 PDF 和扩展名不匹配；run [31334929174](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31334929174) 的前端、Linux Rust fmt/check、100 个 Rust 测试和 Windows 测试编译/采样通过，性能 artifact [9044080190](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31334929174#artifacts) 正常上传。
- M8.5.2 已增加有界元数据探测：PDF 版本、PNG/GIF/WebP/JPEG 尺寸与 MIME、MOBI/AZW 记录偏移和 MOBI 头长度；图片尺寸限制为 100,000×100,000，JPEG 扫描最多 1 MiB，仍不解码、不导入。run [31335820920](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31335820920) 通过前端、Linux fmt/check、100 个 Rust 测试和 Windows 编译/采样，artifact [9044331289](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31335820920#artifacts) 已上传。
- M8.5.3 已落实统一导入闸门：`require_importable_format` 同时用于导入与预览；已知魔数与扩展名冲突会拒绝，MOBI/AZW/AZW3 仍只读探测并明确不会绕过 DRM；AZW 变体与 MOBI 容器的签名兼容规则有夹具覆盖。run [31336890258](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31336890258) 通过前端、Rust fmt/check、100 个 Rust 测试、TXT 性能证据和 Windows 编译/峰值 RSS 采样，artifact [9044645355](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31336890258#artifacts) 已上传。
- MOBI/AZW：探测和导入边界已完成；后续只有在许可证、解析器维护性、内存上限和 DRM 拒绝策略都明确后，才评估只读解析器。

- M8.5.4a 已把格式探测接入本地文件导入前置流程：只有 importable 才进入预览/导入，PDF、图片和 MOBI/AZW 会显示只读边界及已探测元数据；run [31337644268](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31337644268) 的前端、Rust checks 和 Windows sampler 全部通过，artifact [9044864924](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31337644268#artifacts) 已上传。
- M8.5.4b 已冻结 PDF 与图片/漫画的独立阅读模型、缓存/资源配额、受保护内容拒绝策略和依赖采用闸门，详见 [PDF/图片阅读模型契约](m8-reader-model.md)；本切片不引入解析器或渲染依赖。
- M8.5.5 依赖评估结论已记录在 [M8.5.5 依赖与 Windows 渲染路径评估](m8.5.5-dependency-evaluation.md)：图片优先采用受限 feature 的 Rust 原生解码链，PDF 继续只读探测；依赖与 Windows 渲染路径闸门已收口。
- M8.5.5a 图片单页最小闭环已完成：以 `image 0.25.9` 的受限 feature 解码 PNG/JPEG/GIF/WebP，执行 64 MiB 输入、20,000 边长、32,000,000 像素和 128 MiB 单页解码缓冲配额；Tauri 只返回安全元数据，前端显示单页预览，不写入书架。GitHub Actions run [31350842759](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31350842759) 的前端、Rust、Windows 编译/峰值 RSS 采样全部通过。
- M8.5.5b 图片序列/双页模型已完成：后端提供 `ltr`/`rtl`/`vertical` 阅读方向、`single`/`double`/`long_strip` 排版模式和稳定页索引；序列上限为 2,048 页、128,000,000 总像素、512 MiB 总解码字节，暂不接入多文件书架导入。GitHub Actions run [31351541210](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31351541210) 的前端、Rust 105 项测试、TXT 性能证据和 Windows 编译/峰值 RSS 采样全部通过。
- M8.5.5c 多图片安全预览已完成：后端逐页调用受限解码后再构建序列，额外限制原始输入总量 256 MiB；前端支持多选图片、最多 24 个临时缩略图、方向切换、单页/双页/长图选择和只读预览，不写入书架。GitHub Actions run [31352828101](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31352828101) 的前端、Rust、Windows 编译/峰值 RSS 采样全部通过。
- M8.5.5d1 已完成：前端为每个图片页生成内容摘要，组合为不含绝对路径的序列缓存键，并在本机恢复页码、缩放、方向和排版；当前只保存位置元数据，不写入书架或磁盘缩略图。GitHub Actions run [31353873233](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31353873233) 的前端、Rust、Windows 编译/峰值 RSS 采样全部通过。
- M8.5.5e1-e4 已完成：Rust 在 Tauri 应用缓存目录下按内容摘要键写入 PNG 缩略图；单页最多 8 MiB、缓存总量最多 512 MiB；临时文件完成 sync_all 后 rename 安装，损坏条目自动重建，超额按最早修改时间清理。e4 复用独立取消 token 支持协作式取消，前端提供重试入口；缓存读取命令一次最多恢复当前页及相邻两页，位置恢复后优先使用磁盘缩略图；内容摘要键保留文件选择顺序，换序后不会复用错误页序；清理上一次进程遗留的临时文件。图片序列仍不写入书架或 SQLite。GitHub Actions run [31359196211](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31359196211) 的前端、Rust fmt/check/tests、TXT 性能证据和 Windows 编译/峰值 RSS 采样全部通过。
- 下一阶段需要单独制定 M9 图片序列持久化与书架恢复计划：先冻结数据库 schema、相对路径/文件变更检测、原始文件不可用时的失效提示和恢复测试；PDF 继续独立评估，不转成普通文本章节。
- PDF：独立评估渲染、搜索、目录和页码定位模型，不转成普通文本章节。
- 漫画/图片：独立建模缓存、缩放、双页和阅读方向；不复用文本章节字段。

## 验收规则

每个子切片必须有格式夹具、错误恢复测试、远程 CI 证据和兼容性矩阵更新；本地不构建、不安装。