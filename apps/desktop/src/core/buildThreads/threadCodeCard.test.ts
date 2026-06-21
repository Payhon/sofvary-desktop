import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { parseThreadCodeCard, tokenizeJsonHighlight } from "./threadCodeCard";

describe("thread code card parsing", () => {
  it("parses agent files JSON into a compact code card model", () => {
    const card = parseThreadCodeCard(
      'Agent message: {"files":[{"relativePath":"index.html","contents":"<main></main>"}]}',
    );

    assert.equal(card?.title, "index.html");
    assert.equal(card?.fileCount, 1);
    assert.deepEqual(card?.paths, ["index.html"]);
    assert.match(card?.formattedJson ?? "", /"relativePath": "index\.html"/);
  });

  it("parses fenced JSON payloads", () => {
    const card = parseThreadCodeCard(
      '```json\n{"files":[{"relativePath":"src/App.tsx","contents":"export default App"}]}\n```',
    );

    assert.equal(card?.title, "src/App.tsx");
  });

  it("ignores ordinary assistant text", () => {
    assert.equal(parseThreadCodeCard("Created local assets from the prompt envelope"), null);
  });

  it("emits key and string tokens separately for JSON highlighting", () => {
    const tokens = tokenizeJsonHighlight('{"path":"index.html","ok":true}');

    assert(tokens.some((token) => token.kind === "key" && token.text === '"path"'));
    assert(tokens.some((token) => token.kind === "string" && token.text === '"index.html"'));
    assert(tokens.some((token) => token.kind === "boolean" && token.text === "true"));
  });
});
