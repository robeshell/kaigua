import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";

import { useAppStore, type Library, type MediaType } from "../store/appStore";

export function SettingsModal({ onClose }: { onClose: () => void }) {
  const { t } = useTranslation();

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-barrier p-5"
      onClick={onClose}
      onKeyDown={(e) => {
        if (e.key === "Escape") onClose();
      }}
      role="presentation"
    >
      <div
        className="flex max-h-[min(720px,calc(100vh-48px))] w-full max-w-[min(var(--kg-settings-max),94vw)] flex-col overflow-hidden rounded-dialog border border-glass-border bg-elevated shadow-[0_8px_28px_rgb(0_0_0_/_0.09)]"
        onClick={(e) => e.stopPropagation()}
        role="dialog"
        aria-modal="true"
        aria-labelledby="settings-title"
      >
        <header className="flex items-start justify-between gap-4 px-6 pb-4 pt-5">
          <div>
            <h2
              id="settings-title"
              className="truncate text-[26px] font-extrabold tracking-[-0.55px] text-fg"
            >
              {t("settings.title")}
            </h2>
            <p className="mt-1 text-[12.5px] text-fg-secondary">{t("settings.subtitle")}</p>
          </div>
          <button type="button" onClick={onClose} className="kg-btn kg-btn-toolbar shrink-0">
            {t("settings.close")}
          </button>
        </header>

        <div className="min-h-0 flex-1 overflow-auto px-6 pb-8">
          <section className="mb-7">
            <p className="kg-section-label">{t("settings.section.library")}</p>
            <LibrarySection />
          </section>

          <section className="mb-7">
            <p className="kg-section-label">{t("settings.section.api")}</p>
            <ApiKeysSection />
          </section>

          <section className="mb-7">
            <p className="kg-section-label">{t("settings.section.scan")}</p>
            <ScrapeExclusionsSection />
          </section>

          <section className="mb-7">
            <p className="kg-section-label">{t("settings.section.later")}</p>
            <div className="kg-settings-group">
              <LaterRow title="Rename Rules" note="M3" />
              <LaterRow title="Appearance" note="M6" />
            </div>
          </section>
        </div>
      </div>
    </div>
  );
}

function LaterRow({ title, note }: { title: string; note: string }) {
  return (
    <div className="flex min-h-[58px] items-center justify-between gap-3 px-3.5 py-2">
      <div className="min-w-0">
        <p className="truncate text-[13.5px] font-semibold text-fg">{title}</p>
        <p className="truncate text-[11.5px] text-fg-secondary">Ships in {note}</p>
      </div>
    </div>
  );
}

function ApiKeysSection() {
  const showToast = useAppStore((s) => s.showToast);
  const [tmdb, setTmdb] = useState("");
  const [bangumi, setBangumi] = useState("");
  const [concurrency, setConcurrency] = useState(4);
  const [language, setLanguage] = useState("zh-CN");
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    void (async () => {
      const config = await invoke<{
        apiKeys: { tmdb: string; bangumi?: string };
        scrapeConcurrency: number;
        metadataLanguage: string;
      }>("get_config");
      setTmdb(config.apiKeys.tmdb ?? "");
      setBangumi(config.apiKeys.bangumi ?? "");
      setConcurrency(config.scrapeConcurrency ?? 4);
      setLanguage(config.metadataLanguage ?? "zh-CN");
    })();
  }, []);

  const save = async () => {
    setSaving(true);
    try {
      const config = await invoke<Record<string, unknown>>("get_config");
      const apiKeys = {
        ...((config.apiKeys as Record<string, string>) ?? {}),
        tmdb,
        bangumi,
      };
      await invoke("save_config", {
        config: {
          ...config,
          apiKeys,
          scrapeConcurrency: Math.min(8, Math.max(1, concurrency)),
          metadataLanguage: language,
        },
      });
      showToast("设置已保存");
    } catch (err) {
      showToast(String(err));
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="space-y-3">
      <div className="kg-settings-group">
        <label className="block px-3.5 py-3">
          <span className="mb-1.5 block text-[12.5px] font-semibold text-fg-secondary">
            TMDB API Key (Bearer)
          </span>
          <input
            className="kg-field !py-2"
            value={tmdb}
            onChange={(e) => setTmdb(e.target.value)}
            placeholder="eyJhbGciOi..."
          />
        </label>
        <label className="block px-3.5 py-3">
          <span className="mb-1.5 block text-[12.5px] font-semibold text-fg-secondary">
            Bangumi Access Token（可选）
          </span>
          <input
            className="kg-field !py-2"
            value={bangumi}
            onChange={(e) => setBangumi(e.target.value)}
            placeholder="optional"
          />
        </label>
        <label className="block px-3.5 py-3">
          <span className="mb-1.5 block text-[12.5px] font-semibold text-fg-secondary">
            刮削并发（1–8）
          </span>
          <input
            type="number"
            min={1}
            max={8}
            className="kg-field !py-2"
            value={concurrency}
            onChange={(e) => setConcurrency(Number(e.target.value) || 4)}
          />
        </label>
        <label className="block px-3.5 py-3">
          <span className="mb-1.5 block text-[12.5px] font-semibold text-fg-secondary">
            元数据语言
          </span>
          <input
            className="kg-field !py-2"
            value={language}
            onChange={(e) => setLanguage(e.target.value)}
          />
        </label>
      </div>
      <button type="button" className="kg-btn" disabled={saving} onClick={() => void save()}>
        保存
      </button>
    </div>
  );
}

function LibrarySection() {
  const libraries = useAppStore((s) => s.libraries);
  const addLibrary = useAppStore((s) => s.addLibrary);
  const refreshLibraries = useAppStore((s) => s.refreshLibraries);
  const [mediaType, setMediaType] = useState<MediaType>("movie");
  const filtered = libraries.filter((l) => l.mediaType === mediaType);

  return (
    <div className="space-y-3">
      <div className="flex flex-wrap items-center gap-2">
        {(["movie", "tvShow", "anime"] as MediaType[]).map((type) => (
          <button
            key={type}
            type="button"
            data-selected={mediaType === type}
            onClick={() => setMediaType(type)}
            className="kg-chip"
          >
            {type === "movie" ? "Movie" : type === "tvShow" ? "TV Show" : "Anime"}
          </button>
        ))}
        <button
          type="button"
          onClick={() => void addLibrary(mediaType)}
          className="kg-btn ml-auto"
        >
          Add Library
        </button>
      </div>

      <div className="kg-settings-group">
        {filtered.length === 0 ? (
          <div className="px-3.5 py-8 text-center text-[12px] text-fg-muted">
            No libraries in this category.
          </div>
        ) : (
          filtered.map((lib) => (
            <LibraryRow key={lib.id} library={lib} onChanged={() => void refreshLibraries()} />
          ))
        )}
      </div>
    </div>
  );
}

function LibraryRow({
  library,
  onChanged,
}: {
  library: Library;
  onChanged: () => void;
}) {
  const [renaming, setRenaming] = useState(false);
  const [name, setName] = useState(library.name);

  const rename = async () => {
    const trimmed = name.trim();
    if (!trimmed) return;
    await invoke("rename_library", { id: library.id, name: trimmed });
    setRenaming(false);
    onChanged();
  };

  const remove = async () => {
    if (!window.confirm(`Remove library「${library.name}」? DB only, files stay.`)) return;
    await invoke("delete_library", { id: library.id });
    onChanged();
  };

  return (
    <div className="min-h-[64px] px-3.5 py-2">
      {renaming ? (
        <div className="flex items-center gap-2 py-1">
          <input
            value={name}
            onChange={(e) => setName(e.target.value)}
            className="kg-field flex-1 !py-2"
          />
          <button type="button" onClick={() => void rename()} className="kg-btn">
            Save
          </button>
          <button type="button" onClick={() => setRenaming(false)} className="kg-btn kg-btn-toolbar">
            Cancel
          </button>
        </div>
      ) : (
        <div className="flex items-center justify-between gap-3">
          <div className="min-w-0">
            <p className="truncate text-[13.5px] font-semibold text-fg">{library.name}</p>
            <p className="truncate font-mono text-[11.5px] text-fg-secondary">{library.rootPath}</p>
          </div>
          <div className="flex shrink-0 gap-1">
            <button type="button" onClick={() => setRenaming(true)} className="kg-btn kg-btn-toolbar">
              Rename
            </button>
            <button type="button" onClick={() => void remove()} className="kg-btn kg-btn-destructive">
              Remove
            </button>
          </div>
        </div>
      )}
    </div>
  );
}

function ScrapeExclusionsSection() {
  const [folders, setFolders] = useState<string[]>([]);
  const [draft, setDraft] = useState("");
  const [saving, setSaving] = useState(false);
  const [message, setMessage] = useState<string | null>(null);

  useEffect(() => {
    void (async () => {
      const config = await invoke<{ scanExcludedFolders: string[] }>("get_config");
      setFolders(config.scanExcludedFolders ?? []);
    })();
  }, []);

  const persist = async (next: string[]) => {
    setSaving(true);
    setMessage(null);
    try {
      const config = await invoke<Record<string, unknown>>("get_config");
      await invoke("save_config", {
        config: { ...config, scanExcludedFolders: next },
      });
      setFolders(next);
      setMessage("Saved");
    } catch (err) {
      setMessage(String(err));
    } finally {
      setSaving(false);
    }
  };

  const add = () => {
    const value = draft.trim();
    if (!value) return;
    if (folders.some((f) => f.toLowerCase() === value.toLowerCase())) {
      setDraft("");
      return;
    }
    void persist([...folders, value]);
    setDraft("");
  };

  const remove = (name: string) => {
    void persist(folders.filter((f) => f !== name));
  };

  return (
    <div className="space-y-3">
      <p className="px-1 text-[11.5px] leading-[1.45] text-fg-secondary">
        Folder names skipped while scanning (case-insensitive).
      </p>
      <div className="kg-settings-group">
        {folders.length === 0 ? (
          <div className="px-3.5 py-6 text-center text-[12px] text-fg-muted">No exclusions yet.</div>
        ) : (
          folders.map((name) => (
            <div
              key={name}
              className="flex min-h-[54px] items-center justify-between gap-3 px-3.5 py-1.5"
            >
              <span className="font-mono text-[12.5px] text-fg">{name}</span>
              <button type="button" onClick={() => remove(name)} className="kg-btn kg-btn-destructive">
                Remove
              </button>
            </div>
          ))
        )}
      </div>
      <div className="flex gap-2">
        <input
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") add();
          }}
          placeholder="Folder name"
          className="kg-field flex-1 !py-2"
        />
        <button type="button" disabled={saving} onClick={add} className="kg-btn">
          Add
        </button>
      </div>
      {message ? <p className="text-[11.5px] text-fg-secondary">{message}</p> : null}
    </div>
  );
}
