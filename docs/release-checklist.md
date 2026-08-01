# Windows 发布验收清单

这份清单用于把 M5.5 的安全边界转化为可重复的 Windows 发布流程。当前签名暂缓，GitHub Actions 会发布明确标记为 unsigned 的安装器和便携 ZIP；安装、升级和卸载回归完成前仍不建议面向普通用户推广。

## 已可执行

- [ ] 在干净工作区执行 \`npm install --no-audit --no-fund\`
- [ ] 执行 \`npm run typecheck\`
- [ ] 执行 \`npm run build\`
- [ ] 执行 \`npm run test:rust\`
- [ ] 执行 \`cargo fmt --check --manifest-path src-tauri/Cargo.toml\`
- [ ] 执行 \`cargo check --offline --manifest-path src-tauri/Cargo.toml\`
- [ ] 在书源页运行“校验 JSON”和“安全审计”
- [ ] 在书源页运行“缓存状态”，确认只显示统计，不显示正文
- [ ] 用本机合成夹具验证详情/章节缓存、容量淘汰和 stale 回退
- [ ] 检查启动与缓存写入日志没有 Cookie、Authorization 或完整查询参数

## 仍需准备

- [x] 添加并核验 \`src-tauri/icons/icon.ico\`、\`icon.png\` 等 Windows 图标资产
- [x] 开启 Tauri \`bundle.active\`，并由严格预检保护打包入口
- [ ] 选择 Windows 签名证书和安全保存/轮换方案（暂缓）
- [ ] 在干净 Windows 环境验证首次安装、覆盖升级、卸载和数据保留策略
- [ ] 验证 WebView2 缺失、网络不可用和权限不足时的错误提示
- [ ] 记录版本号、构建产物 SHA-256 和发布说明
- [x] 建立 GitHub Actions 自动 GitHub Release 流程，并由维护者复核权限与隐私说明
- [ ] 验证 Release 回滚和撤回流程

## 通过标准

1. Rust 与前端检查全部通过，且没有新的编译或类型错误。
2. 安装、升级、卸载流程在干净 Windows 环境可重复执行。
3. 书源网络请求仍受 HTTP/HTTPS、15 秒超时、2 MiB 响应体和 5 次重定向限制。
4. WebView CSP 不允许任意 HTTPS \`connect-src\`；敏感请求头会在保存前被拒绝。
5. 签名暂缓时，Release 必须明确标记 unsigned；安装器、便携 ZIP 和 SHA-256 清单与 Release 记录一致。
6. 发布说明明确：用户只能读取自己有权访问的内容，项目不绕过登录、付费或验证码。

## 当前状态

M5.5 已完成安全审计、缓存统计和前端可观测性。M5.6 已加入真实图标、自动安装器/便携 ZIP 发布和 SHA-256 清单；签名暂缓，安装/升级/卸载回归待执行。

## M5.6 发布预检

在仓库根目录执行：

\`\`\`powershell
npm install --no-audit --no-fund
npm run verify:release
npm run verify:release:strict
\`\`\`

- \`verify:release\` 检查版本一致性、产品名、图标路径和打包状态；当前允许以“BLOCKED”结果退出 0，便于 CI 报告未完成的外部准备项。
- \`verify:release:strict\` 是真正的发布门禁，要求 \`bundle.active=true\`、所有图标存在且不是占位文件，并且 \`dist/index.html\` 已生成；任一条件不满足都会退出 1。
- CI 会运行非严格预检；版本标签工作流会严格检查图标和前端产物，签名暂缓不会阻止生成 unsigned 产物，但安装回归仍是发布前必做项。

## 自动化 Windows 安装回归

新增的 \`Windows installer smoke\` 工作流只使用 GitHub 的 \`windows-latest\` runner，不在本地构建或安装。它会在成功的 \`Windows release\` 工作流结束后自动运行，也可以手动传入 release workflow run ID 运行。

自动覆盖：

- release artifact 内的 NSIS、MSI、便携 ZIP 与 \`release-sha256.txt\` 完整性；
- 便携版解压、文件完整性和进程启动；
- NSIS 静默安装、启动、卸载和应用数据哨兵保留；
- MSI 静默安装、启动、卸载和应用数据哨兵保留。

仍需人工回归：

- 两个版本之间的覆盖升级；
- 缺少 WebView2 Runtime、离线、网络错误和权限不足提示；
- Release 回滚、撤回和签名/SmartScreen 行为。

要手动重跑：

1. 打开 Actions → Windows installer smoke → Run workflow。
2. 在 \`release_run_id\` 填写成功的 Windows release run ID（当前候选为 \`30661044824\`）。
3. 等待 smoke job 完成；失败时先查看失败步骤，再决定是否修复 workflow 或记录环境差异。

## 手动 Windows 发布候选工作流

GitHub Actions 的 \`Windows release\` 支持 \`workflow_dispatch\` 和推送 \`v*\` 版本标签：

1. 先确认 \`npm run verify:release:strict\` 在目标提交上通过。
2. 工作流会执行前端检查、Tauri 打包，并生成 NSIS/MSI 安装器、便携 ZIP 和 \`release-sha256.txt\`。
3. 推送与 package.json 版本一致的 \`v*\` 标签时，工作流自动创建标记为 unsigned 的 GitHub Release。
4. 签名暂缓时，发布说明必须保留 SmartScreen 警告和 WebView2 依赖提示。
5. 公开推广前仍需完成安装、覆盖升级、卸载、数据保留和 WebView2 回归。

