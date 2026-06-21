import { useEffect, useRef } from "react";
import { safeInvoke } from "../../platform/tauriClient";
import {
  evaluateShortcutSequence,
  type ShortcutSequenceState,
} from "./shortcutSequence";

export function useGlobalShortcutState(onSummon: () => void) {
  const shortcutState = useRef<ShortcutSequenceState>({ altAAt: null });

  useEffect(() => {
    safeInvoke("bootstrap_platform").catch(() => {
      // Browser-only Vite sessions use the keyboard fallback below.
    });

    const onKeyDown = (event: KeyboardEvent) => {
      const result = evaluateShortcutSequence(shortcutState.current, event, Date.now());
      shortcutState.current = result.nextState;

      if (result.shouldPreventDefault) {
        event.preventDefault();
      }

      if (result.shouldSummon) {
        onSummon();
      }

      if (event.key.toLowerCase() === "escape") {
        window.dispatchEvent(new CustomEvent("sofvary:escape"));
      }
    };

    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [onSummon]);
}
