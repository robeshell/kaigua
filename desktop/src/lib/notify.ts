import {
  isPermissionGranted,
  requestPermission,
  sendNotification,
} from "@tauri-apps/plugin-notification";

/** PLAT-07: OS notification when a long task finishes (best-effort). */
export async function notifyTaskDone(title: string, body?: string): Promise<void> {
  try {
    let granted = await isPermissionGranted();
    if (!granted) {
      const permission = await requestPermission();
      granted = permission === "granted";
    }
    if (!granted) return;
    sendNotification({
      title,
      body: body?.trim() || undefined,
    });
  } catch {
    /* notifications optional — toast remains primary feedback */
  }
}
