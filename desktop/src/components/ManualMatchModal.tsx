import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";

import { useAppStore, type MediaType, type TaskSnapshot } from "../store/appStore";

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
  const { t } = useTranslation();
  const showToast = useAppStore((s) => s.showToast);
  const upsertTask = useAppStore((s) => s.upsertTask);
  const [query, setQuery] = useState(initialQuery);
  const [loading, setLoading] = useState(false);
  const [applying, setApplying] = useState(false);
  const [candidates, setCandidates] = useState<MatchCandidate[]>([]);
  const [searched, setSearched] = useState(false);

  const search = async () => {
    setLoading(true);
    try {
      const rows = await invoke<MatchCandidate[]>("search_match_candidates", {
        query,
        mediaType,
      });
      setCandidates(rows);
      setSearched(true);
      if (rows.length === 0) {
        showToast(t("match.noResults"));
      }
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
    if (applying) return;
    setApplying(true);
    try {
      const task = await invoke<TaskSnapshot>("apply_manual_match", {
        itemId,
        sourceId,
      });
      upsertTask(task);
      showToast(t("toast.manualMatchStarted"));
      onClose();
    } catch (err) {
      showToast(String(err));
      setApplying(false);
    }
  };

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-barrier px-5 py-6"
      onClick={onClose}
      onKeyDown={(e) => {
        if (e.key === "Escape") onClose();
      }}
      role="presentation"
    >
      <div
        className="kg-glass kg-dialog"
        onClick={(e) => e.stopPropagation()}
        role="dialog"
        aria-modal="true"
        aria-labelledby="manual-match-title"
      >
        <header className="kg-dialog-header flex items-center justify-between gap-3">
          <h2
            id="manual-match-title"
            className="truncate kg-type-section-title font-extrabold tracking-[-0.25px] text-fg"
          >
            {t("action.manualMatch")}
          </h2>
          <button type="button" className="kg-btn kg-btn-toolbar" onClick={onClose}>
            {t("settings.close")}
          </button>
        </header>

        <div className="flex gap-2 px-6 pb-3">
          <input
            className="kg-field kg-field-compact flex-1"
            value={query}
            autoFocus
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") void search();
            }}
          />
          <button
            type="button"
            className="kg-btn"
            disabled={loading || applying}
            onClick={() => void search()}
          >
            {loading ? t("match.searching") : t("action.search")}
          </button>
        </div>

        <ul className="kg-dialog-body !px-3">
          {candidates.length === 0 ? (
            <li className="px-2 py-8 text-center kg-type-body-secondary text-fg-muted">
              {loading
                ? t("match.searching")
                : searched
                  ? t("match.noResults")
                  : t("match.searching")}
            </li>
          ) : (
            candidates.map((c) => (
              <li key={c.sourceId}>
                <button
                  type="button"
                  disabled={applying}
                  onClick={() => void apply(c.sourceId)}
                  className="kg-list-row !min-h-[58px] rounded-control"
                >
                  <span className="min-w-0 flex-1">
                    <span className="kg-list-row-title">
                      {c.title}
                      {c.year ? (
                        <span className="ml-2 font-medium text-fg-secondary">({c.year})</span>
                      ) : null}
                    </span>
                    <span className="kg-list-row-subtitle">
                      {c.sourceId} · {(c.confidence * 100).toFixed(0)}%
                    </span>
                    {c.overview ? (
                      <span className="mt-1 line-clamp-2 kg-type-caption text-fg-muted">
                        {c.overview}
                      </span>
                    ) : null}
                  </span>
                </button>
              </li>
            ))
          )}
        </ul>
      </div>
    </div>
  );
}
