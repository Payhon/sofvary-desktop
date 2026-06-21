import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { hitsScrollableScrollbar, shouldStartWindowDrag } from "./windowDragPriority";

const draggableState = {
  isPrimaryButton: true,
  isTauriRuntime: true,
  hasDragHandle: true,
  hasInteractiveAncestor: false,
  isInsideContentBoundary: false,
  isInsideScrollableRegion: false,
  hitsScrollbar: false,
};

describe("window drag priority", () => {
  it("allows native window drag from explicit drag handles", () => {
    assert.equal(shouldStartWindowDrag(draggableState), true);
  });

  it("does not steal pointer drags from controls or content regions", () => {
    assert.equal(shouldStartWindowDrag({ ...draggableState, hasInteractiveAncestor: true }), false);
    assert.equal(shouldStartWindowDrag({ ...draggableState, isInsideContentBoundary: true }), false);
    assert.equal(shouldStartWindowDrag({ ...draggableState, isInsideScrollableRegion: true }), false);
    assert.equal(shouldStartWindowDrag({ ...draggableState, hitsScrollbar: true }), false);
  });

  it("requires a primary button, Tauri runtime, and a drag handle", () => {
    assert.equal(shouldStartWindowDrag({ ...draggableState, isPrimaryButton: false }), false);
    assert.equal(shouldStartWindowDrag({ ...draggableState, isTauriRuntime: false }), false);
    assert.equal(shouldStartWindowDrag({ ...draggableState, hasDragHandle: false }), false);
  });
});

describe("scrollbar hit testing", () => {
  it("detects native vertical scrollbar gutter drags", () => {
    assert.equal(
      hitsScrollableScrollbar({
        clientX: 294,
        clientY: 80,
        rect: { left: 0, right: 300, top: 0, bottom: 200 },
        clientWidth: 284,
        clientHeight: 200,
        offsetWidth: 300,
        offsetHeight: 200,
        scrollWidth: 284,
        scrollHeight: 600,
      }),
      true,
    );
  });

  it("does not treat regular scrollable content as scrollbar chrome", () => {
    assert.equal(
      hitsScrollableScrollbar({
        clientX: 120,
        clientY: 80,
        rect: { left: 0, right: 300, top: 0, bottom: 200 },
        clientWidth: 284,
        clientHeight: 200,
        offsetWidth: 300,
        offsetHeight: 200,
        scrollWidth: 284,
        scrollHeight: 600,
      }),
      false,
    );
  });

  it("reserves an edge band for overlay scrollbars", () => {
    assert.equal(
      hitsScrollableScrollbar({
        clientX: 292,
        clientY: 80,
        rect: { left: 0, right: 300, top: 0, bottom: 200 },
        clientWidth: 300,
        clientHeight: 200,
        offsetWidth: 300,
        offsetHeight: 200,
        scrollWidth: 300,
        scrollHeight: 600,
      }),
      true,
    );
  });

  it("detects horizontal and rtl scrollbar gutters", () => {
    assert.equal(
      hitsScrollableScrollbar({
        clientX: 140,
        clientY: 194,
        rect: { left: 0, right: 300, top: 0, bottom: 200 },
        clientWidth: 300,
        clientHeight: 184,
        offsetWidth: 300,
        offsetHeight: 200,
        scrollWidth: 640,
        scrollHeight: 184,
      }),
      true,
    );

    assert.equal(
      hitsScrollableScrollbar({
        clientX: 8,
        clientY: 80,
        rect: { left: 0, right: 300, top: 0, bottom: 200 },
        clientWidth: 284,
        clientHeight: 200,
        offsetWidth: 300,
        offsetHeight: 200,
        scrollWidth: 284,
        scrollHeight: 600,
        direction: "rtl",
      }),
      true,
    );
  });
});
