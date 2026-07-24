import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

import { useAppStore, type MediaType } from "../store/appStore";

export type MatchCandidate = {
  sourceId: string;
  title: string;
  originalTitle?: string | null;
  year?: number | null;
  overview?: string | null;
  posterUrl?: string | null;
  confidence: number;
  mediaType: MediaType;
};

export function ManualMatchModal({
  itemId,
  mediaType,
  initialQuery,
  onClose,
}: {
  itemId: string;
  mediaType: MediaType;
  initialQuery: string;
  onClose: () => void;
}) {
  const showToast = useAppStore((s) => s.showToast);
  const selectLibrary = useAppStore((s) => s.selectLibrary);
  const selectedLibraryId = useAppStore((s) => s.selectedLibraryId);
  const [query, setQuery] = useState(initialQuery);
  const [loading, setLoading] = useState(false);
  const [applying, setApplying] = useState(false);
  const [candidates, setCandidates] = useState<MatchCandidate[]>([]);

  const search = async () => {
    setLoading(true);
    try {
      const rows = await invoke<MatchCandidate[]>("search_match_candidates", {
        query,
        mediaType,
      });
      setCandidates(rows);
    } catch (err) {
      showToast(String(err));
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    void search();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const apply = async (sourceId: string) => {
    setApplying(true);
    try {
      await invoke("apply_manual_match", { itemId, sourceId });
      showToast("手动匹配已写入");
      if (selectedLibraryId) await selectLibrary(selectedLibraryId);
      onClose();
    } catch (err) {
      showToast(String(err));
    } finally {
      setApplying(false);
    }
  };

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-barrier p-5"
      onClick={onClose}
      role="presentation"
    >
      <div
        className="flex max-h-[min(640px,90vh)] w-full max-w-[520px] flex-col overflow-hidden rounded-dialog border border-glass-border bg-elevated shadow-[0_8px_28px_rgb(0_0_0_/_0.09)]"
        onClick={(e) => e.stopPropagation()}
        role="dialog"
        aria-modal="true"
      >
        <header className="flex items-center justify-between gap-3 px-5 pb-3 pt-5">
          <h2 className="text-[20px] font-extrabold tracking-[-0.25px] text-fg">手动匹配</h2>
          <button type="button" className="kg-btn kg-btn-toolbar" onClick={onClose}>
            关闭
          </button>
        </header>
        <div className="flex gap-2 px-5 pb-3">
          <input
            className="kg-field flex-1 !py-2"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") void search();
            }}
          />
          <button type="button" className="kg-btn" disabled={loading} onClick={() => void search()}>
            {loading ? "…" : "搜索"}
          </button>
        </div>
        <ul className="min-h-0 flex-1 overflow-auto px-3 pb-5">
          {candidates.length === 0 ? (
            <li className="px-2 py-8 text-center text-[12px] text-fg-muted">无结果</li>
          ) : (
            candidates.map((c) => (
              <li key={c.sourceId}>
                <button
                  type="button"
                  disabled={applying}
                  onClick={() => void apply(c.sourceId)}
                  className="flex w-full items-start gap-3 rounded-control px-2 py-2.5 text-left hover:bg-row-hover"
                >
                  <div className="min-w-0 flex-1">
                    <p className="truncate text-[13.5px] font-semibold text-fg">
                      {c.title}
                      {c.year ? (
                        <span className="ml-2 font-medium text-fg-secondary">({c.year})</span>
                      ) : null}
                    </p>
                    <p className="mt-0.5 truncate text-[11.5px] text-fg-secondary">
                      {c.sourceId} · {(c.confidence * 100).toFixed(0)}%
                    </p>
                    {c.overview ? (
                      <p className="mt-1 line-clamp-2 text-[11.5px] text-fg-muted">{c.overview}</p>
                    ) : null}
                  </div>
                </button>
              </li>
            ))
          )}
        </ul>
      </div>
    </div>
  );
}
