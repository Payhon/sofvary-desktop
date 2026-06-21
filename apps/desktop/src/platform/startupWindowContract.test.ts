import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, it } from "node:test";

type TauriWindowConfig = {
  label: string;
  visible?: boolean;
};

type TauriConfig = {
  app: {
    windows: TauriWindowConfig[];
  };
};

function readStartupWindows(): Map<string, TauriWindowConfig> {
  const configPath = join(process.cwd(), "src-tauri", "tauri.conf.json");
  const config = JSON.parse(readFileSync(configPath, "utf8")) as TauriConfig;

  return new Map(config.app.windows.map((window) => [window.label, window]));
}

describe("startup window contract", () => {
  it("shows the main app window while keeping stealth UI windows hidden", () => {
    const windows = readStartupWindows();

    assert.equal(windows.get("main")?.visible, true);
    assert.equal(windows.get("command")?.visible, false);
    assert.equal(windows.get("glyph")?.visible, false);
  });
});
