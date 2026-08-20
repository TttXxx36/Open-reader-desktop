# 维护进度

## 2026-08-20 当前状态

1. 已核对 main 基线 2b5d973、PR10、Issues #1–#5、路线图、阻塞记录与远程 Actions。
2. PR10（fix/search-shelf-workspace）保持 open；head 2ac53b9，CI run 31590842650 的 Frontend checks、Rust checks、Rust Windows sampler 全部 success。
3. M3/M4、PR9 视觉刷新和 M9.3.1-d1 只读预览保持已完成；#4/#5 已关闭，#1/#2/#3 仍 open。
4. 本轮新增 [2026-08-20 开发状态审计与执行记录](docs/development-status-2026-08-20.md)，列出所有未完成项、优先级、依赖、决策和问题。
5. 未在本地构建或安装；不自动合并 PR10，不执行 M9.3.1-d2 的 0016 迁移、物理删除或静默合并。

## 当前未完成项

- P0：维护者审阅/合并 PR10；合并后重新跑 Windows Release/installer smoke。
- P0：目标 Windows 手工验收（升级、WebView2、离线/权限错误、字体、窄窗口、Tab 焦点、高对比度、书源导入和图片路径等）。
- P0：验收 PR10 的独立搜索工作区、在线结果整行点击、书架分区、4 列网格和长按/右键操作。
- P1：M0 文档治理一致性（Issue #1）、M7 快照清理/键盘收尾、M8 EPUB/PDF/MOBI 评估。
- P1：在 P0 证据完成后实施 M9.3.1-d2；d3、M10、M11、M12 依次后置。

## 关键问题

目标 Windows 操作和安装器环境不在当前连接内，CI 不能替代人工记录；PR10 的最新代码尚未进入 main；M9.3.1-d2 仍受迁移安全闸门约束。详细处理顺序以状态审计文档为准。
