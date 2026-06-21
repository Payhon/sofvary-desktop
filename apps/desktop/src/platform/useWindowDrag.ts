import { useCallback } from "react";
import { isTauriRuntime } from "./tauriClient";
import { snapCommandWindow } from "./shellClient";
import { startCurrentWindowDrag } from "./windowClient";
import {
  hitsScrollableScrollbar,
  shouldStartWindowDrag,
  type WindowDragPriorityState,
} from "./windowDragPriority";

const interactiveSelector = [
  "button",
  "textarea",
  "input",
  "select",
  "option",
  "a",
  "iframe",
  "label",
  "summary",
  "[contenteditable='true']",
  "[contenteditable]",
  "[role='button']",
  "[role='menuitem']",
  "[role='slider']",
  "[role='scrollbar']",
  "[data-no-drag]",
  "[data-no-window-drag]",
  "[data-prevent-window-drag]",
].join(", ");

const dragHandleSelector = [
  "[data-window-drag-handle]",
  "[data-tauri-drag-region]",
  ".command-titlebar",
  ".feature-heading",
  ".preview-titlebar",
  ".build-overlay",
  ".build-overlay__panel",
  ".empty-preview",
  ".empty-preview__canvas",
  ".empty-preview__bar",
  ".empty-preview__grid",
  ".floating-glyph-window",
].join(", ");

const contentBoundarySelector = [
  "[data-window-drag-boundary]",
  "[data-scroll-area]",
  ".shell-nav",
  ".command-feature",
  ".create-panel",
  ".settings-panel",
  ".agent-selector",
  ".agent-settings",
  ".workspace-list",
  ".workspace-list__rows",
  ".workspace-row",
  ".pack-list",
  ".pack-list__rows",
  ".pack-row",
  ".deep-link-panel",
  ".prompt-envelope-summary",
  ".shell-status-grid",
  ".runtime-selector",
  ".runtime-environment-card",
  ".runtime-environment-tools",
  ".runtime-environment-actions",
  ".panel-stack",
  ".settings-row",
  ".theme-choice",
  ".agent-card-grid",
  ".agent-card",
  ".agent-settings__list",
  ".agent-row",
  ".llm-provider-form",
  ".llm-provider-form__actions",
  ".build-thread-panel",
  ".thread-list",
  ".thread-detail",
  ".thread-entry-list",
  ".thread-entry",
  ".thread-code-card",
  ".thread-code-card__body",
  ".preview-switcher",
  "pre",
  "code",
].join(", ");

interface PointerDragEvent {
  button: number;
  clientX: number;
  clientY: number;
}

export function useWindowDrag(label: "main" | "command" | "glyph") {
  return useCallback(
    (event: React.PointerEvent<HTMLElement>) => {
      const tauriRuntime = isTauriRuntime();
      if (event.button !== 0 || !tauriRuntime) {
        return;
      }

      const target = event.target;
      if (!(target instanceof Element)) {
        return;
      }

      const priority = getWindowDragPriorityState(target, event, tauriRuntime);
      if (!shouldStartWindowDrag(priority)) {
        return;
      }

      event.preventDefault();
      void startCurrentWindowDrag(label);

      if (label === "command") {
        window.setTimeout(() => void snapCommandWindow(), 320);
        window.setTimeout(() => void snapCommandWindow(), 900);
      }
    },
    [label],
  );
}

function getWindowDragPriorityState(
  target: Element,
  event: PointerDragEvent,
  tauriRuntime: boolean,
): WindowDragPriorityState {
  const dragHandle = target.closest(dragHandleSelector);
  const contentBoundary = target.closest(contentBoundarySelector);
  const scrollPriority = getScrollableRegionPriority(target, dragHandle, event);

  return {
    isPrimaryButton: event.button === 0,
    isTauriRuntime: tauriRuntime,
    hasDragHandle: Boolean(dragHandle),
    hasInteractiveAncestor: Boolean(target.closest(interactiveSelector)),
    isInsideContentBoundary: isContentBoundaryBlockingDrag(contentBoundary, dragHandle),
    isInsideScrollableRegion: scrollPriority.isInsideScrollableRegion,
    hitsScrollbar: scrollPriority.hitsScrollbar,
  };
}

function isContentBoundaryBlockingDrag(contentBoundary: Element | null, dragHandle: Element | null) {
  if (!contentBoundary) {
    return false;
  }

  if (!dragHandle) {
    return true;
  }

  return !contentBoundary.contains(dragHandle);
}

function getScrollableRegionPriority(
  target: Element,
  dragHandle: Element | null,
  event: PointerDragEvent,
) {
  for (let element: Element | null = target; element; element = element.parentElement) {
    if (!(element instanceof HTMLElement) || !isScrollableElement(element)) {
      continue;
    }

    const rect = element.getBoundingClientRect();
    const hitsScrollbar = hitsScrollableScrollbar({
      clientX: event.clientX,
      clientY: event.clientY,
      rect,
      clientWidth: element.clientWidth,
      clientHeight: element.clientHeight,
      offsetWidth: element.offsetWidth,
      offsetHeight: element.offsetHeight,
      scrollWidth: element.scrollWidth,
      scrollHeight: element.scrollHeight,
      direction: window.getComputedStyle(element).direction,
    });

    if (hitsScrollbar || !dragHandle || !element.contains(dragHandle)) {
      return {
        isInsideScrollableRegion: true,
        hitsScrollbar,
      };
    }
  }

  return {
    isInsideScrollableRegion: false,
    hitsScrollbar: false,
  };
}

function isScrollableElement(element: HTMLElement) {
  const style = window.getComputedStyle(element);
  const overflowX = style.overflowX || style.overflow;
  const overflowY = style.overflowY || style.overflow;
  const canScrollX = hasScrollableOverflow(overflowX) && element.scrollWidth > element.clientWidth;
  const canScrollY = hasScrollableOverflow(overflowY) && element.scrollHeight > element.clientHeight;
  return canScrollX || canScrollY;
}

function hasScrollableOverflow(value: string) {
  return value === "auto" || value === "scroll" || value === "overlay";
}
