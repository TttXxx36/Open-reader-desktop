# Open Reader Desktop

Windows-first open-source desktop reader inspired by the extensible reading experience of Legado/阅读。

> 项目仍处于早期规划阶段，当前仓库用于公开讨论产品边界、技术路线和实现任务。

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

## 当前状态

- [x] 创建公开仓库并明确产品边界
- [x] 初始化 Tauri + Vue + Rust 工程
- [x] 完成本地 TXT/EPUB 阅读 MVP
- [x] 定义并测试基础书源协议
- [x] 完成本地授权夹具的单书源端到端流程
- [x] 完成书源保存、启用/停用与端到端调试面板
- [x] 完成多源搜索、去重与失败隔离
- [x] 完成远程详情、目录、章节阅读与 TTL 缓存
- [ ] 完成替换规则、章节更新、配置导入/导出和发布流程

## 本地开发

```powershell
npm install
npm run tauri dev
```

M2 支持导入 TXT/EPUB、章节目录、阅读进度和字体/行距/主题设置。M3 增加书源 JSON 校验、HTML/JSON 提取器与受限 HTTP 预览；M4/M4.1 将搜索、详情、目录和首章正文串成可测试链路，并加入书源持久化、启停和调试诊断；M5 已支持启用书源并发搜索、标题/作者去重、单源失败隔离，以及搜索结果进入远程详情、目录和章节阅读。远程详情与章节正文使用 SQLite TTL 缓存，应用启动时会清理过期缓存。替换规则、章节更新和配置导入/导出仍在后续迭代。详细检查命令见 [docs/development.md](docs/development.md)，流程说明见 [docs/source-pipeline.md](docs/source-pipeline.md)。

## 参与开发

请先阅读 [CONTRIBUTING.md](CONTRIBUTING.md)。功能建议和缺陷反馈请使用 GitHub Issues；涉及安全问题请按 [SECURITY.md](SECURITY.md) 私下报告。

## License

项目许可证将在 M0 阶段通过公开 Issue 讨论后确定；在许可证确定前，请勿将仓库代码用于无法回溯的再分发。
