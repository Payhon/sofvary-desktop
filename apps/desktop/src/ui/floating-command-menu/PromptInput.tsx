import { useDesktopLocale } from "../i18n/DesktopLocaleProvider";

interface PromptInputProps {
  value: string;
  disabled: boolean;
  rows?: number;
  placeholder?: string;
  onChange: (value: string) => void;
}

export function PromptInput({
  value,
  disabled,
  rows = 5,
  placeholder,
  onChange,
}: PromptInputProps) {
  const { t } = useDesktopLocale();
  return (
    <textarea
      className="prompt-input"
      value={value}
      disabled={disabled}
      rows={rows}
      placeholder={placeholder ?? t("prompt.placeholderCreate")}
      onChange={(event) => onChange(event.target.value)}
    />
  );
}
