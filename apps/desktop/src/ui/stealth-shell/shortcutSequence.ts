export interface ShortcutSequenceState {
  altAAt: number | null;
}

export interface ShortcutKeyEvent {
  altKey: boolean;
  key: string;
}

export interface ShortcutSequenceResult {
  nextState: ShortcutSequenceState;
  shouldPreventDefault: boolean;
  shouldSummon: boolean;
}

export const SHORTCUT_SEQUENCE_WINDOW_MS = 1200;

export function evaluateShortcutSequence(
  state: ShortcutSequenceState,
  event: ShortcutKeyEvent,
  now: number,
): ShortcutSequenceResult {
  const key = event.key.toLowerCase();

  if (event.altKey && key === "a") {
    return {
      nextState: { altAAt: now },
      shouldPreventDefault: true,
      shouldSummon: false,
    };
  }

  if (
    event.altKey &&
    key === "i" &&
    state.altAAt !== null &&
    now - state.altAAt <= SHORTCUT_SEQUENCE_WINDOW_MS
  ) {
    return {
      nextState: { altAAt: null },
      shouldPreventDefault: true,
      shouldSummon: true,
    };
  }

  return {
    nextState: state,
    shouldPreventDefault: false,
    shouldSummon: false,
  };
}
