import { useRef } from "react";
import { useUiAppearance } from "../../core/uiSettings/uiSettingsClient";
import { showCommandWindow } from "../../platform/shellClient";
import { startCurrentWindowDrag } from "../../platform/windowClient";
import { SofvaryBrandMark } from "../brand/SofvaryBrandMark";
import { useDesktopLocale } from "../i18n/DesktopLocaleProvider";

interface PointerStart {
  x: number;
  y: number;
  dragging: boolean;
}

export function FloatingGlyphRoot() {
  const pointerStart = useRef<PointerStart | null>(null);
  const draggedDuringPointer = useRef(false);
  const uiAppearance = useUiAppearance();
  const { t } = useDesktopLocale();

  const onPointerDown = (event: React.PointerEvent<HTMLElement>) => {
    if (event.button !== 0) return;
    pointerStart.current = { x: event.clientX, y: event.clientY, dragging: false };
    draggedDuringPointer.current = false;
  };

  const onPointerMove = (event: React.PointerEvent<HTMLElement>) => {
    const start = pointerStart.current;
    if (!start || start.dragging) return;

    const distance = Math.hypot(event.clientX - start.x, event.clientY - start.y);
    if (distance > 4) {
      start.dragging = true;
      draggedDuringPointer.current = true;
      void startCurrentWindowDrag("glyph");
    }
  };

  const onPointerUp = () => {
    pointerStart.current = null;
  };

  const onClick = () => {
    if (!draggedDuringPointer.current) {
      void showCommandWindow();
    }
    pointerStart.current = null;
    draggedDuringPointer.current = false;
  };

  return (
    <main
      className="floating-glyph-window"
      aria-label={t("glyph.aria", {}, "Sofvary floating glyph")}
      data-theme={uiAppearance.resolvedTheme}
      data-theme-preference={uiAppearance.preference}
      style={uiAppearance.cssVariables}
      {...uiAppearance.dataAttributes}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerUp}
      onContextMenu={(event) => {
        event.preventDefault();
        // Reserved for the upcoming glyph context menu.
      }}
    >
      <button type="button" aria-label={t("glyph.open")} onClick={onClick}>
        <SofvaryBrandMark className="floating-glyph-window__mark" />
      </button>
    </main>
  );
}
