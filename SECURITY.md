# 安全政策

## 报告安全问题

请不要在公开 Issue 中发布可利用的漏洞、账号、Cookie、Token 或个人数据。请通过 GitHub 私下联系维护者，并提供复现步骤、影响范围和建议修复方向。

## 书源与脚本安全边界

- 默认拒绝任意本机文件、进程、剪贴板和系统命令访问。
- 网络请求必须受超时、大小、重定向和并发限制。
- 当前默认不执行 JavaScript、模板脚本或 XPath；若未来引入运行时，必须先通过独立沙箱、资源配额、可取消机制、API 白名单、审计和许可证评估。
- 日志中不得写入 Cookie、Authorization 和完整个人 URL 参数。
- 只为用户有权访问的内容提供解析和缓存能力。


## 内容与书源治理

- 项目不内置未经授权的版权书源或书籍；用户只能对自有、已授权、公开测试或公版内容进行解析和缓存。
- “导入成功”不代表“执行成功”：安全 CSS/JSONPath/正则子集可执行；XPath、JavaScript、认证态和未实现扩展可保留并提示但不执行；危险或越权能力明确拒绝。
- `permission` 字段只是用户提供的复核记录，不是项目对授权真实性的证明；未知、缺少范围或缺少复核日期时必须提示。
- 书源网络访问通过 Rust 受限客户端，限制协议、超时、响应体、重定向和并发；日志不得记录敏感请求头或完整个人 URL 参数。
- 详细决策见 [ADR 0002：许可证与书源兼容政策](docs/adr/0002-license-and-source-policy.md)。

## 当前实现的安全审计

- 书源 JSON 可包含 `permission.status`、`permission.scope` 和 `permission.reviewedAt`。`audit_sources` 只返回来源名称、主机名、权限记录、错误和提示，不返回完整查询 URL。
- `get_source_cache_status` 只返回缓存条目数、payload 字节数、过期条目数和容量上限，不返回缓存正文。
- Rust 受限客户端只允许 HTTP/HTTPS，默认超时 15 秒、响应体上限 2 MiB，最多跟随 5 次重定向；敏感请求头会被拒绝。
- WebView CSP 不允许任意 HTTPS `connect-src`；所有远程书源请求都通过 Rust 客户端发起。
- 当前 `bundle.active=true`，GitHub Actions 可在版本标签上发布未签名的 Windows 安装器和便携 ZIP；签名暂缓，Release 标题与说明会明确标注 unsigned，干净 Windows 安装回归仍需完成。
