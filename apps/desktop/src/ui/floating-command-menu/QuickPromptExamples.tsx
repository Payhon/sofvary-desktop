import { useDesktopLocale } from "../i18n/DesktopLocaleProvider";

const exampleKeys = ["quickPrompt.countdown", "quickPrompt.crm", "quickPrompt.cleaner"] as const;

interface QuickPromptExamplesProps {
  disabled: boolean;
  onPick: (value: string) => void;
}

export function QuickPromptExamples({ disabled, onPick }: QuickPromptExamplesProps) {
  const { t } = useDesktopLocale();
  return (
    <div className="quick-prompts">
      {exampleKeys.map((key) => {
        const example = t(key);
        return (
        <button key={example} type="button" disabled={disabled} onClick={() => onPick(example)}>
          {example}
        </button>
        );
      })}
    </div>
  );
}
