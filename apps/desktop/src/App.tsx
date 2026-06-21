import { useCallback } from "react";
import { showCommandWindow } from "./platform/shellClient";
import { getCurrentWindowLabel } from "./platform/windowClient";
import { CommandWindowRoot } from "./ui/floating-command-menu/CommandWindowRoot";
import { FloatingGlyphRoot } from "./ui/floating-glyph/FloatingGlyphRoot";
import { DesktopLocaleProvider } from "./ui/i18n/DesktopLocaleProvider";
import { StealthShellRoot } from "./ui/stealth-shell/StealthShellRoot";
import { useGlobalShortcutState } from "./ui/stealth-shell/useGlobalShortcutState";

export function App() {
  const label = getCurrentWindowLabel("main");
  const showCommand = useCallback(() => {
    void showCommandWindow().catch(() => {
      // Browser-only Vite sessions cannot open native Tauri windows.
    });
  }, []);

  useGlobalShortcutState(showCommand);

  if (label === "command") {
    return (
      <DesktopLocaleProvider>
        <CommandWindowRoot />
      </DesktopLocaleProvider>
    );
  }

  if (label === "glyph") {
    return (
      <DesktopLocaleProvider>
        <FloatingGlyphRoot />
      </DesktopLocaleProvider>
    );
  }

  return (
    <DesktopLocaleProvider>
      <StealthShellRoot />
    </DesktopLocaleProvider>
  );
}
