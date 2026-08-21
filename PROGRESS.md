# 维护进度

## 2026-08-20 当前状态

1. 已核对 PR11、PR12、PR13 合并后的 main 基线 `89185e640cefb2665510fc8b4622d918a9f1ab16`、开放/已关闭 Issues、路线图、阻塞记录与远程 Actions。
2. PR11、PR12、PR13 已按顺序完成审核并合并；PR11 记录文档治理与状态审计，PR12 落地书源快照保留闸门，PR13 补齐 EPUB 标题层级与缺失资源诊断。
3. 合并后 CI run `32368235610`、Windows Release run `32368262290` 和 installer smoke run `32369094467` 全部 success；Artifact `open-reader-windows-main-89185e640cefb2665510fc8b4622d918a9f1ab16` 的摘要为 `sha256:9372402dc2fb734a16fd75cd763c7971b197e54f717b754042557a894ddea7da`。
4. M3/M4、PR9/PR10 视觉与搜索书架重构、M9.3.1-d1 只读预览保持完成；#1/#4/#5 已关闭，#2/#3 仍 open；#2/#3 继续等待目标 Windows 手工验收。
5. 未在本地构建或安装；继续遵守通过 GitHub Actions 验证 Windows 产物、签名暂缓和 d2 迁移前安全闸门。

## 2026-08-21 验收与 M9.3.1-d2

1. 已使用 Computer Use 完成 0.2.0 便携版验收，记录见 [Windows 手工验收记录](docs/windows-manual-acceptance-2026-08-21.md)：搜索与书架分区、在线结果点击反馈、本地阅读位置/已读状态恢复、设置项和在线书源 URL 预览均已验证。
2. 原 P0“在线结果点击无响应”已关闭；`toc/content` 规则不匹配与乱码标题记录为 P1 书源兼容性问题。
3. M9.3.1-d2 已完成：新增 `0016_book_merge.sql`、active/merged 生命周期、操作/来源快照/单跳 alias、纯文本无冲突单事务提交和失败回滚夹具；实现提交 `5184451b38a27123b7ac2330ca957c222f790b26`。
4. CI [32463202309](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/32463202309) 的前端、Linux Rust、Windows 编译/峰值 RSS 采样全部通过；本轮未在本地构建或安装。

## 2026-08-21 M7 P1 书源兼容性首轮切片

1. 已修复 HTML/CSS 书源中 `item` 选择器命中自身节点时被遗漏的问题：搜索结果和目录条目现在会先检查当前节点，再检查后代节点，覆盖“条目本身就是 `<a>`”的常见 Legado 形态。
2. 普通书源响应不再统一按 UTF-8 解码：按 BOM、HTTP `charset`、HTML 元信息和 UTF-8 有效性依次识别 UTF-8、UTF-16LE/BE、GB18030/GBK 与 Windows-1252；声明字符集解码异常时会选择更少替换字符的 GB18030 回退。
3. 书源调试步骤会记录实际解码方式；若仍出现解码替换，会增加脱敏的 `encoding_warning`，便于定位乱码结果而不暴露响应正文。
4. 新增自身节点匹配、GB18030 和 UTF-16LE 合成夹具；远程 CI [32466122037](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/32466122037) 的 Frontend、Rust checks、Windows sampler 全部通过。
5. 本轮仍不执行 XPath/JavaScript，不放宽认证头和网络边界；未在本地构建或安装。

## 2026-08-21 M7 P1 安全备用回退切片

1. 目录解析在原始 `item/title/url` 规则没有产出时，增加受限的章节链接回退：只扫描有限数量的可导航 `a[href]`，并要求标题、class/id 或链接元数据呈现章节特征，不把页面上的任意链接当成目录。
2. 正文解析在规则无匹配时，按优先级尝试 `.content`、`#content`、`article.content`、`.read-content`、`.chapter-content`、`articleBody`、`article` 和 `main` 等安全 CSS 候选，并保留候选节点的 HTML 结构；不会执行 XPath、JavaScript 或认证态逻辑。
3. 新增目录链接与正文候选的合成夹具；首次回归发现正文回退丢失内层 HTML，已修正并由远程 CI [32468240226](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/32468240226) 的 Frontend、Rust checks、Windows sampler 全部验证通过。
4. 回退只在配置规则无结果时启用，仍受 HTTP、响应体、超时、同源和取消边界约束；本轮未在本地构建或安装。

## 当前未完成项

- 已完成：0.2.0 便携版 Computer Use 验收（搜索/书架分区、在线结果点击反馈、本地阅读恢复、设置项和 URL 书源预览）。
- P0：用合并后 main 产物补做目标 Windows 长回归（升级、WebView2、离线/权限错误、字体、窄窗口、Tab 焦点、高对比度和图片路径）。
- 已完成：PR10 的独立搜索工作区、在线结果整行点击反馈、书架分区和 4 列网格已在 0.2.0 候选中核对；仍需合并后产物复验。
- 已完成：M0 文档治理一致性（Issue #1 已关闭）。
- P1：M7 快照清理/键盘收尾、M8 EPUB/PDF/MOBI 评估。
- P1：M7 书源兼容性剩余收尾（真实授权响应夹具、乱码标题降级展示、`toc/content` 字段级差异诊断）和 M9.3.1-d3 撤销设计；M10、M11、M12 依次后置。

## 关键问题

目标 Windows 的安装升级、WebView2、字体、焦点、窄窗口和签名仍需持续人工记录；本轮 Computer Use 已补齐 0.2.0 搜索/书架、阅读恢复、设置和书源预览证据。M9.3.1-d2 已通过远程 CI，物理删除/撤销仍由安全边界禁止。详细处理顺序以状态审计文档为准。

## 2026-08-21 候选修复与下一步

1. PR18 候选 `fa9a1c01bb56be2678fc97ab3a5398d74357b032` 已完成 0.2.0 验收缺陷修复：在线搜索能力判定、阅读位置/已读状态持久化和 128 MiB 书源导入边界。
2. 候选 CI `32401610084`、Windows Release `32402250634` 和 installer smoke `32403363944` 均 success；产物包含 NSIS、MSI、便携 ZIP 和 SHA-256 清单。
3. P1 书源兼容性两轮安全切片已完成：响应编码识别、`item` 自身节点匹配、目录章节链接回退和正文安全 CSS 回退已由 CI 32466122037、32468240226 验证；下一步补充授权响应夹具、乱码标题降级提示和字段级 `toc/content` 差异诊断。
4. M9.3.1-d3 再实现 7 天撤销、旧 ID 单跳跳转、外部修改冲突和 alias 环检测；继续禁止物理删除和静默覆盖。
