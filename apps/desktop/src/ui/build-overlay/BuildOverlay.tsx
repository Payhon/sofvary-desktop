import type { ShellState } from "../../types";
import type { BuildOverlayViewModel } from "../../core/buildThreads/buildThreadLogic";
import { useWindowDrag } from "../../platform/useWindowDrag";
import softwareGrowingBackground from "../../assets/software-growing-background.png";
import { useDesktopLocale } from "../i18n/DesktopLocaleProvider";
import { ConstructionAnimation } from "./ConstructionAnimation";
import { FloatingSofvaryGlyph } from "./FloatingSofvaryGlyph";

interface BuildOverlayProps {
  state: ShellState;
  activity?: BuildOverlayViewModel | null;
}

export function BuildOverlay({ state, activity }: BuildOverlayProps) {
  const { t } = useDesktopLocale();
  const startWindowDrag = useWindowDrag("main");

  if (state !== "Planning" && state !== "Building") {
    return null;
  }

  return (
    <div
      className="build-overlay"
      aria-live="polite"
      data-tauri-drag-region
      onPointerDownCapture={startWindowDrag}
    >
      <img
        className="build-overlay__background"
        src={softwareGrowingBackground}
        alt=""
        draggable={false}
        data-tauri-drag-region
      />
      <div className="build-overlay__panel" data-tauri-drag-region>
        <ConstructionAnimation />
        <div className="build-overlay__copy">
          <strong>{activity?.title ?? t("build.overlay.creating")}</strong>
          <p>{activity?.phase ?? (state === "Planning" ? t("build.overlay.planning") : t("build.overlay.building"))}</p>
          {activity?.detail ? <small>{activity.detail}</small> : null}
        </div>
      </div>
      <FloatingSofvaryGlyph />
    </div>
  );
}
