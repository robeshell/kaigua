/** Windows uses undecorated immersive chrome; macOS keeps Overlay + traffic lights. */
export function isImmersiveWindow(): boolean {
  return navigator.userAgent.includes("Windows");
}
