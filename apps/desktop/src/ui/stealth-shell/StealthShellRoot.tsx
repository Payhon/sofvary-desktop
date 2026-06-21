import { useCallback, useEffect, useRef, useState } from "react";
import {
  BuildThreadEventBatcher,
  type BuildThreadEventBatch,
} from "../../core/buildThreads/buildThreadEventBatcher";
import {
  getBuildOverlayViewModel,
  isTerminalBuildThreadStatus,
} from "../../core/buildThreads/buildThreadLogic";
import { useUiAppearance } from "../../core/uiSettings/uiSettingsClient";
import { listWorkspaces, previewWorkspace } from "../../core/workspace/workspaceClient";
import { listenShellEvent } from "../../platform/eventClient";
import { useWindowDrag } from "../../platform/useWindowDrag";
import type {
  BuildThreadEntry,
  BuildThreadSummary,
  RuntimePreview,
  ShellState,
  WorkspaceSummary,
} from "../../types";
import { BuildOverlay } from "../build-overlay/BuildOverlay";
import { GeneratedAppHost } from "../generated-app-host/GeneratedAppHost";
import { useDesktopLocale } from "../i18n/DesktopLocaleProvider";

export function StealthShellRoot() {
  const [shellState, setShellState] = useState<ShellState>("BackgroundIdle");
  const [preview, setPreview] = useState<RuntimePreview | null>(null);
  const [workspaces, setWorkspaces] = useState<WorkspaceSummary[]>([]);
  const [activeBuildThread, setActiveBuildThread] = useState<BuildThreadSummary | null>(null);
  const [latestBuildEntry, setLatestBuildEntry] = useState<BuildThreadEntry | null>(null);
  const [switchingAppId, setSwitchingAppId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const activeBuildThreadIdRef = useRef<string | null>(null);
  const latestBuildEntryIdRef = useRef<string | null>(null);
  const shellStateRef = useRef<ShellState>("BackgroundIdle");
  const startWindowDrag = useWindowDrag("main");
  const uiAppearance = useUiAppearance();
  const { t } = useDesktopLocale();

  const refreshWorkspaces = useCallback(() => {
    listWorkspaces()
      .then(setWorkspaces)
      .catch(() => setWorkspaces([]));
  }, []);

  const clearBuildOverlayActivity = useCallback(() => {
    activeBuildThreadIdRef.current = null;
    latestBuildEntryIdRef.current = null;
    setActiveBuildThread(null);
    setLatestBuildEntry(null);
  }, []);

  const applyBuildThreadBatch = useCallback((batch: BuildThreadEventBatch) => {
    for (const summary of batch.summaries) {
      if (isLiveBuildThread(summary)) {
        activeBuildThreadIdRef.current = summary.id;
        setActiveBuildThread(summary);
        continue;
      }

      if (
        activeBuildThreadIdRef.current === summary.id &&
        isTerminalBuildThreadStatus(summary.status)
      ) {
        clearBuildOverlayActivity();
        if (
          summary.status === "canceled" &&
          (shellStateRef.current === "Planning" || shellStateRef.current === "Building")
        ) {
          shellStateRef.current = "BackgroundIdle";
          setShellState("BackgroundIdle");
        }
      }
    }

    for (const entry of batch.entries) {
      const activeId = activeBuildThreadIdRef.current;
      if (activeId && activeId !== entry.threadId) {
        continue;
      }
      if (latestBuildEntryIdRef.current === entry.id) {
        continue;
      }
      latestBuildEntryIdRef.current = entry.id;
      setLatestBuildEntry(entry);
    }
  }, [clearBuildOverlayActivity]);

  useEffect(() => {
    const batcher = new BuildThreadEventBatcher(applyBuildThreadBatch);
    const unlisteners = [
      listenShellEvent<ShellState>("sofvary-build-state", (payload) => {
        shellStateRef.current = payload;
        setShellState(payload);
        if (payload !== "Error") {
          setError(null);
        }
        if (payload === "Planning") {
          clearBuildOverlayActivity();
        }
      }),
      listenShellEvent<RuntimePreview>("sofvary-runtime-preview", (payload) => {
        setPreview(payload);
        shellStateRef.current = "Previewing";
        setShellState("Previewing");
        setError(null);
        clearBuildOverlayActivity();
        refreshWorkspaces();
      }),
      listenShellEvent<string>("sofvary-runtime-error", (payload) => {
        setError(payload);
        shellStateRef.current = "Error";
        setShellState("Error");
        clearBuildOverlayActivity();
      }),
      listenShellEvent<BuildThreadSummary>("sofvary-build-thread-updated", (payload) => {
        batcher.pushSummary(payload);
      }),
      listenShellEvent<BuildThreadEntry>("sofvary-build-thread-entry", (payload) => {
        batcher.pushEntry(payload);
      }),
    ];

    return () => {
      batcher.dispose();
      void Promise.all(unlisteners).then((listeners) => listeners.forEach((unlisten) => unlisten()));
    };
  }, [applyBuildThreadBatch, clearBuildOverlayActivity, refreshWorkspaces]);

  useEffect(() => {
    refreshWorkspaces();
  }, [refreshWorkspaces]);

  const switchPreviewWorkspace = useCallback(async (workspace: WorkspaceSummary) => {
    setSwitchingAppId(workspace.appId);
    shellStateRef.current = "Previewing";
    setShellState("Previewing");
    setError(null);
    try {
      const nextPreview = await previewWorkspace(workspace, "dev");
      setPreview(nextPreview);
      refreshWorkspaces();
    } catch (switchError) {
      setError(switchError instanceof Error ? switchError.message : String(switchError));
      shellStateRef.current = "Error";
      setShellState("Error");
    } finally {
      setSwitchingAppId(null);
    }
  }, [refreshWorkspaces]);

  return (
    <main
      className="stealth-shell host-shell"
      data-state={shellState}
      data-theme={uiAppearance.resolvedTheme}
      data-theme-preference={uiAppearance.preference}
      data-tauri-drag-region
      style={uiAppearance.cssVariables}
      {...uiAppearance.dataAttributes}
    >
      <GeneratedAppHost
        state={shellState}
        preview={preview}
        workspaces={workspaces}
        switchingAppId={switchingAppId}
        error={error}
        onSwitchWorkspace={(workspace) => void switchPreviewWorkspace(workspace)}
      />
      {preview ? (
        <div
          className="host-drag-strip"
          data-tauri-drag-region
          onPointerDownCapture={startWindowDrag}
        />
      ) : null}
      <BuildOverlay
        state={shellState}
        activity={getBuildOverlayViewModel(shellState, activeBuildThread, latestBuildEntry, t)}
      />
    </main>
  );
}

function isLiveBuildThread(thread: BuildThreadSummary): boolean {
  return (
    thread.status === "queued" ||
    thread.status === "planning" ||
    thread.status === "building" ||
    thread.status === "previewing"
  );
}
