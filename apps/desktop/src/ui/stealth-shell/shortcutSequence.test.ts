import assert from "node:assert/strict";
import { describe, it } from "node:test";
import {
  evaluateShortcutSequence,
  SHORTCUT_SEQUENCE_WINDOW_MS,
  type ShortcutSequenceState,
} from "./shortcutSequence";

describe("ALT + A + I shortcut sequence", () => {
  it("records ALT + A without summoning the menu", () => {
    const state: ShortcutSequenceState = { altAAt: null };

    const result = evaluateShortcutSequence(state, { altKey: true, key: "a" }, 1000);

    assert.equal(result.shouldPreventDefault, true);
    assert.equal(result.shouldSummon, false);
    assert.equal(result.nextState.altAAt, 1000);
  });

  it("summons only when ALT + I follows ALT + A inside the sequence window", () => {
    const state: ShortcutSequenceState = { altAAt: 1000 };

    const result = evaluateShortcutSequence(state, { altKey: true, key: "i" }, 1600);

    assert.equal(result.shouldPreventDefault, true);
    assert.equal(result.shouldSummon, true);
    assert.equal(result.nextState.altAAt, null);
  });

  it("does not summon on ALT + I alone", () => {
    const state: ShortcutSequenceState = { altAAt: null };

    const result = evaluateShortcutSequence(state, { altKey: true, key: "i" }, 1000);

    assert.equal(result.shouldPreventDefault, false);
    assert.equal(result.shouldSummon, false);
    assert.equal(result.nextState.altAAt, null);
  });

  it("does not summon after the sequence window expires", () => {
    const state: ShortcutSequenceState = { altAAt: 1000 };

    const result = evaluateShortcutSequence(
      state,
      { altKey: true, key: "i" },
      1000 + SHORTCUT_SEQUENCE_WINDOW_MS + 1,
    );

    assert.equal(result.shouldPreventDefault, false);
    assert.equal(result.shouldSummon, false);
    assert.equal(result.nextState.altAAt, 1000);
  });
});
