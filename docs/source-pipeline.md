# M4 单书源端到端流程

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
