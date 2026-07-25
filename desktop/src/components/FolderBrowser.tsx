import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";

type DirectoryEntry = {
  name: string;
  path: string;
  isDirectory: boolean;
  fileSize?: number | null;
  modifiedAt?: string | null;
};

type PathSegment = {
  name: string;
  path: string;
};

function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  if (n < 1024 * 1024 * 1024) return `${(n / (1024 * 1024)).toFixed(1)} MB`;
  return `${(n / (1024 * 1024 * 1024)).toFixed(2)} GB`;
}

export function FolderBrowser({
  rootPath,
  rootName,
  onClose,
}: {
  rootPath: string;
  rootName: string;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const [segments, setSegments] = useState<PathSegment[]>([
    { name: rootName, path: rootPath },
  ]);
  const [entries, setEntries] = useState<DirectoryEntry[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const current = segments[segments.length - 1]?.path ?? rootPath;

  useEffect(() => {
    setSegments([{ name: rootName, path: rootPath }]);
  }, [rootPath, rootName]);

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      setLoading(true);
      setError(null);
      try {
        const list = await invoke<DirectoryEntry[]>("list_directory", { path: current });
        if (!cancelled) setEntries(list);
      } catch (err) {
        if (!cancelled) {
          setEntries([]);
          setError(String(err));
        }
      } finally {
        if (!cancelled) setLoading(false);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [current]);

  function navigateTo(index: number) {
    setSegments((prev) => prev.slice(0, index + 1));
  }

  function enter(entry: DirectoryEntry) {
    if (!entry.isDirectory) return;
    setSegments((prev) => [...prev, { name: entry.name, path: entry.path }]);
  }

  return (
    <div className="flex min-h-0 flex-1 flex-col overflow-hidden">
      <div className="flex shrink-0 items-center gap-2 border-b border-hairline px-3 py-2">
        <button type="button" className="kg-btn kg-btn-toolbar shrink-0" onClick={onClose}>
          ← {t("browser.back")}
        </button>
        <nav className="flex min-w-0 flex-1 items-center gap-1 overflow-x-auto text-[12px]">
          {segments.map((seg, i) => (
            <span key={seg.path} className="flex shrink-0 items-center gap-1">
              {i > 0 ? <span className="text-fg-muted">/</span> : null}
              {i === segments.length - 1 ? (
                <span className="font-semibold text-fg">{seg.name}</span>
              ) : (
                <button
                  type="button"
                  className="text-fg-secondary hover:text-fg"
                  onClick={() => navigateTo(i)}
                >
                  {seg.name}
                </button>
              )}
            </span>
          ))}
        </nav>
      </div>

      <div className="min-h-0 flex-1 overflow-auto px-2 py-2">
        {loading ? (
          <p className="px-2 py-8 text-center text-[12.5px] text-fg-muted">{t("browser.loading")}</p>
        ) : error ? (
          <p className="px-2 py-8 text-center text-[12.5px] text-error">{error}</p>
        ) : entries.length === 0 ? (
          <p className="px-2 py-8 text-center text-[12.5px] text-fg-muted">{t("browser.empty")}</p>
        ) : (
          <ul className="space-y-0.5">
            {entries.map((entry) => (
              <li key={entry.path}>
                <button
                  type="button"
                  className="flex w-full items-center gap-2 rounded-control px-2 py-1.5 text-left text-[12.5px] hover:bg-subtle"
                  onClick={() => enter(entry)}
                  onDoubleClick={() => {
                    if (!entry.isDirectory) {
                      /* files: no-op beyond reveal elsewhere */
                    }
                  }}
                  disabled={!entry.isDirectory}
                >
                  <span className="w-5 shrink-0 text-fg-muted">
                    {entry.isDirectory ? "▸" : "·"}
                  </span>
                  <span className="min-w-0 flex-1 truncate font-medium text-fg">{entry.name}</span>
                  {!entry.isDirectory && entry.fileSize != null ? (
                    <span className="shrink-0 text-[11px] text-fg-muted">
                      {formatBytes(entry.fileSize)}
                    </span>
                  ) : null}
                  {entry.modifiedAt ? (
                    <span className="shrink-0 text-[11px] text-fg-muted">{entry.modifiedAt}</span>
                  ) : null}
                </button>
              </li>
            ))}
          </ul>
        )}
      </div>
    </div>
  );
}
