import { create } from "zustand";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";

import {
  filterAndSortMedia,
  type MediaSortOption,
  type MediaStatusFilter,
} from "../lib/mediaList";

export type AppConfig = {
  scrapeConcurrency: number;
  metadataLanguage: string;
  nfoFormat: string;
  scanExcludedFolders: string[];
  renameAutoAfterScrape: boolean;
  renameCreateSeasonFolders: boolean;
  appearance: string;
  apiKeys: {
    tmdb: string;
    tvdb: string;
    omdb: string;
  };
};

export type AppStatus = {
  appName: string;
  version: string;
  dataDir: string;
  databasePath: string;
  libraryCount: number;
  config: AppConfig;
  crates: {
    mediaCore: string;
    scraperKit: string;
    renamer: string;
  };
};

export type MediaType = "movie" | "tvShow" | "anime";

export type Library = {
  id: string;
  name: string;
  rootPath: string;
  mediaType: MediaType;
  addedAt: string;
};

export type MediaItem = {
  id: string;
  mediaType: MediaType;
  title: string;
  originalTitle?: string | null;
  year?: number | null;
  folderPath: string;
  filePath: string;
  status: string;
  libraryId: string;
  addedAt: string;
};

export type MediaMetaSummary = {
  mediaItemId: string;
  posterPath?: string | null;
  fanartPath?: string | null;
  overview?: string | null;
  rating?: number | null;
  genres: string[];
};

export type MediaMetadata = {
  mediaItemId: string;
  overview?: string | null;
  tagline?: string | null;
  genres: string[];
  rating?: number | null;
  ratingVotes?: number | null;
  contentRating?: string | null;
  director?: string | null;
  runtime?: number | null;
  showStatus?: string | null;
  posterPath?: string | null;
  fanartPath?: string | null;
  sourceId: string;
};

export type TvSeason = {
  id: string;
  mediaItemId: string;
  seasonNumber: number;
  title?: string | null;
  overview?: string | null;
  posterPath?: string | null;
  airDate?: string | null;
  episodeCount?: number | null;
};

export type TvEpisode = {
  id: string;
  seasonId: string;
  episodeNumber: number;
  title?: string | null;
  overview?: string | null;
  airDate?: string | null;
  stillPath?: string | null;
  filePath: string;
  runtime?: number | null;
  rating?: number | null;
  director?: string | null;
};

export type MediaDetail = {
  item: MediaItem;
  metadata?: MediaMetadata | null;
  seasons: TvSeason[];
  episodes: TvEpisode[];
};

export type TaskSnapshot = {
  id: string;
  title: string;
  kind: string;
  status: string;
  progress?: {
    completed: number;
    total: number;
    current: string;
    stageKey?: string;
  } | null;
  errorMessage?: string | null;
  createdAt: string;
  updatedAt: string;
};

type AppStore = {
  status: AppStatus | null;
  libraries: Library[];
  selectedLibraryId: string | null;
  mediaItems: MediaItem[];
  metadataById: Record<string, MediaMetaSummary>;
  selectedMediaId: string | null;
  detail: MediaDetail | null;
  posterUrl: string | null;
  searchQuery: string;
  sortOption: MediaSortOption;
  statusFilter: MediaStatusFilter;
  tasks: TaskSnapshot[];
  loading: boolean;
  error: string | null;
  toastMessage: string | null;
  showToast: (message: string) => void;
  refreshStatus: () => Promise<void>;
  refreshLibraries: () => Promise<void>;
  selectLibrary: (id: string | null) => Promise<void>;
  selectMedia: (id: string | null) => Promise<void>;
  setSearchQuery: (q: string) => void;
  setSortOption: (o: MediaSortOption) => void;
  setStatusFilter: (f: MediaStatusFilter) => void;
  addLibrary: (mediaType: MediaType) => Promise<void>;
  deleteSelectedLibrary: () => Promise<void>;
  refreshSelectedLibrary: () => Promise<void>;
  refreshTasks: () => Promise<void>;
  upsertTask: (task: TaskSnapshot) => void;
  visibleMediaItems: () => MediaItem[];
};

let toastTimer: ReturnType<typeof setTimeout> | null = null;

export const useAppStore = create<AppStore>((set, get) => ({
  status: null,
  libraries: [],
  selectedLibraryId: null,
  mediaItems: [],
  metadataById: {},
  selectedMediaId: null,
  detail: null,
  posterUrl: null,
  searchQuery: "",
  sortOption: "nameAscending",
  statusFilter: "all",
  tasks: [],
  loading: false,
  error: null,
  toastMessage: null,

  showToast: (message) => {
    if (toastTimer) clearTimeout(toastTimer);
    set({ toastMessage: message });
    toastTimer = setTimeout(() => {
      set({ toastMessage: null });
      toastTimer = null;
    }, 1400);
  },

  visibleMediaItems: () => {
    const { mediaItems, searchQuery, statusFilter, sortOption } = get();
    return filterAndSortMedia(mediaItems, searchQuery, statusFilter, sortOption);
  },

  refreshStatus: async () => {
    set({ loading: true, error: null });
    try {
      const status = await invoke<AppStatus>("app_status");
      set({ status, loading: false });
    } catch (err) {
      const message = String(err);
      set({ loading: false, error: message });
      get().showToast(message);
    }
  },

  refreshLibraries: async () => {
    try {
      const libraries = await invoke<Library[]>("list_libraries");
      const selected = get().selectedLibraryId;
      const nextSelected =
        selected && libraries.some((l) => l.id === selected)
          ? selected
          : libraries[0]?.id ?? null;
      set({ libraries, selectedLibraryId: nextSelected });
      if (nextSelected) {
        await get().selectLibrary(nextSelected);
      } else {
        set({
          mediaItems: [],
          metadataById: {},
          selectedMediaId: null,
          detail: null,
          posterUrl: null,
        });
      }
    } catch (err) {
      const message = String(err);
      set({ error: message });
      get().showToast(message);
    }
  },

  selectLibrary: async (id) => {
    set({
      selectedLibraryId: id,
      selectedMediaId: null,
      detail: null,
      posterUrl: null,
    });
    if (!id) {
      set({ mediaItems: [], metadataById: {} });
      return;
    }
    try {
      const page = await invoke<{
        items: MediaItem[];
        metadata: MediaMetaSummary[];
      }>("list_media_page", { libraryId: id });
      const metadataById: Record<string, MediaMetaSummary> = {};
      for (const meta of page.metadata) {
        metadataById[meta.mediaItemId] = meta;
      }
      set({ mediaItems: page.items, metadataById });
    } catch (err) {
      const message = String(err);
      set({ error: message });
      get().showToast(message);
    }
  },

  selectMedia: async (id) => {
    set({ selectedMediaId: id, detail: null, posterUrl: null });
    if (!id) return;
    try {
      const detail = await invoke<MediaDetail>("get_media_detail", { id });
      set({ detail });
      const posterPath = detail.metadata?.posterPath;
      if (posterPath) {
        const cachePath = await invoke<string | null>("resolve_poster_thumbnail", {
          folderPath: detail.item.folderPath,
          posterPath,
          width: 100,
          height: 148,
        });
        set({ posterUrl: cachePath ? convertFileSrc(cachePath) : null });
      }
    } catch (err) {
      const message = String(err);
      set({ error: message });
      get().showToast(message);
    }
  },

  setSearchQuery: (q) => set({ searchQuery: q }),
  setSortOption: (o) => set({ sortOption: o }),
  setStatusFilter: (f) => set({ statusFilter: f }),

  addLibrary: async (mediaType) => {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: "选择资料库根目录",
      });
      if (!selected || Array.isArray(selected)) {
        return;
      }
      const name = selected.split(/[/\\]/).filter(Boolean).pop() ?? "Library";
      await invoke<Library>("add_library", {
        name,
        rootPath: selected,
        mediaType,
      });
      await get().refreshLibraries();
      await get().refreshStatus();
      await get().refreshTasks();
      get().showToast(`已添加「${name}」`);
    } catch (err) {
      const message = String(err);
      set({ error: message });
      get().showToast(message);
    }
  },

  deleteSelectedLibrary: async () => {
    const id = get().selectedLibraryId;
    if (!id) return;
    const lib = get().libraries.find((l) => l.id === id);
    if (!lib) return;
    if (
      !window.confirm(
        `删除资料库「${lib.name}」？仅删除数据库记录，不删磁盘文件。`,
      )
    ) {
      return;
    }
    try {
      await invoke("delete_library", { id });
      set({
        selectedLibraryId: null,
        mediaItems: [],
        selectedMediaId: null,
        detail: null,
      });
      await get().refreshLibraries();
      await get().refreshStatus();
      get().showToast(`已删除「${lib.name}」`);
    } catch (err) {
      const message = String(err);
      set({ error: message });
      get().showToast(message);
    }
  },

  refreshSelectedLibrary: async () => {
    const id = get().selectedLibraryId;
    if (!id) return;
    try {
      const task = await invoke<TaskSnapshot>("refresh_library", {
        libraryId: id,
      });
      get().upsertTask(task);
      get().showToast("开始刷新…");
    } catch (err) {
      const message = String(err);
      set({ error: message });
      get().showToast(message);
    }
  },

  refreshTasks: async () => {
    try {
      const tasks = await invoke<TaskSnapshot[]>("list_tasks");
      set({ tasks });
    } catch (err) {
      const message = String(err);
      set({ error: message });
      get().showToast(message);
    }
  },

  upsertTask: (task) => {
    const prev = get().tasks.find((t) => t.id === task.id);
    set((state) => {
      const rest = state.tasks.filter((t) => t.id !== task.id);
      return { tasks: [task, ...rest] };
    });
    if (prev?.status === task.status) return;
    if (task.status === "completed" && task.kind === "refresh") {
      get().showToast("刷新完成");
    } else if (task.status === "failed") {
      get().showToast(task.errorMessage || "任务失败");
    }
  },
}));
