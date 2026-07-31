import { useEffect, useState } from "react";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";

type ActorAvatarProps = {
  url?: string | null;
  name: string;
  size?: number;
};

/** Lazy-load remote cast photo via CACHE-04 AvatarCache. */
export function ActorAvatar({ url, name, size = 48 }: ActorAvatarProps) {
  const [src, setSrc] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    if (!url) {
      setSrc(null);
      return;
    }
    void (async () => {
      try {
        const path = await invoke<string | null>("resolve_actor_avatar", { url });
        if (!cancelled && path) {
          setSrc(convertFileSrc(path));
        }
      } catch {
        if (!cancelled) setSrc(null);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [url]);

  const letter = (name.trim().charAt(0) || "?").toUpperCase();

  return (
    <div
      className="shrink-0 overflow-hidden rounded-full bg-subtle text-center"
      style={{ width: size, height: size }}
      title={name}
    >
      {src ? (
        <img src={src} alt="" className="h-full w-full object-cover" loading="lazy" />
      ) : (
        <span
          className="flex h-full w-full items-center justify-center kg-type-body-secondary font-semibold text-fg-muted"
          aria-hidden
        >
          {letter}
        </span>
      )}
    </div>
  );
}
