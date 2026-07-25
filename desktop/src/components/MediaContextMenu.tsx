import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

export type MediaContextMenuTarget = {
  x: number;
  y: number;
  itemId: string;
};

type MediaContextMenuProps = {
  menu: MediaContextMenuTarget;
  canScrapeAuto: boolean;
  canRescrape: boolean;
  canManualMatch: boolean;
  canRename: boolean;
  canOrganize: boolean;
  canMergeDuplicates: boolean;
  canCleanResiduals: boolean;
  canDelete: boolean;
  onClose: () => void;
  /** Close menu and keep the right-click selection (for the chosen action). */
  onAction: () => void;
  onScrapeConfirm: () => void;
  onScrapeAuto: () => void;
  onRescrape: () => void;
  onManualMatch: () => void;
  onRename: () => void;
  onOrganize: () => void;
  onMergeDuplicates: () => void;
  onCleanResiduals: () => void;
  onRefreshFromDisk: () => void;
  onReveal: () => void;
  onDelete: () => void;
};

export function MediaContextMenu({
  menu,
  canScrapeAuto,
  canRescrape,
  canManualMatch,
  canRename,
  canOrganize,
  canMergeDuplicates,
  canCleanResiduals,
  canDelete,
  onClose,
  onAction,
  onScrapeConfirm,
  onScrapeAuto,
  onRescrape,
  onManualMatch,
  onRename,
  onOrganize,
  onMergeDuplicates,
  onCleanResiduals,
  onRefreshFromDisk,
  onReveal,
  onDelete,
}: MediaContextMenuProps) {
  const { t } = useTranslation();
  const rootRef = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState({ left: menu.x, top: menu.y });

  useEffect(() => {
    const onDoc = (e: MouseEvent) => {
      if (!rootRef.current?.contains(e.target as Node)) onClose();
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    document.addEventListener("mousedown", onDoc);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDoc);
      document.removeEventListener("keydown", onKey);
    };
  }, [onClose]);

  useLayoutEffect(() => {
    const el = rootRef.current;
    if (!el) return;
    const rect = el.getBoundingClientRect();
    const pad = 8;
    setPos({
      left: Math.max(pad, Math.min(menu.x, window.innerWidth - rect.width - pad)),
      top: Math.max(pad, Math.min(menu.y, window.innerHeight - rect.height - pad)),
    });
  }, [menu.x, menu.y, canScrapeAuto, canRescrape, canManualMatch, canRename, canOrganize, canMergeDuplicates, canCleanResiduals, canDelete]);

  const run = (action: () => void) => {
    onAction();
    action();
  };

  return (
    <div
      ref={rootRef}
      className="kg-menu fixed z-[80]"
      style={pos}
      role="menu"
    >
      {canManualMatch ? (
        <button
          type="button"
          role="menuitem"
          className="kg-menu-item"
          onClick={() => run(onScrapeConfirm)}
        >
          {t("action.scrapeItem")}
        </button>
      ) : null}
      {canScrapeAuto ? (
        <button
          type="button"
          role="menuitem"
          className="kg-menu-item"
          onClick={() => run(onScrapeAuto)}
        >
          {t("action.scrapeAuto")}
        </button>
      ) : null}
      {canRescrape ? (
        <button
          type="button"
          role="menuitem"
          className="kg-menu-item"
          onClick={() => run(onRescrape)}
        >
          {t("action.rescrape")}
        </button>
      ) : null}
      {canManualMatch ? (
        <button
          type="button"
          role="menuitem"
          className="kg-menu-item"
          onClick={() => run(onManualMatch)}
        >
          {t("action.manualMatch")}
        </button>
      ) : null}
      {canRename ? (
        <button
          type="button"
          role="menuitem"
          className="kg-menu-item"
          onClick={() => run(onRename)}
        >
          {t("action.applyRename")}
        </button>
      ) : null}
      {canOrganize ? (
        <button
          type="button"
          role="menuitem"
          className="kg-menu-item"
          onClick={() => run(onOrganize)}
        >
          {t("action.organizeSeasons")}
        </button>
      ) : null}
      {canMergeDuplicates ? (
        <button
          type="button"
          role="menuitem"
          className="kg-menu-item"
          onClick={() => run(onMergeDuplicates)}
        >
          {t("action.mergeDuplicates")}
        </button>
      ) : null}
      {canCleanResiduals ? (
        <button
          type="button"
          role="menuitem"
          className="kg-menu-item"
          onClick={() => run(onCleanResiduals)}
        >
          {t("action.cleanResiduals")}
        </button>
      ) : null}
      <button
        type="button"
        role="menuitem"
        className="kg-menu-item"
        onClick={() => run(onRefreshFromDisk)}
      >
        {t("action.refreshFromDisk")}
      </button>
      <button
        type="button"
        role="menuitem"
        className="kg-menu-item"
        onClick={() => run(onReveal)}
      >
        {t("action.revealInFinder")}
      </button>
      {canDelete ? (
        <button
          type="button"
          role="menuitem"
          className="kg-menu-item"
          data-destructive="true"
          onClick={() => run(onDelete)}
        >
          {t("action.deleteItem")}
        </button>
      ) : null}
    </div>
  );
}
