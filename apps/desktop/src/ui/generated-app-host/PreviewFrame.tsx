import { useEffect, useMemo, useRef, useState } from "react";
import type { PlatformBootstrap } from "../../types";
import { safeInvoke } from "../../platform/tauriClient";
import { useWindowDrag } from "../../platform/useWindowDrag";
import { showCommandWindow } from "../../platform/shellClient";
import stealthEmptyGuide from "../../assets/stealth-empty-guide.png";
import { useDesktopLocale } from "../i18n/DesktopLocaleProvider";
import { GENERATED_APP_IFRAME_SECURITY_PROPS } from "./previewFrameSecurity";
import {
  PREVIEW_WATCHDOG_INTERVAL_MS,
  evaluatePreviewWatchdogDrift,
} from "./previewWatchdog";

interface PreviewFrameProps {
  previewUrl: string | null;
}

export function PreviewFrame({ previewUrl }: PreviewFrameProps) {
  const { t } = useDesktopLocale();
  const startWindowDrag = useWindowDrag("main");
  const [shortcutLabel, setShortcutLabel] = useState("Alt+A+I");
  const [isPreviewSuspended, setPreviewSuspended] = useState(false);
  const lastWatchdogTickRef = useRef(0);
  const watchdogHitCountRef = useRef(0);
  const shortcutKeys = useMemo(() => parseShortcutKeys(shortcutLabel), [shortcutLabel]);
  const openStealthUi = () => {
    void showCommandWindow().catch(() => {
      // Browser-only Vite sessions cannot open native Tauri windows.
    });
  };
  const resumePreview = () => {
    watchdogHitCountRef.current = 0;
    lastWatchdogTickRef.current = performance.now();
    setPreviewSuspended(false);
  };

  useEffect(() => {
    safeInvoke<PlatformBootstrap>("bootstrap_platform")
      .then((bootstrap) => setShortcutLabel(bootstrap.shortcut))
      .catch(() => {
        // Browser-only Vite sessions use the Windows/Linux label fallback.
      });
  }, []);

  useEffect(() => {
    watchdogHitCountRef.current = 0;
    lastWatchdogTickRef.current = performance.now();
    setPreviewSuspended(false);
  }, [previewUrl]);

  useEffect(() => {
    if (!previewUrl || isPreviewSuspended) {
      return undefined;
    }

    lastWatchdogTickRef.current = performance.now();
    const timer = window.setInterval(() => {
      const now = performance.now();
      const driftMs = now - lastWatchdogTickRef.current - PREVIEW_WATCHDOG_INTERVAL_MS;
      lastWatchdogTickRef.current = now;
      const decision = evaluatePreviewWatchdogDrift(watchdogHitCountRef.current, driftMs);
      watchdogHitCountRef.current = decision.hitCount;
      if (decision.shouldSuspend) {
        setPreviewSuspended(true);
      }
    }, PREVIEW_WATCHDOG_INTERVAL_MS);

    return () => window.clearInterval(timer);
  }, [isPreviewSuspended, previewUrl]);

  if (!previewUrl) {
    return (
      <div
        className="empty-preview"
        aria-label={t("preview.empty.aria", {}, "Generated app host empty state")}
        data-tauri-drag-region
        onPointerDownCapture={startWindowDrag}
      >
        <div className="empty-preview__stage" data-tauri-drag-region>
          <div className="empty-preview__art" data-tauri-drag-region>
            <img
              className="empty-preview__image"
              src={stealthEmptyGuide}
              alt=""
              draggable={false}
              data-tauri-drag-region
            />
          </div>
          <div className="empty-preview__guide" data-no-drag>
            <p className="empty-preview__eyebrow">{t("preview.empty.eyebrow")}</p>
            <h1>{t("preview.empty.title")}</h1>
            <p>{t("preview.empty.copy")}</p>
            <div className="empty-preview__shortcut" aria-label={shortcutKeys.join(" ")}>
              {shortcutKeys.map((key, index) => (
                <ShortcutKey key={`${key}-${index}`} label={key} showSeparator={index > 0} />
              ))}
            </div>
            <button type="button" onClick={openStealthUi} data-no-drag>
              {t("preview.empty.open")}
            </button>
          </div>
        </div>
      </div>
    );
  }

  if (isPreviewSuspended) {
    return (
      <div className="preview-guard" role="status" aria-label={t("preview.guard.aria")}>
        <div className="preview-guard__panel" data-no-drag>
          <p className="preview-guard__eyebrow">{t("preview.guard.eyebrow")}</p>
          <h2>{t("preview.guard.title")}</h2>
          <p>{t("preview.guard.copy")}</p>
          <div className="preview-guard__actions">
            <button type="button" onClick={resumePreview} data-no-drag>
              {t("preview.guard.resume")}
            </button>
            <button type="button" className="is-secondary" onClick={openStealthUi} data-no-drag>
              {t("preview.guard.open")}
            </button>
          </div>
        </div>
      </div>
    );
  }

  return (
    <iframe
      className="preview-frame"
      title="Generated app preview"
      src={previewUrl}
      {...GENERATED_APP_IFRAME_SECURITY_PROPS}
    />
  );
}

interface ShortcutKeyProps {
  label: string;
  showSeparator: boolean;
}

function ShortcutKey({ label, showSeparator }: ShortcutKeyProps) {
  return (
    <>
      {showSeparator ? <span>+</span> : null}
      <kbd>{label}</kbd>
    </>
  );
}

function parseShortcutKeys(shortcut: string): string[] {
  const keys = shortcut
    .split("+")
    .map((key) => key.trim())
    .filter(Boolean);

  return keys.length > 0 ? keys : ["Alt", "A", "I"];
}
