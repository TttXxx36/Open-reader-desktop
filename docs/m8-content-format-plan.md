# M8 内容格式 v2 计划

M8 负责本地内容导入与阅读模型，不把 EPUB、TXT、MOBI、PDF 或图片漫画混成同一种章节文本。所有格式继续遵守本地文件范围、资源配额和可恢复错误边界。

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

## M8.3 EPUB 阅读结构完善（跨章节跳转、CSS 白名单与损坏恢复切片已完成）

- `ContentDocument` 已增加 `links` 元数据，收集片段链接和相对文档链接的可读文本；只保留安全的相对/片段目标，并解析到可阅读章节索引。
- `ContentBlock` 已保留经过校验的安全 `anchor` 与有限 `style`，拒绝空白、控制字符、路径分隔符、标记字符和未列入白名单的 CSS；前端再次执行同一组属性和值校验。
- 脚本协议、data/http/https 外链、协议相对链接、绝对路径和包含 `..` 的穿越路径均不会进入链接模型，也不会触发自动跳转。
- 本地阅读器已显示经过同一安全过滤的“本章内部链接”索引；同章片段和安全的跨章节片段链接可加载目标章节并平滑滚动，未解析目标仍只展示索引。
- 损坏 EPUB 现在区分 ZIP 容器、container.xml、OPF 缺失与单个 spine 章节缺失：前者返回明确导入错误，后者跳过坏章节并保留可读章节，最终无可读章节才拒绝。
- GitHub Actions runs [31327448497](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31327448497)、[31327671723](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31327671723)、[31327687242](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31327687242) 和 [31328288413](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31328288413)：前端检查、Rust fmt/check 和 92 个 Rust 测试通过。
- 剩余工作：标题层级/缺失资源提示；M8.3 收尾后转入 M8.4 流式解析和性能阈值。

## M8.4 大文件与性能基线

- 首个性能切片已让 TXT 按行累积单个章节正文，移除每章 `Vec<String>` 行缓存，并通过字节范围裁剪首尾空行，保留缩进与段落空行。
- `normalize_txt_text` 已改为单次字符扫描，同时归一化 CRLF/CR 换行和全角空格，避免先复制整段文本再做多轮换行替换。
- 无替换规则时，TXT 拆章改为逐行归一化并直接累积章节；有效 UTF-8 使用借用解码路径，避免额外的整段解码副本。
- CI 合成夹具覆盖 512 章约 1 MiB 文本；run [31328789321](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31328789321) 验证行累积，run [31329875168](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31329875168) 验证混合换行归一化，run [31330389289](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31330389289) 验证流式行处理与 UTF-8 借用；最新运行的前端检查、Rust fmt/check 和 95 个 Rust 测试通过。
- 新增 1/16/64 MiB 单章节 TXT 尺寸矩阵，每档解析时间上限 20 秒；run [31330961076](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31330961076) 通过。CI 已在 run [31331505049](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31331505049) 中重复执行基线并上传 artifact [9043104393](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31331505049#artifacts)，包含 `txt-performance-output.log` 与 `txt-performance-time.log`。
- 默认无替换规则路径已接入 64 KiB 分块解码：UTF-16LE/BE 与 GB18030 使用有状态 decoder，跨块多字节字符、CRLF 和尾部 flush 均有夹具；run [31332336289](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31332336289) 的前端检查、Rust fmt/check 和 97 个 Rust 测试通过，性能日志 artifact [9043330887](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31332336289#artifacts)。替换规则路径暂保留完整文本语义，后续补充跨平台峰值内存采样与替换规则的流式策略。
- 后续在 64 MiB 默认导入上限下接入分块/流式编码解码，避免一次性复制多份正文；EPUB 继续使用 ZIP 条目配额。
- 记录导入耗时、峰值章节数和失败原因的本地测试摘要，不上传遥测；以 1 MiB、16 MiB、64 MiB 合成文件建立 CI 时间和内存回归阈值。

## M8.5 其他格式评估

- MOBI/AZW：先做只读识别和依赖许可证审查，再决定是否加入解析器。
- PDF：独立评估渲染、搜索、目录和页码定位模型，不转成普通文本章节。
- 漫画/图片：独立建模缓存、缩放、双页和阅读方向；不复用文本章节字段。

## 验收规则

每个子切片必须有格式夹具、错误恢复测试、远程 CI 证据和兼容性矩阵更新；本地不构建、不安装。