import { safeInvoke } from "./tauriClient";

type CommandWindowMinimizeCommand = "hide_command_window" | "minimize_command_window";

export async function showCommandWindow(): Promise<void> {
  await safeInvoke<void>("show_command_window");
}

export async function showMainWindow(): Promise<void> {
  await safeInvoke<void>("show_main_window");
}

export async function hideCommandWindow(): Promise<void> {
  await safeInvoke<void>("hide_command_window");
}

export function getCommandWindowMinimizeCommand(
  hasActiveTask: boolean,
): CommandWindowMinimizeCommand {
  return hasActiveTask ? "minimize_command_window" : "hide_command_window";
}

export async function minimizeCommandWindow(options: { hasActiveTask?: boolean } = {}): Promise<void> {
  await safeInvoke<void>(getCommandWindowMinimizeCommand(options.hasActiveTask === true));
}

export async function snapCommandWindow(): Promise<void> {
  await safeInvoke<void>("snap_command_window");
}
