# kaigua Desktop (Tauri 2)

跨平台桌面壳。行为规格见 `docs/cross-platform-migration-plan.md`。

## 开发

```bash
# 仓库根目录
cargo test -p media-core
cargo test -p renamer

cd desktop
pnpm install
pnpm tauri dev
```

## 打包（beta）

详见 [docs/packaging.md](../docs/packaging.md)。

```bash
cd desktop
pnpm build:app
```

产物：`src-tauri/target/release/bundle/`

## 性能回归

见 [docs/perf.md](../docs/perf.md)。

## 布局

- `crates/media-core` — 模型 / SQLite / 扫描 / 文件系统
- `crates/scraper-kit` — 刮削
- `crates/renamer` — 模板改名 + 批量规则重命名
- `desktop/` — React + Tailwind + Tauri
