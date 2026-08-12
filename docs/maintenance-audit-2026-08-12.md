# 2026-08-12 GitHub 维护审计

## 审计基线

- 仓库：TttXxx36/Open-reader-desktop，main 提交 [0e73968](https://github.com/TttXxx36/Open-reader-desktop/commit/0e73968ce14dea5e53f613c9df66d97f32316a72)。
- Issues #1–#5 在审计开始时全部为 open，且没有历史评论。
- CI [31574147034](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31574147034)、Windows release [31574767135](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31574767135) 和 installer smoke [31575465554](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31575465554) 均以 success 完成。
- 自动化结果不替代目标 Windows 环境人工验收；签名方案继续暂缓。

## Issue 决策

| Issue | 验收判断 | 证据 | 本轮动作 |
| --- | --- | --- | --- |
| #1 M0 许可证与兼容政策 | 部分完成 | 根目录已有 MIT License；但 README、CONTRIBUTING、SECURITY 和兼容性矩阵尚未完全统一，缺少独立可引用的许可证/书源政策决策记录 | 保持 open，评论文档治理缺口；不宣称 M0 完成 |
| #2 M1 工程骨架 | 代码与 CI 完成，启动验收未闭环 | Tauri/Vue/Rust/SQLite 工程、前端/Rust 检查和 Actions 已通过；新开发者按文档安装并启动开发窗口尚未在目标环境记录 | 保持 open，评论待验收项 |
| #3 M2 本地 TXT/EPUB MVP | 代码与自动化测试完成，Windows 阅读体验未闭环 | TXT/EPUB、目录、进度、设置和 SQLite 代码及 Rust/Windows 检查已通过；目标 Windows 导入、重启恢复、离线阅读和排版体验尚无手工记录 | 保持 open，评论待验收项 |
| #4 M3 书源协议 | 已满足当前验收边界 | `docs/source-protocol.md`、兼容性矩阵和本机合成夹具覆盖统一模型、解析、超时/大小/错误边界；CI 31574147034 通过 | 评论证据后关闭 |
| #5 M4 单书源端到端 | 已满足当前安全边界 | `docs/source-pipeline.md` 的本机临时 TCP 合成夹具覆盖搜索→详情→目录→正文和脱敏诊断；脚本不执行而是明确拒绝，真实站点不使用私人账号；CI 31574147034 通过 | 评论证据后关闭；脚本运行时另由 M7.4 评估 |

## 文档同步结果

- README：移除 M6.5 等待 Actions 权限恢复的过时描述，补充 M7–M9、视觉刷新、16 MiB 书源导入边界和当前 Windows 人工验收清单。
- docs/development.md：区分普通远程请求的 2 MiB 边界与书源配置导入的 16 MiB/30 秒边界，补充 M6.6 视觉系统和最新 Actions 证据。
- docs/release-checklist.md：更新当前 release/smoke run，明确自动化已覆盖安装/卸载而覆盖升级、WebView2 和 UI 体验仍需人工验收。
- docs/compatibility-matrix.md：同步图片序列 M9.0–M9.2 状态和书源导入边界，保留 XPath/JavaScript/Cookie/Authorization/音频的明确拒绝。
- docs/source-pipeline.md：补充 16 MiB 配置导入、30 秒在线拉取和当前 Actions 证据。
- M7/M8/M9 专项计划与 roadmap：补充本次维护复核、当前执行顺序和后续阶段入口。

## 后续三阶段

1. P0 目标 Windows 人工验收：覆盖安装升级、WebView2、离线/网络错误、中文字体、窄窗口、键盘焦点、高对比度和书源导入/冲突/恢复。
2. M9.3.1-d2 迁移前收口：冻结 0016 schema，先通过旧库/回滚/冲突夹具，再实现纯文本、无正文冲突、可回滚合并；禁止物理删除和静默覆盖。
3. M7.1/M8 横向收尾后进入 M10：补快照清理、缺失资源提示和可访问性；再实现版本化备份/恢复，之后评估 WebDAV、RSS/OPDS、TTS/音频。

## 结论

当前不能把五个 issue 全部关闭。#4、#5 可以在本审计评论后关闭；#1–#3 保持 open，直到各自剩余的治理或目标 Windows 验收有可追溯记录。
