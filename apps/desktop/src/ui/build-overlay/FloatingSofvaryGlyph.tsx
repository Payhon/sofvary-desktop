import { showCommandWindow } from "../../platform/shellClient";
import { SofvaryBrandMark } from "../brand/SofvaryBrandMark";
import { useDesktopLocale } from "../i18n/DesktopLocaleProvider";

export function FloatingSofvaryGlyph() {
  const { t } = useDesktopLocale();
  const openStealthUi = (event: React.MouseEvent<HTMLButtonElement>) => {
    event.stopPropagation();
    void showCommandWindow().catch(() => {
      // Browser-only Vite sessions cannot open native Tauri windows.
    });
  };

  return (
    <button
      className="floating-glyph"
      type="button"
      aria-label={t("glyph.open")}
      title={t("glyph.open")}
      onPointerDown={(event) => event.stopPropagation()}
      onClick={openStealthUi}
      data-no-drag
    >
      <SofvaryBrandMark className="floating-glyph__mark" />
    </button>
  );
}
