export const SCROLLBAR_HIT_FALLBACK_SIZE = 16;

export interface ScrollbarHitTestBox {
  clientX: number;
  clientY: number;
  rect: {
    left: number;
    right: number;
    top: number;
    bottom: number;
  };
  clientWidth: number;
  clientHeight: number;
  offsetWidth: number;
  offsetHeight: number;
  scrollWidth: number;
  scrollHeight: number;
  direction?: string;
}

export interface WindowDragPriorityState {
  isPrimaryButton: boolean;
  isTauriRuntime: boolean;
  hasDragHandle: boolean;
  hasInteractiveAncestor: boolean;
  isInsideContentBoundary: boolean;
  isInsideScrollableRegion: boolean;
  hitsScrollbar: boolean;
}

export function shouldStartWindowDrag(state: WindowDragPriorityState): boolean {
  if (!state.isPrimaryButton || !state.isTauriRuntime || !state.hasDragHandle) {
    return false;
  }

  return (
    !state.hasInteractiveAncestor &&
    !state.isInsideContentBoundary &&
    !state.isInsideScrollableRegion &&
    !state.hitsScrollbar
  );
}

export function hitsScrollableScrollbar(box: ScrollbarHitTestBox): boolean {
  const canScrollX = box.scrollWidth > box.clientWidth;
  const canScrollY = box.scrollHeight > box.clientHeight;

  if (!canScrollX && !canScrollY) {
    return false;
  }

  const isWithinVerticalBounds = box.clientY >= box.rect.top && box.clientY <= box.rect.bottom;
  const isWithinHorizontalBounds = box.clientX >= box.rect.left && box.clientX <= box.rect.right;
  const verticalScrollbarWidth = Math.max(
    box.offsetWidth - box.clientWidth,
    SCROLLBAR_HIT_FALLBACK_SIZE,
  );
  const horizontalScrollbarHeight = Math.max(
    box.offsetHeight - box.clientHeight,
    SCROLLBAR_HIT_FALLBACK_SIZE,
  );

  const hitsRightScrollbar =
    canScrollY && isWithinVerticalBounds && box.clientX >= box.rect.right - verticalScrollbarWidth;
  const hitsLeftScrollbar =
    canScrollY &&
    box.direction === "rtl" &&
    isWithinVerticalBounds &&
    box.clientX <= box.rect.left + verticalScrollbarWidth;
  const hitsBottomScrollbar =
    canScrollX &&
    isWithinHorizontalBounds &&
    box.clientY >= box.rect.bottom - horizontalScrollbarHeight;

  return hitsRightScrollbar || hitsLeftScrollbar || hitsBottomScrollbar;
}
