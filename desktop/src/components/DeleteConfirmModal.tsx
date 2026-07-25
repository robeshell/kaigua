import { useState } from "react";
import { useTranslation } from "react-i18next";

export function DeleteConfirmModal({
  title,
  onConfirm,
  onClose,
}: {
  title: string;
  onConfirm: (alsoTrash: boolean) => void;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const [alsoTrash, setAlsoTrash] = useState(false);

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-barrier px-5 py-6"
      onClick={onClose}
      role="presentation"
    >
      <div
        className="kg-glass kg-dialog max-w-[400px]"
        onClick={(e) => e.stopPropagation()}
        role="dialog"
        aria-modal="true"
        aria-labelledby="delete-confirm-title"
      >
        <header className="kg-dialog-header">
          <h2
            id="delete-confirm-title"
            className="truncate text-[20px] font-extrabold tracking-[-0.25px] text-fg"
          >
            {t("delete.title")}
          </h2>
          <p className="mt-2 text-[13.5px] leading-[1.45] text-fg-secondary">
            {t("delete.message", { title })}
          </p>
        </header>
        <div className="px-6 pb-2">
          <label className="flex cursor-pointer items-start gap-2.5 rounded-control px-1 py-2">
            <input
              type="checkbox"
              checked={alsoTrash}
              onChange={(e) => setAlsoTrash(e.target.checked)}
              className="mt-0.5"
            />
            <span className="min-w-0">
              <span className="block text-[13.5px] font-semibold text-fg">
                {t("delete.alsoTrash")}
              </span>
              <span className="mt-0.5 block text-[11.5px] leading-[1.45] text-fg-muted">
                {t("delete.alsoTrashHint")}
              </span>
            </span>
          </label>
        </div>
        <div className="kg-dialog-footer">
          <button type="button" className="kg-btn kg-btn-toolbar" onClick={onClose}>
            {t("settings.close")}
          </button>
          <button
            type="button"
            className="kg-btn kg-btn-destructive"
            onClick={() => onConfirm(alsoTrash)}
          >
            {t("delete.confirm")}
          </button>
        </div>
      </div>
    </div>
  );
}
