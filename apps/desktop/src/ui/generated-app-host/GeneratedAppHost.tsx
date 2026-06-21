import type { RuntimePreview, ShellState, WorkspaceSummary } from "../../types";
import { PreviewFrame } from "./PreviewFrame";
import { PreviewTitlebar } from "./PreviewTitlebar";
import { RuntimeStatusBoundary } from "./RuntimeStatusBoundary";

interface GeneratedAppHostProps {
  state: ShellState;
  preview: RuntimePreview | null;
  workspaces: WorkspaceSummary[];
  switchingAppId: string | null;
  error: string | null;
  onSwitchWorkspace: (workspace: WorkspaceSummary) => void;
}

export function GeneratedAppHost({
  state,
  preview,
  workspaces,
  switchingAppId,
  error,
  onSwitchWorkspace,
}: GeneratedAppHostProps) {
  return (
    <section className="generated-host" aria-label="Generated app host" data-tauri-drag-region>
      <PreviewFrame previewUrl={preview?.previewUrl ?? null} />
      {preview ? (
        <PreviewTitlebar
          activeAppId={preview.appId}
          activeName={preview.manifest.name}
          workspaces={workspaces}
          switchingAppId={switchingAppId}
          onSwitchWorkspace={onSwitchWorkspace}
        />
      ) : null}
      <RuntimeStatusBoundary state={state} error={error} />
    </section>
  );
}
