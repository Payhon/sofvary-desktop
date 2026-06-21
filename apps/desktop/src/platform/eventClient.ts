import { emit, listen } from "@tauri-apps/api/event";
import { isTauriRuntime } from "./tauriClient";

export type ShellEventName =
  | "sofvary-build-state"
  | "sofvary-build-thread-updated"
  | "sofvary-build-thread-entry"
  | "sofvary-runtime-preview"
  | "sofvary-runtime-error"
  | "sofvary-agent-install-updated"
  | "sofvary-agent-install-log"
  | "sofvary-runtime-environment-install-updated";

export async function emitShellEvent<T>(eventName: ShellEventName, payload: T): Promise<void> {
  if (!isTauriRuntime()) {
    return;
  }

  await emit(eventName, payload);
}

export function listenShellEvent<T>(
  eventName: ShellEventName,
  handler: (payload: T) => void,
): Promise<() => void> {
  if (!isTauriRuntime()) {
    return Promise.resolve(() => {});
  }

  return listen<T>(eventName, (event) => handler(event.payload));
}
