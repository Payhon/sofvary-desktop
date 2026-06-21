import { useEffect, useRef, useState } from "react";
import { hideWindow, minimizeWindow, toggleWindowMaximize } from "../../platform/windowClient";
import { useWindowDrag } from "../../platform/useWindowDrag";
import type { WorkspaceSummary } from "../../types";

interface PreviewTitlebarProps {
  activeAppId: string | null;
  activeName: string | null;
  workspaces: WorkspaceSummary[];
  switchingAppId: string | null;
  onSwitchWorkspace: (workspace: WorkspaceSummary) => void;
}

export function PreviewTitlebar({
  activeAppId,
  activeName,
  workspaces,
  switchingAppId,
  onSwitchWorkspace,
}: PreviewTitlebarProps) {
  const [isMenuOpen, setMenuOpen] = useState(false);
  const menuRef = useRef<HTMLDivElement | null>(null);
  const startWindowDrag = useWindowDrag("main");

  useEffect(() => {
    if (!isMenuOpen) return;

    const onPointerDown = (event: PointerEvent) => {
      if (event.target instanceof Node && menuRef.current?.contains(event.target)) {
        return;
      }
      setMenuOpen(false);
    };

    window.addEventListener("pointerdown", onPointerDown);
    return () => window.removeEventListener("pointerdown", onPointerDown);
  }, [isMenuOpen]);

  return (
    <header
      className="preview-titlebar"
      aria-label="Generated app window controls"
      data-tauri-drag-region
      onPointerDownCapture={startWindowDrag}
    >
      <div className="preview-titlebar__menu" ref={menuRef} data-no-drag>
        <button
          className="preview-titlebar__button"
          type="button"
          aria-expanded={isMenuOpen}
          aria-label="Switch preview app"
          title="Switch preview app"
          onClick={() => setMenuOpen((current) => !current)}
        >
          ☰
        </button>
        {isMenuOpen ? (
          <div className="preview-switcher" role="menu" aria-label="Preview another app">
            <div className="preview-switcher__current">
              <span>Previewing</span>
              <strong>{activeName ?? "Generated app"}</strong>
            </div>
            {workspaces.length > 0 ? (
              <div className="preview-switcher__list">
                {workspaces.map((workspace) => {
                  const isActive = workspace.appId === activeAppId;
                  const isSwitching = workspace.appId === switchingAppId;

                  return (
                    <button
                      key={workspace.appId}
                      type="button"
                      role="menuitem"
                      className={isActive ? "is-active" : ""}
                      disabled={isSwitching}
                      onClick={() => {
                        setMenuOpen(false);
                        onSwitchWorkspace(workspace);
                      }}
                    >
                      <span>{workspace.name}</span>
                      <small>{isSwitching ? "Opening..." : workspace.mode}</small>
                    </button>
                  );
                })}
              </div>
            ) : (
              <p>No local apps yet.</p>
            )}
          </div>
        ) : null}
      </div>
      <div className="preview-titlebar__controls" data-no-drag>
        <button
          className="preview-titlebar__button"
          type="button"
          aria-label="Minimize preview window"
          title="Minimize"
          onClick={() => void minimizeWindow("main").catch(() => {})}
        >
          -
        </button>
        <button
          className="preview-titlebar__button"
          type="button"
          aria-label="Maximize or restore preview window"
          title="Maximize"
          onClick={() => void toggleWindowMaximize("main").catch(() => {})}
        >
          □
        </button>
        <button
          className="preview-titlebar__button preview-titlebar__button--close"
          type="button"
          aria-label="Close preview window"
          title="Close"
          onClick={() => void hideWindow("main").catch(() => {})}
        >
          ×
        </button>
      </div>
    </header>
  );
}
