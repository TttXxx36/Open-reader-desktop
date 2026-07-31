# 开发环境

## 必备环境

- Windows 10 1809 或更高版本。
- Node.js 20 LTS 与 npm。
- Rust stable toolchain、cargo 和 rustfmt。
- WebView2 Runtime（Windows 11 通常已内置）。
- Git。

## 安装与启动

在仓库根目录执行：

```powershell
npm install
npm run tauri dev
```

仅预览前端：

```powershell
npm run dev
```

## 检查命令

```powershell
npm run typecheck
npm run build
npm run test:rust
cargo fmt --check --manifest-path src-tauri/Cargo.toml
cargo check --manifest-path src-tauri/Cargo.toml
```

## M2 本地阅读

- 点击“导入 TXT / EPUB”选择本地文件，单文件限制为 64 MB。
- TXT 会尝试识别 UTF-8、UTF-16LE/BE 和 GB18030，并按“第 X 章/节/回/卷”等标题拆分。
- EPUB 首先读取标准 container.xml、OPF manifest/spine 和 XHTML 正文；复杂脚本、DRM 和特殊布局暂不保证。
- 书籍、章节正文和阅读进度写入本机 SQLite；阅读设置保存在应用本地存储。
- 当前不会联网上传导入文件，也不会内置版权书源。

## M3 书源协议

- 在“书源”页粘贴 JSON，点击“校验 JSON”。
- 校验包括 URL scheme、CSS 选择器、正则表达式、字段别名和缺失阶段提示。
- fetch_source_preview 只允许 HTTP/HTTPS，默认超时 15 秒、响应上限 2 MiB，且只返回前 2,000 个字符。
- 不执行 JavaScript，不自动携带 Cookie/Authorization，不绕过验证码或付费限制。
- 书源协议与示例见 [docs/source-protocol.md](source-protocol.md)。

## M4 单书源端到端

- `run_source_pipeline` 接收书源 JSON 和搜索关键词，按“搜索 → 详情 → 目录 → 第一章正文”执行。
- URL 模板支持 `{{keyword}}`、`{{bookId}}` 和 `{{chapterId}}` 等占位符；搜索结果中的相对链接会解析为绝对 URL。
- 端到端测试使用本机临时 TCP 服务和合成 HTML，不连接真实版权站点。测试范围与后续授权要求见 [docs/source-pipeline.md](source-pipeline.md)。
- Rust 协议层可使用 `cargo test --manifest-path src-tauri/Cargo.toml` 验证；完整 CI 还会执行前端类型检查、构建和 Rust 检查。

## M4.1 书源管理与调试

- 书源配置保存到 SQLite 的 `book_sources` 表，迁移文件为 `0003_sources.sql`。
- `list_sources`、`save_source`、`set_source_enabled` 和 `delete_source` 提供保存、启用/停用和删除能力。
- 书源调试面板调用 `run_source_pipeline`，显示搜索、详情、目录、正文四个阶段的状态、耗时、响应大小和脱敏 URL。
- 配置中的 `Authorization`、`Cookie` 和 `Proxy-Authorization` 请求头会被拒绝；不要把私人账号或令牌提交到仓库。
- 前端检查命令为 `npm run typecheck` 和 `npm run build`。

## M5 多源搜索

- 书架顶部的“搜索书源”会调用 search_sources，只查询 SQLite 中已启用的书源。
- 每个书源在独立异步任务中执行；请求、规则缺失或解析错误只会记录为该书源失败，不会阻断其他结果。
- 统一结果按“标题 + 作者”归一化去重（折叠空白并转小写）；无标题且无作者时退回使用书籍链接作为去重键。
- 当前前端支持点击有链接的结果进入详情、目录和正文阅读；无链接结果仍只展示元数据。测试仍只使用本机合成夹具，不连接真实版权站点。

## M5.1 远程详情与章节阅读

- 搜索结果带有书源 ID 和书籍链接时，可以打开远程详情；应用随后读取目录并加载第一章正文。
- Tauri 命令 fetch_source_book 负责详情/目录，fetch_source_chapter 负责章节正文；书源停用、配置损坏、缺少规则或网络错误都会返回明确错误，不会修改本地书架。
- 远程详情缓存 5 分钟，章节正文缓存 10 分钟；缓存键包含书源 ID、书源更新时间和请求 URL，避免书源配置更新后继续复用旧结果。
- 应用启动时调用缓存清理逻辑删除过期条目。当前缓存只服务远程阅读，不把远程内容导入本地书架，也不保存 Cookie/Authorization。
- 测试使用本机合成 HTTP 夹具；真实站点接入前仍需确认授权和服务条款。

## M5.2 配置迁移与章节刷新

- 书源页的“导出 JSON”会生成版本为 1 的书源包，包含书源 ID、启用状态和原始配置 JSON；“导入 JSON”只接受不超过 2 MB 且通过现有书源校验的配置。
- 导入会按书源 ID 更新已有配置，并恢复启用/停用状态；Authorization、Cookie 和 Proxy-Authorization 等敏感请求头仍会被拒绝。
- 远程阅读页的“刷新内容”会同时强制刷新详情、目录和当前章节，绕过 TTL 缓存并写回新的缓存条目；普通打开和章节切换仍优先使用缓存。
- 导入/导出只迁移书源配置，不迁移远程正文缓存；远程内容不会导入本地书架。

## M5.3 内容替换与缓存容量

- 书源可选配置 `replaceRules`，正文提取后按顺序执行 Rust regex 替换；`replacement` 支持 `$1` 等捕获组，`enabled` 默认开启。
- 校验阶段限制单书源最多 32 条规则、pattern 最多 512 字节、replacement 最多 4 KiB；无效正则会阻止保存，停用规则会给出警告。
- 远程详情与章节缓存仍分别使用 5 分钟和 10 分钟 TTL；应用启动清理过期条目，并在每次写入后保留最新的最多 256 条、总 payload 不超过 32 MiB。
- 容量淘汰按 `fetched_at` 倒序执行；超出条目数或字节预算的旧条目会删除，缓存只用于远程阅读，不会写入本地书架。

## M5.4 章节更新与失败回退

- 远程阅读页的“刷新内容”会重新获取目录并计算目录指纹，比较章节 URL 集合并计算包含标题/顺序的目录指纹，显示新增、移除和保留数量。
- 当前章节优先按 URL 保留；如果来源移除了当前章节，则选择刷新后最接近的索引，避免刷新后跳回第一章。
- 普通打开与章节切换继续使用详情/章节 TTL 缓存，不启动后台轮询；只有用户点击“刷新内容”才发起强制网络请求。
- 强制刷新失败时优先返回本机缓存并标记 `stale`，同时显示 `refresh_error`；没有可用缓存时才向前端返回错误。
- 目录指纹是变化检测信号，不是版权、授权或内容完整性证明；真实来源仍需单独确认授权和服务条款。

## M5.5 安全审计与发布前验收

- 书源可填写 `permission` 权限记录；“安全审计”会检查权限状态、主机范围、敏感请求头和结构错误。
- “缓存状态”只显示条目数、字节数、过期条目数与容量上限，不展示缓存正文；应用启动和缓存写入后的淘汰会在 Rust stderr 记录实际删除数量。
- Rust 网络客户端最多跟随 5 次重定向；WebView CSP 已关闭任意 HTTPS `connect-src`，远程来源请求统一经由 Rust。
- Windows 安装包当前仍是发布阻塞项：`bundle.active=false`，因为 `src-tauri/icons/icon.ico`、签名证书和干净安装/升级/卸载环境尚未准备。不要在这些验收完成前发布安装包。
- 当前可执行验收：`npm run typecheck`、`npm run build`、`npm run test:rust`、`cargo fmt --check --manifest-path src-tauri/Cargo.toml` 和 `cargo check --manifest-path src-tauri/Cargo.toml`。

- 完整的安装包、签名和回滚验收项见 [Windows 发布验收清单](release-checklist.md)。

## M5.6 发布候选预检

- `npm run verify:release` 检查 package/Tauri 版本一致性、产品名、图标路径和 `bundle.active`，当前会报告但不放行外部阻塞项。
- `npm run verify:release:strict` 额外要求真实图标、`bundle.active=true` 和已生成的 `dist/index.html`，用于发布前最终门禁。
- CI 已运行非严格预检；它不会替代签名证书、干净 Windows 安装/升级/卸载和 WebView2 环境回归。
- 现有 `src-tauri/icons/*.b64` 是占位编码文件，不是可直接发布的 Windows 图标；准备真实图标后再开启 Tauri bundle。

## 本地数据

开发运行时，SQLite 数据库位于系统应用数据目录下的 `open-reader.db`。数据库迁移脚本位于 `src-tauri/migrations`，后续每次结构变更都新增一个按序号命名的迁移文件。

## 常见问题

- WebView2 缺失：安装 Microsoft Edge WebView2 Runtime 后重启应用。
- Vite 端口被占用：释放 1420 端口，或调整 `vite.config.ts` 与 `tauri.conf.json` 中的端口配置。
- 浏览器预览模式下，SQLite 和 Tauri 命令不可用是正常现象；请使用 `npm run tauri dev` 验证桌面桥接。
- 如果导入失败，先确认扩展名为 `.txt` 或 `.epub`，并检查文件是否超过 64 MB。
