import { useMemo, useState } from "react";
import { useTranslation } from "react-i18next";

import type { ResidualCandidate } from "../store/appStore";

export type { ResidualCandidate };

export function CleanupSheet({
  candidates,
  onConfirm,
  onClose,
}: {
  candidates: ResidualCandidate[];
  onConfirm: (paths: string[]) => void;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const [selected, setSelected] = useState<Set<string>>(
    () => new Set(candidates.map((c) => c.path)),
  );

  const totalSize = useMemo(
    () =>
      candidates
        .filter((c) => selected.has(c.path))
        .reduce((sum, c) => sum + (c.size || 0), 0),
    [candidates, selected],
  );

  const toggle = (path: string) => {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(path)) next.delete(path);
      else next.add(path);
      return next;
    });
  };

  const toggleAll = (on: boolean) => {
    setSelected(on ? new Set(candidates.map((c) => c.path)) : new Set());
  };

  const basename = (p: string) => {
    const parts = p.split(/[/\\]/);
    return parts[parts.length - 1] || p;
  };

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-barrier px-5 py-6"
      onClick={onClose}
      role="presentation"
    >
      <div
        className="kg-glass kg-dialog flex max-h-[min(640px,calc(100vh-48px))] w-full max-w-[520px] flex-col"
        onClick={(e) => e.stopPropagation()}
        role="dialog"
        aria-modal="true"
        aria-labelledby="cleanup-title"
      >
        <header className="kg-dialog-header shrink-0">
          <h2
            id="cleanup-title"
            className="truncate text-[20px] font-extrabold tracking-[-0.25px] text-fg"
          >
            {t("cleanup.title")}
          </h2>
          <p className="mt-2 text-[13.5px] leading-[1.45] text-fg-secondary">
            {t("cleanup.subtitle", { count: candidates.length })}
          </p>
        </header>

        <div className="flex shrink-0 items-center justify-between gap-2 border-b border-hairline px-6 py-2">
          <label className="flex cursor-pointer items-center gap-2 text-[12.5px] text-fg-secondary">
            <input
              type="checkbox"
              checked={selected.size === candidates.length && candidates.length > 0}
              onChange={(e) => toggleAll(e.target.checked)}
            />
            {t("cleanup.selectAll")}
          </label>
          <span className="text-[11.5px] text-fg-muted">
            {t("cleanup.selectedBytes", {
              count: selected.size,
              size: formatBytes(totalSize),
            })}
          </span>
        </div>

        <ul className="min-h-0 flex-1 overflow-auto px-3 py-2">
          {candidates.map((c) => (
            <li key={c.path}>
              <label className="flex cursor-pointer items-start gap-2.5 rounded-control px-2.5 py-2 hover:bg-fill-secondary/40">
                <input
                  type="checkbox"
                  className="mt-1"
                  checked={selected.has(c.path)}
                  onChange={() => toggle(c.path)}
                />
                <span className="min-w-0 flex-1">
                  <span className="block truncate text-[13px] font-semibold text-fg">
                    {basename(c.path)}
                  </span>
                  <span className="mt-0.5 block truncate text-[11px] text-fg-muted" title={c.path}>
                    {c.itemTitle} · {c.path}
                  </span>
                </span>
              </label>
            </li>
          ))}
        </ul>

        <div className="kg-dialog-footer shrink-0">
          <button type="button" className="kg-btn kg-btn-toolbar" onClick={onClose}>
            {t("settings.close")}
          </button>
          <button
            type="button"
            className="kg-btn kg-btn-destructive"
            disabled={selected.size === 0}
            onClick={() => onConfirm([...selected])}
          >
            {t("cleanup.confirm")}
          </button>
        </div>
      </div>
    </div>
  );
}

function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  return `${(n / (1024 * 1024)).toFixed(1)} MB`;
}
