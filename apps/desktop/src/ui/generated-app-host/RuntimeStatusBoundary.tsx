import type { ShellState } from "../../types";

interface RuntimeStatusBoundaryProps {
  state: ShellState;
  error: string | null;
}

export function RuntimeStatusBoundary({ state, error }: RuntimeStatusBoundaryProps) {
  if (state !== "Error" || !error) {
    return null;
  }
  const displayError = formatRuntimeError(error);

  return (
    <div className="runtime-error" role="alert">
      <strong>Runtime Error</strong>
      <span>{displayError}</span>
    </div>
  );
}

function formatRuntimeError(error: string): string {
  const trimmed = error.trim();
  if (trimmed.length <= 720) {
    return trimmed;
  }
  return `${trimmed.slice(0, 720)}...`;
}
