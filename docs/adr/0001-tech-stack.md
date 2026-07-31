# ADR 0001：桌面技术栈

- 状态：提案
- 日期：2026-07-31

## 决策

采用 Tauri v2 + Vue 3/TypeScript/Vite + Rust + SQLite，Windows 运行时依赖 WebView2。

## 原因

- Tauri 的桌面包体和系统集成成本适合 Windows-first 产品。
- Vue/TypeScript 便于快速迭代阅读界面、设置页和调试器。
- Rust 适合承载网络、解析、缓存、任务调度和权限边界。
- SQLite 能以单文件保存书架、进度、缓存和配置。
- WebView2 提供成熟的 Windows Web 渲染能力。

## 备选方案

- .NET WinUI 3/WPF：Windows 原生能力强，但前端复用和跨平台扩展较弱。
- Compose Multiplatform：UI 统一性好，但本项目仍需验证 Windows 和 Web 内容解析生态。
- Electron：开发门槛低，但包体、内存和本地权限面更大。

## 代价与约束

- 前后端边界需要稳定的命令/API 数据模型。
- 必须处理 WebView2 Runtime 缺失或版本差异。
- Rust 与前端都要建立错误码、日志和取消机制。
- 书源脚本不能直接获得任意本机权限。
