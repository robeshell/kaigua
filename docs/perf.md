# 性能验收（M6）

对应迁移计划 §5。下列命令用于本机/CI 做回归，不替代真实 NAS 手测。

## 空增量刷新（SCAN-13）

要求：目录无变更时只做目录 mtime walk + 写 scan state，不重新枚举/入库媒体文件。

```bash
# 含 early-exit 单测 + 中等规模目录压测
cargo test -p media-core --lib refresh -- --nocapture
cargo test -p media-core --lib empty_refresh_many_dirs -- --nocapture
```

`empty_refresh_many_dirs` 会建约 300 个电影目录，断言第二次 `refresh_library` 的 `early_exit == true` 且 `discovered_media_count == 0`。

## 刮削限流

- 设置项 `scrapeConcurrency`（1–8，默认 4）
- HTTP 层对 429 有退避（`scraper-kit`）

手测：批量刮削时观察日志与任务进度，确认不会瞬时打爆 API。

## 缩略图滚动

- 列表/海报必须走 `ThumbnailCache` 磁盘缩略图（`resolve_poster_thumbnail`）
- 清除缓存：设置 → 缓存

手测：在 NAS 库切换海报网格并快速滚动，主线程不应反复解码原图。

## 任务进度节流

扫描类进度事件由任务队列推送；前端 toast/任务面板消费。勿在热路径上每文件 emit。
