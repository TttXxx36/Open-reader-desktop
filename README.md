# Open Reader Desktop

Windows-first open-source desktop reader inspired by the extensible reading experience of Legado/阅读。

> 项目处于持续开发阶段，M2–M6 代码切片已完成，M6.5 Windows 安装包回归等待 Actions 权限恢复；M7.0 书源元数据兼容已开始。

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

## 当前状态

- [x] 创建公开仓库并明确产品边界
- [x] 初始化 Tauri + Vue + Rust 工程
- [x] 完成本地 TXT/EPUB 阅读 MVP
- [x] 定义并测试基础书源协议
- [x] 完成本地授权夹具的单书源端到端流程
- [x] 完成书源保存、启用/停用与端到端调试面板
- [x] 完成多源搜索、去重与失败隔离
- [x] 完成远程详情、目录、章节阅读与 TTL 缓存
- [x] 完成书源配置导入/导出、远程章节强制刷新与正文替换规则
- [x] 完成远程缓存过期清理与容量上限（256 条或 32 MiB）
- [x] 完成章节目录指纹、增量差异和失败回退
- [x] 完成来源权限记录、敏感请求头审计与缓存状态查看
- [x] 完成发布预检脚本与 CI 状态报告
- [x] 完成手动/标签触发的 Windows 安装器、便携 ZIP 与 SHA-256 发布工作流
- [x] 完成 M6.0–M6.4 设置、导入兼容、内容块、阅读设置和首页组件化代码切片
- [x] 完成 M7.0 Legado 书源元数据保真与音频类型明确拒绝
- [ ] 完成 Windows 安装、升级、卸载与 WebView2 回归（签名暂缓）
- [ ] 完成 M7.1 书源分组、排序和元数据 SQLite 迁移

## 本地开发

```powershell
npm install
npm run tauri dev
```

M2 支持导入 TXT/EPUB、章节目录、阅读进度和字体/行距/主题设置。M3 增加书源 JSON 校验、HTML/JSON 提取器与受限 HTTP 预览；M4/M4.1 将搜索、详情、目录和首章正文串成可测试链路，并加入书源持久化、启停和调试诊断；M5 已支持启用书源并发搜索、标题/作者去重、单源失败隔离，以及搜索结果进入远程详情、目录和章节阅读。远程详情与章节正文使用 SQLite TTL 缓存，应用启动时会清理过期缓存；书源页支持经过校验的配置 JSON 导入/导出，远程阅读支持手动强制刷新，章节正文支持按顺序执行的 replaceRules 替换。缓存写入后会保留最新的最多 256 条、总 payload 不超过 32 MiB；手动刷新会计算目录指纹、显示新增/移除章节，并在网络失败时回退到本机缓存。普通阅读不启动后台轮询。当前已加入来源权限审计、敏感请求头拦截、缓存统计和 WebView CSP/重定向限制；Windows 图标已加入仓库；GitHub Actions 会在 `v*` 标签上自动生成未签名安装器、便携 ZIP 和 SHA-256 清单，`npm run verify:release:strict` 作为发布门禁，安装与升级回归仍待执行。M7.0 已开始保留 Legado 的来源 URL、分组、类型、权重、发现开关、自定义顺序、备注和书籍 URL 模式；XPath、JavaScript、认证态和音频书源仍按兼容性矩阵明确标记或拒绝。详细检查命令见 [docs/development.md](docs/development.md)，流程说明见 [docs/source-pipeline.md](docs/source-pipeline.md)。

## 参与开发

请先阅读 [CONTRIBUTING.md](CONTRIBUTING.md)。功能建议和缺陷反馈请使用 GitHub Issues；涉及安全问题请按 [SECURITY.md](SECURITY.md) 私下报告。

## License

项目许可证将在 M0 阶段通过公开 Issue 讨论后确定；在许可证确定前，请勿将仓库代码用于无法回溯的再分发。
