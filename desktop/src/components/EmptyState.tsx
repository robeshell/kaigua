import type { ReactNode } from "react";

type EmptyStateProps = {
  title: string;
  message: string;
  action?: ReactNode;
  /** When false, omit the default muted outline icon. */
  icon?: boolean;
  className?: string;
};

export function EmptyState({
  title,
  message,
  action,
  icon = true,
  className = "",
}: EmptyStateProps) {
  return (
    <div
      className={`flex flex-col items-center justify-center px-6 text-center ${className}`}
    >
      <div className="w-full max-w-[420px]">
        {icon ? (
          <svg
            className="mx-auto mb-3.5 text-fg-muted opacity-[0.68]"
            width="30"
            height="30"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.25"
            strokeLinecap="round"
            strokeLinejoin="round"
            aria-hidden
          >
            <rect x="2" y="6" width="20" height="12" rx="2" />
            <path d="M7 6V5a2 2 0 0 1 2-2h6a2 2 0 0 1 2 2v1" />
            <path d="M10 12h4" />
          </svg>
        ) : null}
        <p className="kg-type-title font-semibold leading-snug text-fg/88">{title}</p>
        <p className="mt-1.5 kg-type-body-secondary leading-[1.45] text-fg-muted/76">{message}</p>
        {action ? <div className="mt-3.5 flex justify-center">{action}</div> : null}
      </div>
    </div>
  );
}
