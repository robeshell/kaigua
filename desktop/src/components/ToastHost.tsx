import { useAppStore } from "../store/appStore";

export function ToastHost() {
  const message = useAppStore((s) => s.toastMessage);
  if (!message) return null;
  return (
    <div className="pointer-events-none fixed inset-x-0 bottom-9 z-50 flex justify-center px-4">
      <div className="min-w-[220px] max-w-[min(420px,calc(100%-2rem))] rounded-menu border border-hairline bg-overlay px-4 py-2.5 text-center text-[13.5px] font-medium text-fg shadow-[0_8px_24px_rgb(0_0_0_/_0.09)]">
        {message}
      </div>
    </div>
  );
}
