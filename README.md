# Open Reader Desktop

Windows-first open-source desktop reader inspired by the extensible reading experience of Legado/阅读。

> 状态（2026-08-20）：main 基线为 89185e6；PR10、PR11、PR12、PR13 已按顺序合并。合并后 CI run 32368235610、Windows Release run 32368262290、installer smoke run 32369094467 全部成功；Artifact `open-reader-windows-main-89185e640cefb2665510fc8b4622d918a9f1ab16` 的摘要为 `sha256:9372402dc2fb734a16fd75cd763c7971b197e54f717b754042557a894ddea7da`；目标 Windows 手工验收仍待执行，签名暂缓。

> 当前候选（2026-08-21）：PR18 分支 `0.2.0` 的 Windows Release [32402250634](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/32402250634) 与 installer smoke [32403363944](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/32403363944) 已通过；Computer Use 手工验收记录见 [windows-manual-acceptance-2026-08-21.md](docs/windows-manual-acceptance-2026-08-21.md)。M9.3.1-d2 已实现并由 CI [32463202309](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/32463202309) 全部验证通过；M7 P1 首轮的 item 自身节点匹配与响应字符集识别已由 CI [32466122037](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/32466122037) 验证；候选安装包仍未签名。

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
- [x] 完成 M9.3.1-d2：0016 生命周期、纯文本无冲突单事务合并、来源快照、单跳 alias、失败回滚和 active 查询过滤
- [x] 完成统一的阅读工作区视觉刷新：导航、书架、书源、阅读器和设置面板
- [x] PR10 搜索工作区与书架重构：已合并至 main；搜索工作区、书架分区和在线条目点击修复已进入发布基线
- [x] M7 P1 书源兼容性首轮：item 自身节点匹配、BOM/charset/HTML 元信息响应解码、GB18030/UTF-16 合成夹具和编码诊断已完成；授权响应夹具、乱码标题降级与 `toc/content` 差异诊断仍在推进
- [x] PR11 文档状态审计、PR12 书源快照保留策略、PR13 EPUB 资源诊断已按顺序合并至 main；合并后 CI（`32368235610`）、Windows Release（`32368262290`）和 installer smoke（`32369094467`）均通过
- [ ] 目标 Windows 环境人工验收：升级、WebView2 缺失、离线/网络错误、中文字体、窄窗口、键盘焦点、高对比度和书源导入体验
- [ ] M9.3.1-d3 撤销与旧 ID 兼容：7 天撤销、外部修改冲突和 alias 环检测
- [ ] M10 版本化备份/恢复，之后再评估 WebDAV、RSS/OPDS 与 TTS/音频能力

## 本地开发

```powershell
npm install
npm run tauri dev
```

M2 支持导入 TXT/EPUB、章节目录、阅读进度和字体/行距/主题设置。M3 增加书源 JSON 校验、HTML/JSON 提取器与受限 HTTP 预览；M4/M4.1 将搜索、详情、目录和首章正文串成可测试链路，并加入书源持久化、启停和调试诊断；M5–M7.5 已支持多源搜索、缓存、正文替换、分页策略、取消、失败历史、规则指标和安全兼容边界。书源配置支持本地 JSON 和 HTTP(S) URL 导入：bundle/在线响应体上限为 128 MiB，在线拉取超时 30 秒；URL 本身仍限制为 2 KiB，结构校验、规则安全闸门、脚本/Cookie/Authorization 拒绝和内存防护不变。M8–M9 已完成 TXT/EPUB 大文件、图片序列、书架恢复和文件变更检测切片。当前视觉刷新统一了导航、书架、书源、阅读器和设置面板；PR10 已将搜索结果独立为左侧工作区，并把本地书籍与书源书籍分区展示；PR11–PR13 已补齐治理记录、书源快照保留和 EPUB 资源诊断。GitHub Actions 会在 `v*` 标签或手动 Release 上生成未签名安装器、便携 ZIP 和 SHA-256 清单；`89185e6` 的 Release（`32368262290`）与 installer smoke（`32369094467`）已通过，目标 Windows 手工回归仍是正式推广前置条件。XPath、JavaScript、认证态和音频书源按兼容性矩阵明确标记或拒绝。详细检查命令见 [docs/development.md](docs/development.md)，流程说明见 [docs/source-pipeline.md](docs/source-pipeline.md)，当前维护决策见 [docs/maintenance-audit-2026-08-12.md](docs/maintenance-audit-2026-08-12.md)。

## 参与开发

请先阅读 [CONTRIBUTING.md](CONTRIBUTING.md)。功能建议和缺陷反馈请使用 GitHub Issues；涉及安全问题请按 [SECURITY.md](SECURITY.md) 私下报告。

## License

仓库根目录的原创代码采用 MIT License（见 [LICENSE](LICENSE)）。许可证只覆盖本项目原创代码与文档；第三方依赖、素材和未来引入的代码必须保留其原许可证与归属。书源与内容边界、Legado 兼容安全子集及权限模型见 [ADR 0002](docs/adr/0002-license-and-source-policy.md)；项目不内置未经授权的版权内容，也不绕过登录、付费、验证码、DRM 或访问控制。
