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
cargo fmt --check --manifest-path src-tauri/Cargo.toml
cargo check --manifest-path src-tauri/Cargo.toml
```

## 本地数据

开发运行时，SQLite 数据库位于系统应用数据目录下的 `open-reader.db`。数据库迁移脚本位于 `src-tauri/migrations`，后续每次结构变更都新增一个按序号命名的迁移文件。

## 常见问题

- WebView2 缺失：安装 Microsoft Edge WebView2 Runtime 后重启应用。
- Vite 端口被占用：释放 1420 端口，或调整 `vite.config.ts` 与 `tauri.conf.json` 中的端口配置。
- 浏览器预览模式下，SQLite 命令不可用是正常现象；请使用 `npm run tauri dev` 验证桌面桥接。
