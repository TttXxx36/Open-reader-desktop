# Open Reader Desktop

Windows-first open-source desktop reader inspired by the extensible reading experience of Legado/阅读。

> 状态（2026-08-20）：main 基线为 2b5d973；M0–M9.3.1-d1 的代码切片与远程自动化验证持续通过。PR10 已完成搜索工作区/书架重构和在线条目点击修复，CI run 31590842650 全部成功，但 PR10 尚未合并；正式发布仍需合并后 Release/smoke 与目标 Windows 手工验收，签名暂缓。

## 目标

- 本地优先：支持 TXT/EPUB 等个人或公版内容的阅读与离线缓存。
- 可扩展书源：设计可测试的书源协议，逐步兼容常见 Legado 3.0 JSON 字段。
- Windows 体验：提供可配置排版、书架、阅读进度、搜索和更新能力。
- 合法使用：不内置未经授权的版权内容，不提供绕过付费、登录限制或验证码的功能。

## 技术路线

Tauri v2、Vue 3 + TypeScript + Vite、Rust、SQLite、Windows WebView2。

详细范围、兼容性和路线图：

- [产品范围](docs/product-scope.md)
- [技术路线 ADR](docs/adr/0001-tech-stack.md)
- [书源兼容性矩阵](docs/compatibility-matrix.md)
- [书源协议](docs/source-protocol.md)
- [开发路线图](docs/roadmap.md)
- [开发环境](docs/development.md)
- [Windows 发布验收清单](docs/release-checklist.md)
- [维护审计与 issue 决策](docs/maintenance-audit-2026-08-12.md)
- [2026-08-20 开发状态审计与执行记录](docs/development-status-2026-08-20.md)

## 当前状态

- [x] 创建公开仓库并明确产品边界
- [x] 初始化 Tauri + Vue + Rust 工程
- [x] 完成本地 TXT/EPUB 阅读 MVP 与阅读设置
- [x] 定义并测试基础书源协议与本机合成夹具
- [x] 完成书源保存、启用/停用、导入/导出与端到端调试面板
- [x] 完成多源搜索、去重、失败隔离、远程详情/目录/章节阅读与 TTL 缓存
- [x] 完成章节指纹、增量差异、stale 回退、权限记录和敏感请求头审计
- [x] 完成 Legado 书源元数据保真、分组/排序/批量管理、冲突策略和导入快照恢复
- [x] 完成受限分页策略、取消链、失败历史、请求/规则指标和 XPath 静态识别
- [x] 完成 TXT/EPUB 安全边界、大文件流式解析、图片序列预览、缩略图缓存和位置恢复
- [x] 完成图片序列 SQLite 书架模型、重启恢复、文件变更检测、重新关联和 stale 复核
- [x] 完成封面缓存基础和重复书只读合并预览二次校验（M9.3.1-a/b/c/d1）
- [x] 完成统一的阅读工作区视觉刷新：导航、书架、书源、阅读器和设置面板
- [ ] PR10 搜索工作区与书架重构：CI 31590842650 已通过，等待审阅/合并后再做 Release/smoke
- [x] GitHub Actions 已通过前端/Rust CI、Windows Release 和 installer smoke
- [ ] 目标 Windows 环境人工验收：升级、WebView2 缺失、离线/网络错误、中文字体、窄窗口、键盘焦点、高对比度和书源导入体验
- [ ] M9.3.1-d2 可撤销重复书合并：等待 P0 手工记录和迁移前评审收口
- [ ] M10 版本化备份/恢复，之后再评估 WebDAV、RSS/OPDS 与 TTS/音频能力

## 本地开发

```powershell
npm install
npm run tauri dev
```

M2 支持导入 TXT/EPUB、章节目录、阅读进度和字体/行距/主题设置。M3 增加书源 JSON 校验、HTML/JSON 提取器与受限 HTTP 预览；M4/M4.1 将搜索、详情、目录和首章正文串成可测试链路，并加入书源持久化、启停和调试诊断；M5–M7.5 已支持多源搜索、缓存、正文替换、分页策略、取消、失败历史、规则指标和安全兼容边界。书源配置支持本地 JSON 和 HTTP(S) URL 导入：bundle/在线响应体上限为 16 MiB，在线拉取超时 30 秒；URL 本身仍限制为 2 KiB，结构校验、规则安全闸门、脚本/Cookie/Authorization 拒绝和内存防护不变。M8–M9 已完成 TXT/EPUB 大文件、图片序列、书架恢复和文件变更检测切片。当前视觉刷新统一了导航、书架、书源、阅读器和设置面板。GitHub Actions 会在 `v*` 标签上生成未签名安装器、便携 ZIP 和 SHA-256 清单；当前自动化发布与 smoke 已通过，目标 Windows 手工回归仍是正式推广前置条件。XPath、JavaScript、认证态和音频书源按兼容性矩阵明确标记或拒绝。详细检查命令见 [docs/development.md](docs/development.md)，流程说明见 [docs/source-pipeline.md](docs/source-pipeline.md)，当前维护决策见 [docs/maintenance-audit-2026-08-12.md](docs/maintenance-audit-2026-08-12.md)。

## 参与开发

请先阅读 [CONTRIBUTING.md](CONTRIBUTING.md)。功能建议和缺陷反馈请使用 GitHub Issues；涉及安全问题请按 [SECURITY.md](SECURITY.md) 私下报告。

## License

仓库根目录已提供 MIT License。M0 issue #1 的文档一致性治理仍在维护审计中：README、CONTRIBUTING、SECURITY 和兼容性矩阵需要保持同一口径；在该项同步完成前，项目不会把许可证治理标记为完全收口。
