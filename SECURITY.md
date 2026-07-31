# 安全政策

## 报告安全问题

请不要在公开 Issue 中发布可利用的漏洞、账号、Cookie、Token 或个人数据。请通过 GitHub 私下联系维护者，并提供复现步骤、影响范围和建议修复方向。

## 书源与脚本安全边界

- 默认拒绝任意本机文件、进程、剪贴板和系统命令访问。
- 网络请求必须受超时、大小、重定向和并发限制。
- 脚本执行必须有沙箱、资源配额和可取消机制。
- 日志中不得写入 Cookie、Authorization 和完整个人 URL 参数。
- 只为用户有权访问的内容提供解析和缓存能力。


## 当前实现的安全审计

- 书源 JSON 可包含 `permission.status`、`permission.scope` 和 `permission.reviewedAt`。`audit_sources` 只返回来源名称、主机名、权限记录、错误和提示，不返回完整查询 URL。
- `get_source_cache_status` 只返回缓存条目数、payload 字节数、过期条目数和容量上限，不返回缓存正文。
- Rust 受限客户端只允许 HTTP/HTTPS，默认超时 15 秒、响应体上限 2 MiB，最多跟随 5 次重定向；敏感请求头会被拒绝。
- WebView CSP 不允许任意 HTTPS `connect-src`；所有远程书源请求都通过 Rust 客户端发起。
- 当前 `bundle.active=false`，因为图标、签名资产和干净 Windows 安装回归尚未准备完成；未完成这些验收前不会发布安装包。
