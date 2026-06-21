import test from "node:test";
import assert from "node:assert/strict";
import { getCurrentWindowLabel } from "./windowClient";

test("getCurrentWindowLabel uses the browser windowLabel query parameter outside Tauri", () => {
  const previousWindow = globalThis.window;
  Object.defineProperty(globalThis, "window", {
    configurable: true,
    value: {
      location: {
        search: "?windowLabel=command",
      },
    },
  });

  assert.equal(getCurrentWindowLabel("main"), "command");

  Object.defineProperty(globalThis, "window", {
    configurable: true,
    value: previousWindow,
  });
});

test("getCurrentWindowLabel ignores unknown browser labels outside Tauri", () => {
  const previousWindow = globalThis.window;
  Object.defineProperty(globalThis, "window", {
    configurable: true,
    value: {
      location: {
        search: "?windowLabel=workspace",
      },
    },
  });

  assert.equal(getCurrentWindowLabel("main"), "main");

  Object.defineProperty(globalThis, "window", {
    configurable: true,
    value: previousWindow,
  });
});
