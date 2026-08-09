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

## M8.3 EPUB 阅读结构完善（片段定位切片已完成）

- `ContentDocument` 已增加 `links` 元数据，收集片段链接和相对文档链接的可读文本；只保留安全的相对/片段目标。
- `ContentBlock` 已保留经过校验的安全 `anchor`，拒绝空白、控制字符、路径分隔符和标记字符，避免把任意 HTML 属性变成 DOM 定位器。
- 脚本协议、data/http/https 外链、协议相对链接、绝对路径和包含 `..` 的穿越路径均不会进入链接模型，也不会触发自动跳转。
- 本地阅读器已显示经过同一安全过滤的“本章内部链接”索引；片段链接可在当前章节内平滑滚动到安全锚点，跨文档链接暂只展示索引，不离开当前阅读器。
- GitHub Actions runs [31326288298](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31326288298)、[31326332655](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31326332655) 和 [31326338416](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31326338416)：前端检查、Rust fmt/check 和 86 个 Rust 测试通过。
- 剩余工作：跨章节跳转、有限 CSS 白名单、标题层级/缺失资源提示，以及损坏 container/OPF/manifest/spine 的恢复夹具。

## M8.4 大文件与性能基线

- 64 MiB 默认导入上限下，TXT 采用分块/流式处理路径，避免一次性复制多份正文；EPUB 继续使用 ZIP 条目配额。
- 记录导入耗时、峰值章节数和失败原因的本地测试摘要，不上传遥测。
- 以 1 MiB、16 MiB、64 MiB 合成文件建立 CI 时间和内存回归阈值。

## M8.5 其他格式评估

- MOBI/AZW：先做只读识别和依赖许可证审查，再决定是否加入解析器。
- PDF：独立评估渲染、搜索、目录和页码定位模型，不转成普通文本章节。
- 漫画/图片：独立建模缓存、缩放、双页和阅读方向；不复用文本章节字段。

## 验收规则

每个子切片必须有格式夹具、错误恢复测试、远程 CI 证据和兼容性矩阵更新；本地不构建、不安装。