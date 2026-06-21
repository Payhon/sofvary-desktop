import { getCurrentWindow } from "@tauri-apps/api/window";
import { isTauriRuntime, safeInvoke } from "./tauriClient";

export type ShellWindowLabel = "main" | "command" | "glyph";

export function getCurrentWindowLabel(defaultLabel = "main"): string {
  if (!isTauriRuntime()) {
    const browserLabel = getBrowserWindowLabel();
    if (browserLabel) {
      return browserLabel;
    }
    return defaultLabel;
  }

  return getCurrentWindow().label;
}

function getBrowserWindowLabel(): ShellWindowLabel | null {
  if (typeof window === "undefined") {
    return null;
  }

  const label = new URLSearchParams(window.location.search).get("windowLabel");
  if (label === "main" || label === "command" || label === "glyph") {
    return label;
  }

  return null;
}

export async function startCurrentWindowDrag(label: "main" | "command" | "glyph"): Promise<void> {
  if (!isTauriRuntime()) {
    return;
  }

  try {
    await getCurrentWindow().startDragging();
  } catch {
    await safeInvoke<void>("start_window_drag", { label });
  }
}

export async function toggleWindowMaximize(label: ShellWindowLabel): Promise<void> {
  await safeInvoke<void>("toggle_shell_window_maximize", { label });
}

export async function minimizeWindow(label: ShellWindowLabel): Promise<void> {
  await safeInvoke<void>("minimize_shell_window", { label });
}

export async function hideWindow(label: ShellWindowLabel): Promise<void> {
  await safeInvoke<void>("hide_shell_window", { label });
}

export async function toggleCurrentWindowMaximize(label?: ShellWindowLabel): Promise<void> {
  if (!isTauriRuntime()) {
    return;
  }

  if (label) {
    await toggleWindowMaximize(label);
    return;
  }

  try {
    await getCurrentWindow().toggleMaximize();
  } catch {
    await toggleWindowMaximize(getCurrentWindow().label as ShellWindowLabel);
  }
}

export async function setCurrentWindowShadow(enable: boolean): Promise<void> {
  if (!isTauriRuntime()) {
    return;
  }

  await getCurrentWindow().setShadow(enable);
}
