import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { InstalledPackSummary } from "../../types";
import { formatPackLabel, formatPackStatus, sortInstalledPacks } from "./packLogic";

describe("packLogic", () => {
  it("formats pack labels and status text", () => {
    assert.equal(formatPackLabel({ id: "sofvary.runtime.static-html", version: "0.1.0" }), "sofvary.runtime.static-html@0.1.0");
    assert.equal(formatPackStatus({ kind: "idle" }), "Installed packs ready.");
    assert.equal(formatPackStatus({ kind: "error", detail: "Registry unavailable." }), "Registry unavailable.");
  });

  it("sorts installed packs by kind, id, and version", () => {
    const packs: InstalledPackSummary[] = [
      pack("plugin", "sofvary.plugin.demo", "0.1.0"),
      pack("runtime", "sofvary.runtime.react-vite", "0.1.0"),
      pack("harness", "sofvary.harness.react-vite", "0.1.0"),
      pack("runtime", "sofvary.runtime.static-html", "0.1.0"),
    ];

    assert.deepEqual(sortInstalledPacks(packs).map(formatPackLabel), [
      "sofvary.runtime.react-vite@0.1.0",
      "sofvary.runtime.static-html@0.1.0",
      "sofvary.harness.react-vite@0.1.0",
      "sofvary.plugin.demo@0.1.0",
    ]);
  });
});

function pack(kind: InstalledPackSummary["kind"], id: string, version: string): InstalledPackSummary {
  return {
    id,
    version,
    kind,
    name: id,
    description: "",
    source: "cache",
  };
}
