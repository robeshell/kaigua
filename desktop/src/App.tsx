import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { listen } from "@tauri-apps/api/event";

import { EmptyState } from "./components/EmptyState";
import { ManualMatchModal } from "./components/ManualMatchModal";
import { SettingsModal } from "./components/SettingsModal";
import { ToastHost } from "./components/ToastHost";
import { SORT_OPTIONS, STATUS_FILTERS } from "./lib/mediaList";
import { useAppStore, type MediaType } from "./store/appStore";

function App() {
  const { t, i18n } = useTranslation();
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [manualMatchOpen, setManualMatchOpen] = useState(false);
  const status = useAppStore((s) => s.status);
  const libraries = useAppStore((s) => s.libraries);
  const selectedLibraryId = useAppStore((s) => s.selectedLibraryId);
  const mediaItems = useAppStore((s) => s.mediaItems);
  const metadataById = useAppStore((s) => s.metadataById);
  const selectedMediaId = useAppStore((s) => s.selectedMediaId);
  const detail = useAppStore((s) => s.detail);
  const posterUrl = useAppStore((s) => s.posterUrl);
  const searchQuery = useAppStore((s) => s.searchQuery);
  const sortOption = useAppStore((s) => s.sortOption);
  const statusFilter = useAppStore((s) => s.statusFilter);
  const tasks = useAppStore((s) => s.tasks);
  const refreshStatus = useAppStore((s) => s.refreshStatus);
  const refreshLibraries = useAppStore((s) => s.refreshLibraries);
  const selectLibrary = useAppStore((s) => s.selectLibrary);
  const selectMedia = useAppStore((s) => s.selectMedia);
  const setSearchQuery = useAppStore((s) => s.setSearchQuery);
  const setSortOption = useAppStore((s) => s.setSortOption);
  const setStatusFilter = useAppStore((s) => s.setStatusFilter);
  const addLibrary = useAppStore((s) => s.addLibrary);
  const deleteSelectedLibrary = useAppStore((s) => s.deleteSelectedLibrary);
  const refreshSelectedLibrary = useAppStore((s) => s.refreshSelectedLibrary);
  const scrapeSelectedLibrary = useAppStore((s) => s.scrapeSelectedLibrary);
  const scrapeSelectedItem = useAppStore((s) => s.scrapeSelectedItem);
  const refreshTasks = useAppStore((s) => s.refreshTasks);
  const upsertTask = useAppStore((s) => s.upsertTask);
  const visibleMediaItems = useAppStore((s) => s.visibleMediaItems);

  useEffect(() => {
    void refreshStatus();
    void refreshLibraries();
    void refreshTasks();
    let unlistenTask: (() => void) | undefined;
    let unlistenLib: (() => void) | undefined;
    void listen("task-updated", (event) => {
      upsertTask(event.payload as Parameters<typeof upsertTask>[0]);
    }).then((fn) => {
      unlistenTask = fn;
    });
    void listen("library-updated", () => {
      void refreshLibraries();
      void refreshStatus();
    }).then((fn) => {
      unlistenLib = fn;
    });
    return () => {
      unlistenTask?.();
      unlistenLib?.();
    };
  }, [refreshStatus, refreshLibraries, refreshTasks, upsertTask]);

  const selected = libraries.find((l) => l.id === selectedLibraryId) ?? null;
  const grouped = groupLibraries(libraries);
  const visible = useMemo(
    () => visibleMediaItems(),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [mediaItems, searchQuery, sortOption, statusFilter, visibleMediaItems],
  );

  return (
    <div className="flex h-full flex-col bg-canvas text-fg">
      <header className="flex items-center justify-between border-b border-hairline bg-chrome py-3 pl-[78px] pr-6">
        <div className="min-w-0">
          <p className="text-[17px] font-extrabold tracking-[-0.35px] text-fg">
            {t("app.brand")}
          </p>
          <p className="text-[12.5px] font-semibold text-fg-secondary">{t("app.title")}</p>
        </div>
        <div className="flex flex-wrap items-center justify-end gap-2">
          {libraries.length > 0 ? <AddMenu onAdd={(type) => void addLibrary(type)} /> : null}
          <button type="button" onClick={() => setSettingsOpen(true)} className="kg-btn kg-btn-toolbar">
            {t("action.settings")}
          </button>
          <select
            value={i18n.language}
            onChange={(e) => void i18n.changeLanguage(e.target.value)}
            className="kg-select"
          >
            <option value="zh-Hans">{t("lang.zh")}</option>
            <option value="en">{t("lang.en")}</option>
          </select>
          <button
            type="button"
            disabled={!selected}
            onClick={() => void refreshSelectedLibrary()}
            className="kg-btn"
          >
            {t("action.refresh")}
          </button>
          <button
            type="button"
            disabled={!selected}
            onClick={() => void scrapeSelectedLibrary()}
            className="kg-btn kg-btn-outlined"
          >
            {t("action.scrapeAll")}
          </button>
          {selected ? (
            <button
              type="button"
              onClick={() => void deleteSelectedLibrary()}
              className="kg-btn kg-btn-destructive"
            >
              {t("action.deleteLibrary")}
            </button>
          ) : null}
        </div>
      </header>

      <div className="grid min-h-0 flex-1 grid-cols-[var(--kg-sidebar-width)_minmax(0,1fr)_var(--kg-detail-width)]">
        <aside className="overflow-auto border-r border-hairline bg-chrome p-3">
          {(["movie", "tvShow", "anime"] as MediaType[]).map((type) => (
            <div key={type} className="mb-5">
              <p className="kg-section-label mb-1.5 px-2">{typeLabel(type, t)}</p>
              {(grouped[type] ?? []).length === 0 ? (
                <p className="px-2 text-[11.5px] text-fg-muted">{t("sidebar.empty")}</p>
              ) : (
                <ul className="space-y-0.5">
                  {(grouped[type] ?? []).map((lib) => {
                    const active = lib.id === selectedLibraryId;
                    return (
                      <li key={lib.id}>
                        <button
                          type="button"
                          onClick={() => void selectLibrary(lib.id)}
                          className={`w-full rounded-control px-2.5 py-2 text-left ${
                            active
                              ? "bg-accent-10"
                              : "text-fg-secondary hover:bg-subtle"
                          }`}
                        >
                          <span
                            className={`block truncate text-[13.5px] ${
                              active ? "font-bold text-fg" : "font-medium"
                            }`}
                          >
                            {lib.name}
                          </span>
                          <span className="mt-0.5 block truncate font-mono text-[10.5px] text-fg-muted">
                            {lib.rootPath}
                          </span>
                        </button>
                      </li>
                    );
                  })}
                </ul>
              )}
            </div>
          ))}
        </aside>

        <main className="flex min-h-0 flex-col overflow-hidden bg-canvas">
          {!selected ? (
            <EmptyState
              className="h-full"
              title={t("empty.welcomeTitle")}
              message={
                libraries.length === 0 ? t("empty.welcomeMessage") : t("list.pickLibrary")
              }
              action={
                libraries.length === 0 ? (
                  <div className="flex flex-wrap justify-center gap-2">
                    <AddMenu onAdd={(type) => void addLibrary(type)} />
                  </div>
                ) : undefined
              }
            />
          ) : (
            <>
              <div className="flex flex-wrap items-center gap-2 border-b border-hairline px-4 py-2.5">
                <input
                  value={searchQuery}
                  onChange={(e) => setSearchQuery(e.target.value)}
                  placeholder={t("list.searchPlaceholder")}
                  className="kg-field min-w-[10rem] flex-1 !py-2"
                />
                <select
                  value={statusFilter}
                  onChange={(e) =>
                    setStatusFilter(e.target.value as (typeof STATUS_FILTERS)[number]["value"])
                  }
                  className="kg-select"
                >
                  {STATUS_FILTERS.map((opt) => (
                    <option key={opt.value} value={opt.value}>
                      {opt.label}
                    </option>
                  ))}
                </select>
                <select
                  value={sortOption}
                  onChange={(e) =>
                    setSortOption(e.target.value as (typeof SORT_OPTIONS)[number]["value"])
                  }
                  className="kg-select"
                >
                  {SORT_OPTIONS.map((opt) => (
                    <option key={opt.value} value={opt.value}>
                      {opt.label}
                    </option>
                  ))}
                </select>
                <span className="text-[11.5px] text-fg-muted">
                  {visible.length}/{mediaItems.length}
                </span>
              </div>

              <div className="min-h-0 flex-1 overflow-auto">
                {visible.length === 0 ? (
                  <EmptyState
                    className="h-full"
                    title={
                      mediaItems.length === 0 ? t("empty.scanTitle") : t("empty.filterTitle")
                    }
                    message={
                      mediaItems.length === 0 ? t("list.emptyScan") : t("list.emptyFilter")
                    }
                    action={
                      mediaItems.length === 0 ? (
                        <button
                          type="button"
                          onClick={() => void refreshSelectedLibrary()}
                          className="kg-btn"
                        >
                          {t("action.refresh")}
                        </button>
                      ) : undefined
                    }
                  />
                ) : (
                  <ul>
                    {visible.map((item) => {
                      const meta = metadataById[item.id];
                      const active = item.id === selectedMediaId;
                      return (
                        <li key={item.id} className="border-b border-hairline">
                          <button
                            type="button"
                            onClick={() => void selectMedia(item.id)}
                            className={`flex min-h-[54px] w-full items-center gap-2.5 px-3.5 py-1.5 text-left hover:bg-row-hover ${
                              active ? "bg-row-selected" : ""
                            }`}
                          >
                            <span className="w-10 shrink-0 text-center text-[10.5px] font-semibold uppercase text-fg-muted">
                              {item.status}
                            </span>
                            <span className="min-w-0 flex-1">
                              <span className="block truncate text-[13.5px] font-semibold text-fg">
                                {item.title}
                                {item.year ? (
                                  <span className="ml-2 font-medium text-fg-secondary">
                                    ({item.year})
                                  </span>
                                ) : null}
                              </span>
                              {meta?.overview ? (
                                <span className="mt-0.5 block truncate text-[11.5px] leading-[1.45] text-fg-secondary">
                                  {meta.overview}
                                </span>
                              ) : (
                                <span className="mt-0.5 block truncate font-mono text-[10.5px] text-fg-muted">
                                  {item.filePath || item.folderPath}
                                </span>
                              )}
                            </span>
                            {meta?.rating != null ? (
                              <span className="shrink-0 text-[12.5px] font-medium text-warning">
                                {meta.rating.toFixed(1)}
                              </span>
                            ) : null}
                          </button>
                        </li>
                      );
                    })}
                  </ul>
                )}
              </div>
            </>
          )}
        </main>

        <section className="flex min-h-0 flex-col overflow-hidden border-l border-hairline bg-surface">
          {detail ? (
            <div className="min-h-0 flex-1 overflow-auto p-4">
              <div className="mb-3 flex flex-wrap gap-2">
                <button
                  type="button"
                  className="kg-btn kg-btn-outlined"
                  onClick={() => void scrapeSelectedItem()}
                >
                  {t("action.scrapeItem")}
                </button>
                <button
                  type="button"
                  className="kg-btn kg-btn-toolbar"
                  onClick={() => setManualMatchOpen(true)}
                >
                  {t("action.manualMatch")}
                </button>
              </div>
              <DetailPanel detail={detail} posterUrl={posterUrl} />
            </div>
          ) : (
            <>
              <div className="border-b border-hairline px-4 py-5">
                <p className="text-[13.5px] font-semibold text-fg">{t("detail.title")}</p>
                <p className="mt-1 text-[11.5px] leading-[1.45] text-fg-secondary">
                  {t("detail.pickItem")}
                </p>
              </div>
              <div className="min-h-0 flex-1 overflow-auto p-4">
                <h2 className="mb-2 text-[13.5px] font-semibold text-fg">{t("tasks.title")}</h2>
                {status ? (
                  <p className="mb-3 text-[11.5px] text-fg-muted">
                    DB {status.libraryCount} · {status.version}
                  </p>
                ) : null}
                {tasks.length === 0 ? (
                  <p className="text-[11.5px] text-fg-secondary">{t("tasks.empty")}</p>
                ) : (
                  <ul className="space-y-2">
                    {tasks.map((task) => (
                      <li
                        key={task.id}
                        className="rounded-control border border-hairline bg-elevated px-2.5 py-2 text-[11.5px]"
                      >
                        <div className="flex justify-between gap-2">
                          <span className="font-semibold text-fg">{task.title}</span>
                          <span className="uppercase text-fg-muted">{task.status}</span>
                        </div>
                        {task.progress ? (
                          <p className="mt-1 text-fg-secondary">{task.progress.current}</p>
                        ) : null}
                        {task.errorMessage ? (
                          <p className="mt-1 text-error">{task.errorMessage}</p>
                        ) : null}
                      </li>
                    ))}
                  </ul>
                )}
              </div>
            </>
          )}
        </section>
      </div>
      {settingsOpen ? <SettingsModal onClose={() => setSettingsOpen(false)} /> : null}
      {manualMatchOpen && detail ? (
        <ManualMatchModal
          itemId={detail.item.id}
          mediaType={detail.item.mediaType}
          initialQuery={detail.item.title}
          onClose={() => setManualMatchOpen(false)}
        />
      ) : null}
      <ToastHost />
    </div>
  );
}

function DetailPanel({
  detail,
  posterUrl,
}: {
  detail: NonNullable<ReturnType<typeof useAppStore.getState>["detail"]>;
  posterUrl: string | null;
}) {
  const { t } = useTranslation();
  const { item, metadata, seasons, episodes } = detail;
  const runtime = formatRuntime(metadata?.runtime ?? null);

  return (
    <div className="space-y-4 text-[13.5px]">
      <div className="flex gap-3">
        <div className="h-[148px] w-[100px] shrink-0 overflow-hidden rounded-card border border-hairline bg-subtle">
          {posterUrl ? (
            <img src={posterUrl} alt="" className="h-full w-full object-cover" />
          ) : (
            <div className="flex h-full items-center justify-center text-[10.5px] text-fg-muted">
              {t("detail.noPoster")}
            </div>
          )}
        </div>
        <div className="min-w-0">
          <h2 className="text-[16px] font-extrabold leading-tight tracking-[-0.25px] text-fg">
            {item.title}
          </h2>
          {metadata?.tagline ? (
            <p className="mt-1 text-[11.5px] italic text-fg-secondary">{metadata.tagline}</p>
          ) : null}
          {item.originalTitle && item.originalTitle !== item.title ? (
            <p className="mt-1 text-[11.5px] text-fg-secondary">{item.originalTitle}</p>
          ) : null}
          <p className="mt-2 text-[11.5px] text-fg-secondary">
            {[item.year, runtime, metadata?.contentRating, item.status]
              .filter(Boolean)
              .join(" · ")}
          </p>
          {metadata?.genres?.length ? (
            <p className="mt-2 text-[11.5px] text-fg-muted">{metadata.genres.join(" / ")}</p>
          ) : null}
          {metadata?.rating != null ? (
            <p className="mt-1 text-[12.5px] font-medium text-warning">
              ★ {metadata.rating.toFixed(1)}
              {metadata.ratingVotes ? ` (${metadata.ratingVotes})` : null}
            </p>
          ) : null}
        </div>
      </div>

      {metadata?.overview ? (
        <section>
          <h3 className="kg-section-label">{t("detail.overview")}</h3>
          <p className="whitespace-pre-wrap text-[12px] leading-relaxed text-fg-secondary">
            {metadata.overview}
          </p>
        </section>
      ) : null}

      {(item.mediaType === "tvShow" || item.mediaType === "anime") && seasons.length > 0 ? (
        <section>
          <h3 className="kg-section-label">{t("detail.seasons")}</h3>
          <ul className="space-y-2">
            {seasons.map((season) => {
              const eps = episodes.filter((e) => e.seasonId === season.id);
              return (
                <li key={season.id} className="rounded-control border border-hairline p-2.5">
                  <p className="text-[12.5px] font-semibold text-fg">
                    {season.title ?? `Season ${season.seasonNumber}`}
                    <span className="ml-2 font-medium text-fg-muted">{eps.length} eps</span>
                  </p>
                  <ul className="mt-1 max-h-40 space-y-0.5 overflow-auto">
                    {eps.slice(0, 20).map((ep) => (
                      <li key={ep.id} className="truncate text-[11.5px] text-fg-secondary">
                        E{String(ep.episodeNumber).padStart(2, "0")}{" "}
                        {ep.title || `Episode ${ep.episodeNumber}`}
                      </li>
                    ))}
                    {eps.length > 20 ? (
                      <li className="text-[11.5px] text-fg-muted">… +{eps.length - 20} more</li>
                    ) : null}
                  </ul>
                </li>
              );
            })}
          </ul>
        </section>
      ) : null}

      <section>
        <h3 className="kg-section-label">{t("detail.file")}</h3>
        <p className="break-all font-mono text-[11.5px] text-fg-secondary">{item.folderPath}</p>
        {item.filePath && item.filePath !== item.folderPath ? (
          <p className="mt-1 break-all font-mono text-[11.5px] text-fg-muted">{item.filePath}</p>
        ) : null}
        {metadata?.sourceId ? (
          <p className="mt-2 text-[11.5px] text-fg-muted">source: {metadata.sourceId}</p>
        ) : null}
        {metadata?.director ? (
          <p className="mt-1 text-[11.5px] text-fg-secondary">Director: {metadata.director}</p>
        ) : null}
      </section>
    </div>
  );
}

function formatRuntime(minutes: number | null | undefined) {
  if (minutes == null || minutes <= 0) return null;
  const h = Math.floor(minutes / 60);
  const m = minutes % 60;
  if (h > 0 && m > 0) return `${h}h ${m}m`;
  if (h > 0) return `${h}h`;
  return `${m}m`;
}

function AddMenu({ onAdd }: { onAdd: (t: MediaType) => void }) {
  const { t } = useTranslation();
  return (
    <div className="flex items-center gap-1.5">
      {(
        [
          ["movie", "action.addMovie"],
          ["tvShow", "action.addTv"],
          ["anime", "action.addAnime"],
        ] as const
      ).map(([type, key]) => (
        <button key={type} type="button" onClick={() => onAdd(type)} className="kg-btn kg-btn-outlined">
          {t(key)}
        </button>
      ))}
    </div>
  );
}

function groupLibraries(libraries: ReturnType<typeof useAppStore.getState>["libraries"]) {
  return libraries.reduce(
    (acc, lib) => {
      (acc[lib.mediaType] ??= []).push(lib);
      return acc;
    },
    {} as Record<MediaType, typeof libraries>,
  );
}

function typeLabel(type: MediaType, t: (key: string) => string) {
  switch (type) {
    case "movie":
      return t("sidebar.movies");
    case "tvShow":
      return t("sidebar.tv");
    case "anime":
      return t("sidebar.anime");
  }
}

export default App;
