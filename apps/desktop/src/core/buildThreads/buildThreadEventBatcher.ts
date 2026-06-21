import type { BuildThreadEntry, BuildThreadSummary } from "../../types";

export interface BuildThreadEventBatch {
  summaries: BuildThreadSummary[];
  entries: BuildThreadEntry[];
}

export class BuildThreadEventBatcher {
  private summaries = new Map<string, BuildThreadSummary>();
  private entries: BuildThreadEntry[] = [];
  private timer: ReturnType<typeof setTimeout> | null = null;

  constructor(
    private readonly flushBatch: (batch: BuildThreadEventBatch) => void,
    private readonly delayMs = 64,
  ) {}

  pushSummary(summary: BuildThreadSummary): void {
    this.summaries.set(summary.id, summary);
    this.flushSoon();
  }

  pushEntry(entry: BuildThreadEntry): void {
    this.entries.push(entry);
    this.flushSoon();
  }

  flush(): void {
    if (this.timer) {
      clearTimeout(this.timer);
      this.timer = null;
    }

    if (this.summaries.size === 0 && this.entries.length === 0) {
      return;
    }

    const batch = {
      summaries: [...this.summaries.values()],
      entries: this.entries,
    };
    this.summaries = new Map();
    this.entries = [];
    this.flushBatch(batch);
  }

  dispose(): void {
    if (this.timer) {
      clearTimeout(this.timer);
      this.timer = null;
    }
    this.summaries.clear();
    this.entries = [];
  }

  private flushSoon(): void {
    if (this.timer) {
      return;
    }

    this.timer = setTimeout(() => this.flush(), this.delayMs);
  }
}
