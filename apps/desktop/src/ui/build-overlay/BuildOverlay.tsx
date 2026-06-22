import { MessageSquare } from "lucide-react";
import type { ShellState } from "../../types";
import type { BuildOverlayViewModel } from "../../core/buildThreads/buildThreadLogic";
import { showCommandWindow } from "../../platform/shellClient";
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
      <BuildWaitDialog activity={activity} state={state} />
      <FloatingSofvaryGlyph />
    </div>
  );
}

interface BuildWaitDialogProps {
  state: ShellState;
  activity?: BuildOverlayViewModel | null;
}

function BuildWaitDialog({ state, activity }: BuildWaitDialogProps) {
  const { t } = useDesktopLocale();
  const phase =
    activity?.phase ?? (state === "Planning" ? t("build.overlay.planning") : t("build.overlay.building"));

  return (
    <section className="build-wait-dialog" aria-label={t("build.overlay.eyebrow")} data-tauri-drag-region>
      <div className="build-wait-dialog__visual" data-tauri-drag-region>
        <ConstructionAnimation />
      </div>
      <div className="build-wait-dialog__content" data-tauri-drag-region>
        <div className="build-wait-dialog__header">
          <span>{t("build.overlay.eyebrow")}</span>
          {activity?.eventLabel ? <code>{activity.eventLabel}</code> : null}
        </div>
        <div className="build-wait-dialog__copy">
          <strong>{activity?.title ?? t("build.overlay.creating")}</strong>
          <p>{phase}</p>
          {activity?.detail ? <small>{activity.detail}</small> : null}
        </div>
        {activity?.steps?.length ? (
          <ol className="build-wait-dialog__steps" aria-label={t("build.overlay.steps")}>
            {activity.steps.map((step) => (
              <li key={step.id} data-state={step.state}>
                <span aria-hidden="true" />
                <em>{step.label}</em>
              </li>
            ))}
          </ol>
        ) : null}
        <button
          type="button"
          className="build-wait-dialog__open"
          data-no-drag
          onPointerDown={(event) => event.stopPropagation()}
          onClick={() => {
            void showCommandWindow();
          }}
        >
          <MessageSquare size={14} aria-hidden="true" />
          {activity?.actionLabel ?? t("build.overlay.openSession")}
        </button>
      </div>
    </section>
  );
}
