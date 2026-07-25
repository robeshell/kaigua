# 开刮 / kaigua

跨平台媒体库刮削桌面端。扫描本机电影 / 剧集 / 动漫资料库，匹配 TMDB · Bangumi · TVDB · OMDb 元数据，整理季文件夹与文件名，并写入 NFO / 海报。

## 功能

- 资料库扫描与增量刷新
- 自动 / 手动刮削，支持单部与单季
- 重复剧集合并（同 TMDB / 同名同年）
- 季文件夹整理、模板改名、批量重命名
- 残余文件清理、缩略图缓存
- 中 / 英 / 日界面

## 技术栈

| 层 | 选型 |
|----|------|
| 桌面壳 | Tauri 2 |
| 前端 | React + Tailwind |
| 核心 | Rust（`media-core` / `scraper-kit` / `renamer`） |

## 开发

```bash
cargo test -p media-core
cargo test -p renamer

cd desktop
pnpm install
pnpm tauri dev
```

打包与性能说明见 [`docs/packaging.md`](docs/packaging.md)、[`docs/perf.md`](docs/perf.md)；功能对照见 [`docs/cross-platform-migration-plan.md`](docs/cross-platform-migration-plan.md)。

## 目录

- `crates/media-core` — 模型 / SQLite / 扫描 / 文件系统
- `crates/scraper-kit` — 刮削
- `crates/renamer` — 模板改名与批量规则
- `desktop/` — React + Tauri 壳
