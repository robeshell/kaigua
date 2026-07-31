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

## Typography

Kaigua's UI typography comes from the generated Kai Design roles in
`src/styles/brand.generated.css`. `src/main.tsx` activates the desktop
component profile on the document root, and `src/index.css` exposes stable
`.kg-type-*` aliases for React components to consume.

`src/styles/tokens.css` is reserved for Kaigua-specific compatibility and
layout variables; it must not duplicate generated brand variables. Application
CSS and TSX must not introduce numeric `font-size` declarations or Tailwind
`text-[Npx]` classes. `npm run tokens:check` enforces both the generated/product
variable boundary and this typography boundary.


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
