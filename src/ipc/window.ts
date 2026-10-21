import { isTauri } from "@tauri-apps/api/core";
import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";

export type WindowLayout = "compact" | "expanded" | "settings";
const heights: Record<WindowLayout, number> = {
  compact: 240,
  expanded: 460,
  settings: 620,
};
let revision = 0;

/** Keep user-selected width; resize only when the disclosed surface changes. */
export async function resizeWindow(layout: WindowLayout): Promise<void> {
  if (!isTauri()) return;
  const request = ++revision;
  const window = getCurrentWindow();
  const [size, scale] = await Promise.all([
    window.innerSize(),
    window.scaleFactor(),
  ]);
  if (request !== revision) return;
  await window.setSize(
    new LogicalSize(Math.max(480, size.width / scale), heights[layout]),
  );
}
