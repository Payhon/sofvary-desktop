import type { ShellState } from "../../types";

export function PreviewStateLabel({ state }: { state: ShellState }) {
  return <span className="preview-state">{state}</span>;
}
