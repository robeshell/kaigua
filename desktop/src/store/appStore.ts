import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";

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
  tasks: TaskSnapshot[];
  loading: boolean;
  error: string | null;
  refreshStatus: () => Promise<void>;
  refreshTasks: () => Promise<void>;
  runSmokeTask: () => Promise<void>;
  upsertTask: (task: TaskSnapshot) => void;
};

export const useAppStore = create<AppStore>((set, get) => ({
  status: null,
  tasks: [],
  loading: false,
  error: null,

  refreshStatus: async () => {
    set({ loading: true, error: null });
    try {
      const status = await invoke<AppStatus>("app_status");
      set({ status, loading: false });
    } catch (err) {
      set({ loading: false, error: String(err) });
    }
  },

  refreshTasks: async () => {
    try {
      const tasks = await invoke<TaskSnapshot[]>("list_tasks");
      set({ tasks });
    } catch (err) {
      set({ error: String(err) });
    }
  },

  runSmokeTask: async () => {
    try {
      const task = await invoke<TaskSnapshot>("enqueue_smoke_task", {
        title: "M0 smoke task",
      });
      get().upsertTask(task);
    } catch (err) {
      set({ error: String(err) });
    }
  },

  upsertTask: (task) => {
    set((state) => {
      const rest = state.tasks.filter((t) => t.id !== task.id);
      return { tasks: [task, ...rest] };
    });
  },
}));
