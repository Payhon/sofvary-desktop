import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { readdirSync, readFileSync, statSync } from "node:fs";
import { join, relative } from "node:path";

const uiRoots = ["src/App.tsx", "src/ui"];

describe("UI import boundary", () => {
  it("keeps direct Tauri imports inside platform wrappers", () => {
    const violations = uiRoots
      .flatMap((path) => sourceFiles(path))
      .filter((file) => directTauriImportPattern.test(readFileSync(file, "utf8")))
      .map((file) => relative(process.cwd(), file));

    assert.deepEqual(violations, []);
  });

  it("keeps build start on the merged create surface without fixed launch delay", () => {
    const source = readFileSync(
      join(process.cwd(), "src/ui/floating-command-menu/CommandWindowRoot.tsx"),
      "utf8",
    );

    assert.equal(source.includes("wait(420"), false);
    assert.equal(source.includes('setActiveAction("任务")'), false);
  });

  it("keeps the merged create surface able to return to a new draft", () => {
    const rootSource = readFileSync(
      join(process.cwd(), "src/ui/floating-command-menu/CommandWindowRoot.tsx"),
      "utf8",
    );
    const menuSource = readFileSync(
      join(process.cwd(), "src/ui/floating-command-menu/FloatingCommandMenu.tsx"),
      "utf8",
    );

    assert.equal(rootSource.includes("startNewBuildThreadDraft"), true);
    assert.equal(rootSource.includes("setActiveThreadId(null)"), true);
    assert.equal(rootSource.includes('setCreatePrompt("")'), true);
    assert.equal(menuSource.includes("onStartNewBuildThreadDraft"), true);
    assert.equal(menuSource.includes("onStartNew"), true);
    assert.equal(menuSource.includes('t("task.new")'), true);
  });
});

const directTauriImportPattern = /^\s*import\s+.*["']@tauri-apps\//m;

function sourceFiles(path: string): string[] {
  const absolute = join(process.cwd(), path);
  const stat = statSync(absolute);
  if (stat.isFile()) {
    return absolute.endsWith(".ts") || absolute.endsWith(".tsx") ? [absolute] : [];
  }

  return readdirSync(absolute)
    .flatMap((entry) => sourceFiles(join(path, entry)))
    .sort();
}
