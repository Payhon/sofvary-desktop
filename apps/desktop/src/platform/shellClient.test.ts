import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { getCommandWindowMinimizeCommand } from "./shellClient";

describe("command window minimize behavior", () => {
  it("hides to tray or menu bar by default", () => {
    assert.equal(getCommandWindowMinimizeCommand(false), "hide_command_window");
  });

  it("keeps the glyph path only for active shell tasks", () => {
    assert.equal(getCommandWindowMinimizeCommand(true), "minimize_command_window");
  });
});
