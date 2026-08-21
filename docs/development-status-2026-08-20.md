# 2026-08-20 开发状态审计与下一步执行记录

> 本文是当前仓库的状态基线和执行记录，审计范围为 main、开放/已合并 PR、Issues、路线图、阻塞记录与 GitHub Actions。代码“完成”只表示已提交并通过远程检查；Windows 安装、升级、字体、焦点和真实书源导入仍必须有目标 Windows 手工证据。

## 0.1 追加执行结果（2026-08-21）

- Computer Use 已完成 0.2.0 便携版手工验收，完整记录见 [windows-manual-acceptance-2026-08-21.md](windows-manual-acceptance-2026-08-21.md)。原 P0“在线结果点击无响应”已关闭；测试源 `toc/content` 规则不匹配和乱码标题记录为 P1。
- 在 P0 记录完成后，M9.3.1-d2 已实现：`0016_book_merge.sql`、active/merged 生命周期、操作/来源快照/单跳 alias、预览指纹二次校验、纯文本无冲突单事务提交和写入拒绝边界。
- 实现提交 `5184451b38a27123b7ac2330ca957c222f790b26`；CI [32463202309](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/32463202309) 的前端、Linux Rust、Windows 编译/峰值 RSS 采样均通过；本轮未在本地构建或安装。

## 0.2 追加执行结果（2026-08-21）

- M7 P1 首轮切片已完成：HTML/CSS 书源字段选择器先匹配 item 自身节点，再匹配后代节点；普通响应按 BOM、HTTP/HTML `charset`、UTF-8 有效性识别 UTF-8、UTF-16LE/BE、GB18030/GBK 和 Windows-1252，异常时选择替换字符更少的 GB18030 回退。
- 调试步骤只记录 `encoding`，必要时记录 `encoding_warning`；不保存或导出响应正文。新增自身节点、GB18030、UTF-16LE 夹具，CI [32466122037](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/32466122037) 的 Frontend、Rust checks、Windows sampler 全部通过。
- P1 尚未收口：授权真实响应夹具、乱码标题降级展示和 `toc/content` 字段级差异诊断仍需继续；XPath/JavaScript/认证头边界不变。

## 0.3 追加执行结果（2026-08-21）

- M7 P1 诊断与乱码降级代码切片已完成：搜索失败、书籍详情、章节正文和 pipeline 对外提供字段级 `stage/rule_key/status`，前端调试页与脱敏诊断快照均可读取；旧缓存缺少字段时按空数组兼容。
- 编码回退现在统一标记 `encoding_warning`；标题、作者和简介检测到替换字符、控制字符或常见 UTF-8 乱码序列时安全降级，并增加 `book_info.text_quality` 脱敏步骤。
- 新增字段诊断序列化、GB18030 回退警告和乱码字段降级夹具；本地已通过 Rust 格式检查和差异检查，因依赖下载凭据限制未完成本地完整构建，待 GitHub Actions 验证。

## 0. 后续执行结果（2026-08-20）

- **当前 main**：`89185e640cefb2665510fc8b4622d918a9f1ab16`（`89185e6`），已包含 PR10、PR11、PR12、PR13。
- **合并顺序**：PR11 → PR12 → PR13，均已完成审核、Squash Merge，合并后各自 CI 通过。
- **发布证据**：CI `32368235610`、Windows Release `32368262290`、installer smoke `32369094467` 均 success；Artifact `open-reader-windows-main-89185e640cefb2665510fc8b4622d918a9f1ab16`，digest `sha256:9372402dc2fb734a16fd75cd763c7971b197e54f717b754042557a894ddea7da`。
- **当前结论**：自动化发布和安装冒烟验证已完成；目标 Windows 的手工验收（搜索/书架、安装升级、WebView2、字体、焦点、窄窗口、书源导入等）仍是 P0 外部证据，不由 CI 代替。
- **治理状态**：PR11 已补齐本次状态与治理记录；本次文档同步 PR #14 已合并，Issue #1 的许可证/书源兼容政策已收口并按 completed 关闭。Issue #2/#3 继续等待目标 Windows 手工记录。
- **边界**：未在本地构建或安装；M9.3.1-d2 的 0016 迁移、物理删除、静默合并和自动选择 canonical 仍禁止。

## 1. 审计基线与证据

| 项目 | 当前证据 | 判定 |
| --- | --- | --- |
| main 基线 | 提交 89185e640cefb2665510fc8b4622d918a9f1ab16（PR10–PR13 合并后基线） | CI 32368235610、Windows Release 32368262290、installer smoke 32369094467 均通过；目标 Windows 手工验收仍待执行 |
| PR10「分离搜索工作区并重构书架交互」 | 合并提交 b2ee3d4c26423b434036cbc5f646749476e4d91e，状态 merged | 搜索工作区、书架重构和在线条目点击修复已进入 main；仍需目标 Windows 手工验收 |
| PR12「书源快照保留策略」 | 已合并，提交 3278873372c044446ae430fad208942916e78a31 | 20 条/16 MiB 保留闸门与回归夹具已通过，并已进入 main |
| PR13「EPUB 资源诊断」 | 已合并，提交 c0ff28d309880aabeccc9074e1e25a81e48010a8 | h1-h6 标题识别、缺失章节/图片/样式提示已通过自动化，并已进入 main |
| 合并后自动化 | CI run 32354559064、Windows Release run 32354623024、installer smoke run 32355446613 均 success；artifact `open-reader-windows-main-b2ee3d4c26423b434036cbc5f646749476e4d91e`，digest `sha256:0e23015c96e687533bb48c4c474d1def29e197e0c17154b0881f37dc69422630` | 合并后的发布基线和自动化安装回归已通过；不能替代目标 Windows 手工验收 |
| Issues | #1/#4/#5 closed；#2/#3 open | M0 治理已收口；#2/#3 仍等待目标 Windows 验收，已关闭条目不重新打开 |
| 本地构建/安装 | 本轮未执行 | 遵守 Windows Release 通过 GitHub Actions、禁止本地安装的项目约束 |

## 2. 里程碑逐项判定

| 里程碑 | 状态 | 尚未完成的具体内容 |
| --- | --- | --- |
| M0 范围与治理 | 已完成 | MIT License、ADR 0002 及 README、CONTRIBUTING、SECURITY、product-scope、source-protocol、兼容性矩阵已统一；Issue #1 已关闭 |
| M1 工程骨架 | 代码与 CI 完成 | 目标 Windows 开发启动、WebView2 缺失和升级行为仍需人工记录（Issue #2） |
| M2 本地阅读 MVP | 代码与 CI 完成 | TXT/EPUB 导入、重启恢复、离线阅读、中文字体、窄窗口和排版体验仍需目标 Windows 验收（Issue #3） |
| M3 书源协议 | 已完成 | 保持安全子集边界；不把 XPath/脚本/认证态静默降级为 CSS 或匿名请求 |
| M4 单书源端到端 | 已完成 | 继续使用授权合成夹具；真实站点只在用户确认授权后验证 |
| M5 多源与可维护性 | 核心代码完成 | 兼容矩阵的人工回归和安全拒绝提示随 M7/P0 一并收口 |
| M6 Windows 体验与阅读质量 | 代码切片完成 | 安装包回归、中文字体、窄窗口、键盘焦点、高对比度和书源导入体验待人工验收 |
| M6.5 Windows 发布闸门 | Actions 自动化完成 | 原地升级、WebView2 缺失、离线/网络错误、卸载数据保留和正式发布记录待目标 Windows；签名继续暂缓 |
| M6.6 阅读工作区视觉系统 | 已合并（PR9、PR10） | 搜索工作区与书架信息架构已进入 main，合并后 Release/smoke 已通过；等待目标 Windows 体验验收 |
| M7 书源兼容性 v2 | 核心代码完成，P1 安全回退/诊断/降级代码待 CI | item 自身节点匹配、响应字符集识别、目录章节链接回退、正文安全 CSS 回退、字段级 `toc/content` 诊断和乱码标题降级已完成代码切片；仍需授权响应夹具、远程 CI，以及窄窗口/键盘批量操作与拒绝边界真实提示回归 |
| M8 内容格式 v2 | 大部分完成，PR13 自动化已通过 | PR13 已补齐 h1-h6 标题和缺失章节/图片/stylesheet 预览提示；PDF 独立渲染/搜索/目录模型评估与 MOBI/AZW/AZW3 只读解析器评估仍待后续
| M9 书架、元数据与历史 | M9.0–M9.3.1-d3 代码切片完成 | d3 的 0018 撤销状态、7 天撤销、旧 ID 一跳跳转、外部修改冲突和环检测已通过 CI 32470836506；目标 Windows 手工验收仍待完成 |
| M10 备份与恢复 | 未开始 | 版本化 JSON/ZIP、校验和、损坏恢复、导入预览和冲突策略；必须等待 d2 边界稳定 |
| M11 RSS/Atom/OPDS | 未开始 | 只读目录、授权提示、刷新缓存和去重 |
| M12 TTS/漫画/音频评估 | 未开始 | 独立数据模型、播放器/语音许可、缓存和性能基线；音频书源当前保持明确拒绝 |

## 3. 尚未完成项：按优先级与依赖排序

### P0：必须先完成或明确放行

1. **PR10 合并与发布闸门**：已按维护者确认合并到 main（合并提交 `b2ee3d4`），合并后 CI `32354559064`、Windows Release `32354623024` 和 installer smoke `32355446613` 均成功；该项完成。后续仅保留目标 Windows 手工验收。
2. **目标 Windows 手工验收**：使用合并后的 Release/便携产物，逐项记录安装覆盖升级、卸载数据保留、WebView2 缺失、离线/网络/权限错误、中文字体、窄窗口、Tab 焦点、高对比度、本地 JSON/在线 URL 书源导入、导入预览/冲突策略/快照恢复、图片序列中文/UNC 路径、取消重新关联和 M7.2 追链开关。
3. **在线搜索与书架验收**：PR10 的搜索工作区、在线结果整行点击、远端详情打开失败提示、本地/书源书籍分区、4 列书架、长按/右键菜单、重命名/删除命令必须在 Windows 产物中逐项验证。

### P1：P0 证据满足后按依赖推进

4. **M0 文档治理收口（Issue #1，已完成）**：README、CONTRIBUTING、SECURITY、product-scope、source-protocol、compatibility-matrix 已统一链接 ADR 0002，远程 CI 通过，Issue #1 已关闭。
5. **M7 书源兼容性 P1 收尾**：item/编码/安全回退、字段级诊断和乱码降级代码已完成；下一步由 CI 验证，随后只补授权响应夹具与 Windows 手工回归，继续保留 XPath、JavaScript、Cookie、Authorization、音频的显式拒绝。
6. **M8 格式收尾**：PR13 的 EPUB 标题层级与缺失资源诊断已通过 CI，仍需合并评审和 CSS/Windows 回归；完成 PDF/MOBI/AZW 只读评估，不在许可证、供应链、内存和 DRM 拒绝未通过前引入运行时。
7. **M9.3.1-d2**：已完成 0016 schema、旧库默认 active、预览二次校验、单事务纯文本无冲突合并、source snapshot、merged 生命周期和一跳 alias；物理删除、静默覆盖和自动选择 canonical 仍禁止。
8. **M9.3.1-d3**：代码切片已完成 0018 撤销状态、7 天撤销、旧 ID 单跳重定向、外部修改冲突和 alias 环检测，CI 32470836506 通过；下一步为 d3-e Windows 手工验收。

### P2：后续能力

9. **M10** 版本化备份/恢复稳定后，再评估显式手动 WebDAV。
10. **M11** RSS/Atom/OPDS 只读目录与授权边界。
11. **M12** TTS、漫画和音频能力的独立模型与性能/许可评估。

## 4. 本轮关键决策

- 已按维护者确认合并 PR10：合并提交为 `b2ee3d4`；合并后 CI、Windows Release 和 installer smoke 全部成功。
- 不执行物理删除、撤销或静默重复书合并：d2 已落地但仍保持不可逆操作安全边界，撤销和旧 ID 兼容留给 d3。
- 不在本地构建或安装：本轮只通过 GitHub API 提交文档，验证交给 GitHub Actions。
- 不扩大安全执行边界：XPath、JavaScript、Cookie、Authorization、音频和未授权站点继续按兼容性矩阵拒绝或只导入不执行。
- PR10 合并后，最新搜索/书架工作区必须以合并后 main 的 Release/installer smoke 为唯一发布基线。
- PR11、PR12、PR13 已按顺序合并；主分支发布和目标 Windows 手工验收仍分别记录，自动化通过不替代外部手工证据。

## 5. 遇到的问题与处理

- **目标 Windows 手工证据缺失**：当前连接可以读取 Actions，但不能替代用户目标机的安装器、WebView2、字体、焦点和窄窗口操作；因此这些条目标为待验收，不伪造完成。
- **主文档基线滞后**：PR11 创建时记录的是合并前状态；本次在 PR11 分支同步 main `b2ee3d4`、CI/Release/smoke 证据，并保留历史链接作为审计证据。
- **目标 Windows 证据仍缺失**：PR10 的搜索/书架修复已进入 main，合并后自动化已通过；真实安装、字体、焦点、窄窗口和书源导入仍需用户目标机记录。
- **M9.3.1-d3 已完成代码验证**：撤销仅在 7 天窗口和快照未被外部修改时执行；仍不提供物理删除，Windows 手工验收需验证冲突提示和旧 ID 行为。

## 6. 本次产出与下一步

本次 PR 将提交：

- 本文件：完整状态矩阵、未完成条目、优先级/依赖、决策和问题记录；
- PROGRESS.md：更新合并后的 main、PR10、Release、installer smoke 和 artifact 证据；
- README.md：同步 PR10 已合并、Release/smoke 已通过和目标 Windows 手工验收待执行；
- BLOCKED.md：明确 P0 外部验收与 d2 迁移边界；
- docs/roadmap.md：更新最新维护基线并链接本审计。

下一步依赖顺序调整为：**M7 书源兼容性 P1 远程 CI 与授权夹具/Windows 回归 → M9.3.1-e Windows 撤销与旧 ID 验收 → M7/M8 横向收尾 → M10 备份/恢复**。Windows 安装、升级、字体、焦点和窄窗口仍需用户目标环境继续补充证据。
