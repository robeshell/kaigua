import { useAppStore } from "../store/appStore";

export function ToastHost() {
  const message = useAppStore((s) => s.toastMessage);
  if (!message) return null;
  return (
    <div className="pointer-events-none fixed inset-x-0 bottom-9 z-50 flex justify-center px-4">
      <div className="kg-glass min-w-[220px] max-w-[min(420px,calc(100%-2rem))] rounded-menu px-4 py-2.5 text-center text-[13.5px] font-medium text-fg">
        {message}
      </div>
    </div>
  );
}
