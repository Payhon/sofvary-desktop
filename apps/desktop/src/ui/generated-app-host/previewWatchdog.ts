export const PREVIEW_WATCHDOG_INTERVAL_MS = 1_000;
export const PREVIEW_WATCHDOG_LONG_TASK_MS = 2_000;
export const PREVIEW_WATCHDOG_SEVERE_TASK_MS = 5_000;
export const PREVIEW_WATCHDOG_MAX_HITS = 2;

export interface PreviewWatchdogOptions {
  longTaskMs?: number;
  severeTaskMs?: number;
  maxHits?: number;
}

export interface PreviewWatchdogDecision {
  hitCount: number;
  shouldSuspend: boolean;
}

export function evaluatePreviewWatchdogDrift(
  previousHitCount: number,
  driftMs: number,
  options: PreviewWatchdogOptions = {},
): PreviewWatchdogDecision {
  const longTaskMs = options.longTaskMs ?? PREVIEW_WATCHDOG_LONG_TASK_MS;
  const severeTaskMs = options.severeTaskMs ?? PREVIEW_WATCHDOG_SEVERE_TASK_MS;
  const maxHits = options.maxHits ?? PREVIEW_WATCHDOG_MAX_HITS;

  if (!Number.isFinite(driftMs) || driftMs < longTaskMs) {
    return { hitCount: 0, shouldSuspend: false };
  }

  const hitCount = driftMs >= severeTaskMs ? maxHits : previousHitCount + 1;
  return { hitCount, shouldSuspend: hitCount >= maxHits };
}
