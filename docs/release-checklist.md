# Windows 发布验收清单

这份清单用于把 M5.5 的安全边界转化为可重复的 Windows 发布流程。未完成“仍需准备”部分前，仓库只发布源码和测试构建，不发布公开安装包。

## 已可执行

- [ ] 在干净工作区执行 `npm install --no-audit --no-fund`
- [ ] 执行 `npm run typecheck`
- [ ] 执行 `npm run build`
- [ ] 执行 `npm run test:rust`
- [ ] 执行 `cargo fmt --check --manifest-path src-tauri/Cargo.toml`
- [ ] 执行 `cargo check --offline --manifest-path src-tauri/Cargo.toml`
- [ ] 在书源页运行“校验 JSON”和“安全审计”
- [ ] 在书源页运行“缓存状态”，确认只显示统计，不显示正文
- [ ] 用本机合成夹具验证详情/章节缓存、容量淘汰和 stale 回退
- [ ] 检查启动与缓存写入日志没有 Cookie、Authorization 或完整查询参数

## 仍需准备

- [ ] 添加并核验 `src-tauri/icons/icon.ico`、`icon.png` 等 Windows 图标资产
- [ ] 确认 Tauri `bundle.active` 的开启时机；图标缺失前保持 `false`
- [ ] 选择 Windows 签名证书和安全保存/轮换方案
- [ ] 在干净 Windows 环境验证首次安装、覆盖升级、卸载和数据保留策略
- [ ] 验证 WebView2 缺失、网络不可用和权限不足时的错误提示
- [ ] 记录版本号、构建产物 SHA-256 和发布说明
- [ ] 建立 GitHub Release / 回滚流程，并由维护者复核权限与隐私说明

## 通过标准

1. Rust 与前端检查全部通过，且没有新的编译或类型错误。
2. 安装、升级、卸载流程在干净 Windows 环境可重复执行。
3. 书源网络请求仍受 HTTP/HTTPS、15 秒超时、2 MiB 响应体和 5 次重定向限制。
4. WebView CSP 不允许任意 HTTPS `connect-src`；敏感请求头会在保存前被拒绝。
5. 发布包完成签名，校验值与 Release 记录一致。
6. 发布说明明确：用户只能读取自己有权访问的内容，项目不绕过登录、付费或验证码。

## 当前状态

M5.5 已完成安全审计、缓存统计和前端可观测性。安装包回归、图标、签名和发布候选版本归入 M5.6。
