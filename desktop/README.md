# ScrapeX Desktop (Tauri 2)

跨平台桌面壳。Swift 原版仍保留在仓库根目录 `App/` / `Packages/`，作行为规格。

## 开发

```bash
# 在仓库根目录
cargo test -p media-core

cd desktop
pnpm install
pnpm tauri dev
```

## 布局

- `crates/media-core` — 模型 / SQLite / 文件系统
- `crates/scraper-kit` — 刮削（M0 stub）
- `crates/renamer` — 重命名（M0 stub）
- `desktop/` — React + Tailwind + Tauri
