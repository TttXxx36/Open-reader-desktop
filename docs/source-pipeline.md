# M4 单书源端到端流程

> 维护状态（2026-08-12）：M4/M5 的合成夹具、书源导入和诊断链路仍由 main CI 验证；当前 UI 刷新不改变请求边界。书源配置导入已统一为 16 MiB bundle 上限，在线拉取超时 30 秒。

M4 把 M3 的书源协议串成一条可测试的最小链路：

搜索 → 书籍详情 → 目录 → 第一章正文

## 后端命令

Tauri 命令 `run_source_pipeline` 接收两个参数：

- `config_json`：符合 [书源协议](source-protocol.md) 的 JSON 字符串。
- `keyword`：搜索关键词。

命令返回 `SourcePipelineResult`，包含搜索结果、首本书详情、目录、第一章正文和 `debug_steps`。每个调试步骤包含阶段名、HTTP 状态、耗时、响应大小和去除查询参数后的 URL。URL 模板支持：

- `{{keyword}}` / `{{key}}`
- `{{bookUrl}}` / `{{book_url}}`
- `{{bookId}}` / `{{book_id}}`
- `{{chapterId}}` / `{{chapter_id}}`

相对链接会以当前响应 URL 解析为绝对 URL；请求仍受 M3 的 HTTP/HTTPS、超时和响应体大小限制约束。配置中的 Authorization、Cookie 和 Proxy-Authorization 请求头会被拒绝。

## 测试边界

`src-tauri/src/source.rs` 中的 `runs_authorized_fixture_pipeline` 使用本机临时 TCP 服务和合成 HTML，只验证协议、解析器和请求编排，不连接真实版权站点，也不携带 Cookie、Authorization 或绕过验证码。

后续接入真实站点前，必须确认站点授权、robots/服务条款和测试账号范围，并为每个来源保留可重复的夹具或录制响应。

## M5.1 远程阅读命令

多源搜索结果进入阅读前，会使用已启用书源重新获取详情和目录：

- fetch_source_book：输入 source_id 和搜索结果中的 book_url，返回书籍详情、目录和阶段诊断。
- fetch_source_chapter：输入 source_id 和目录项，返回章节标题与正文。
- 两个命令都会复用受限 HTTP 客户端和书源规则；详情缓存 5 分钟、章节缓存 10 分钟，缓存按书源更新时间自动失效。
- 缓存数据只保存在本机 SQLite，不会把远程书籍写入本地书架；应用启动时会清理过期缓存。

远程章节阅读仍遵守 M3/M4 的限制：只允许 HTTP/HTTPS，不执行 JavaScript，不携带 Cookie/Authorization，不绕过验证码或付费限制。

## M5.2 配置包与强制刷新

书源页提供版本化 JSON 配置包导入/导出。导入前会逐项执行书源协议校验；本地 JSON、对象/数组 bundle 和在线 URL 响应体统一限制为 16 MiB，在线拉取超时为 30 秒，超限或超时不会写入配置；导出内容不包含远程正文缓存。

远程阅读页的“刷新内容”会为当前书籍详情、目录和章节请求设置 force_refresh，绕过未过期的缓存并重新写入 TTL 缓存。普通读取仍按详情 5 分钟、章节 10 分钟的 TTL 工作。

## M5.3 正文替换与缓存容量

书源可通过 `replaceRules` 配置正文清洗规则。规则在正文提取后按顺序执行，使用 Rust regex 语法；`replacement` 支持 `$1` 等捕获组，`enabled` 默认为 true。校验会限制规则数量与单条规则的 pattern/replacement 大小，避免配置失控；替换不会影响搜索、详情和目录字段。

远程缓存继续使用详情 5 分钟、章节 10 分钟的 TTL。应用启动会清理过期条目，详情或章节写入后还会按 `fetched_at` 从新到旧淘汰，最多保留 256 条且 payload 总量不超过 32 MiB。淘汰失败只记录日志，不阻断本次远程内容返回。

## M5.4 章节更新与失败回退

远程阅读的手动“刷新内容”会重新抓取目录并计算目录指纹，同时比较刷新前后的章节 URL、标题和顺序：

- 返回新增、移除和保留章节数量；当前章节优先按 URL 保留，若章节已移除则回退到相近索引。
- 普通打开和章节切换仍只使用 TTL 缓存，不启动后台轮询；网络刷新只由用户明确点击触发。
- 详情或章节刷新失败时，如果本机仍有缓存，会返回带 stale 标记的旧内容并显示错误；没有可用缓存时才返回失败。
- 目录指纹只用于检测变化，不代表内容授权或完整性证明；真实来源仍需遵守授权、robots 和服务条款。


## M5.5 安全审计与缓存可观测性

书源配置可以声明 `permission` 元数据（`status`、`scope`、`reviewedAt`）。桌面端的 `audit_sources` 命令会逐个检查已保存书源，返回权限记录、解析出的主机名、敏感请求头、错误和提示；它不会返回完整请求 URL，也不会把权限记录当作授权证明。

书源页的“缓存状态”按钮调用 `get_source_cache_status`，只显示条目数、payload 字节数、过期条目数与 256 条/32 MiB 容量上限。应用启动和每次远程内容写入后都会执行过期清理与容量淘汰，并在 Rust 日志中记录实际删除数量。

网络边界继续保持：普通书源请求只允许 HTTP/HTTPS、15 秒超时、2 MiB 响应体上限和最多 5 次重定向；配置导入的在线 URL 单独使用 30 秒、16 MiB 上限；WebView CSP 不开放任意 HTTPS `connect-src`。刷新失败时仍优先返回 stale 本机缓存，安全审计与缓存统计不会读取或展示缓存正文。


## 2026-08-12 维护复核

- 本机临时 TCP 合成夹具仍是 M4 的可重复验收边界；真实站点只在获得授权且服务条款允许时使用。
- 当前 CI run [31574147034](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31574147034)、Windows release run [31574767135](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31574767135) 和 installer smoke run [31575465554](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/31575465554) 已通过。
- XPath、JavaScript、Cookie、Authorization 和音频能力继续按兼容性矩阵明确标记或拒绝，不以导入成功代替执行成功。
