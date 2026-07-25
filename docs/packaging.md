# 打包与分发（PLAT-01…03）

开刮桌面端基于 Tauri 2。本机可打出对应平台的 beta 安装包；签名/公证需各自平台证书。

## 前置

- Rust stable、Node 20+、pnpm
- macOS：Xcode CLT；可选 Apple Developer 证书（签名/公证）
- Windows：WebView2（系统自带或 bootstrapper）；可选代码签名证书
- Linux：依赖见 [Tauri 文档](https://v2.tauri.app/start/prerequisites/)；目标 AppImage + deb

```bash
cd desktop
pnpm install
```

## 本地构建

```bash
# 当前平台安装包（macOS → .app + .dmg；Windows → NSIS；Linux → AppImage/deb）
pnpm build:app

# 仅编译前端 + 调试二进制（不打包）
pnpm tauri build --debug
```

产物默认在：

- `desktop/src-tauri/target/release/bundle/`

## macOS 签名与公证（PLAT-01）

未配置证书时，`pnpm build:app` 仍可出未签名包，仅适合本机/内测。

发布前设置环境变量（示例）：

```bash
export APPLE_SIGNING_IDENTITY="Developer ID Application: …"
export APPLE_CERTIFICATE=...          # base64 .p12（CI 常用）
export APPLE_CERTIFICATE_PASSWORD=...
export APPLE_ID=...
export APPLE_PASSWORD=...             # app-specific password
export APPLE_TEAM_ID=...
```

或在 `tauri.conf.json` → `bundle.macOS` 中配置 `signingIdentity` / `providerShortName`。  
公证流程遵循 Tauri v2 macOS 文档；CI 建议用 `apple-actions/import-codesign-certs`。

## Windows（PLAT-02）

- 默认 NSIS（当前用户安装）
- 可选：为 `bundle.windows.certificateThumbprint` 填入签名证书指纹

## Linux（PLAT-03）

- 默认同时打 **AppImage** 与 **deb**
- AppImage 便于便携分发；deb 适合发行版安装源

## CI

仓库提供 `.github/workflows/desktop-build.yml`：在 macOS / Windows / Ubuntu 上各打一次包（artifact 上传）。  
**不包含**签名与公证密钥；正式发版需在 secrets 中补全后再启用对应步骤。

## 版本号

与 `desktop/src-tauri/tauri.conf.json` 的 `version`、workspace `Cargo.toml` 保持一致后再打 tag。
