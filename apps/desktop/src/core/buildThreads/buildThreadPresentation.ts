import type { BuildThreadEntry, BuildThreadEntryKind } from "../../types";
import { parseThreadCodeCard } from "./threadCodeCard";

export type BuildThreadPresentationKind =
  | "user"
  | "assistant"
  | "progress"
  | "file"
  | "tool"
  | "runtime"
  | "system"
  | "error";

export type BuildThreadPresentationTone =
  | "neutral"
  | "user"
  | "success"
  | "working"
  | "warning"
  | "danger";

export interface BuildThreadPresentationDetail {
  label: string;
  value: string;
}

export interface BuildThreadPresentationItem {
  id: string;
  timestamp: string;
  sourceKind: BuildThreadEntryKind;
  kind: BuildThreadPresentationKind;
  tone: BuildThreadPresentationTone;
  icon: string;
  label: string;
  title: string;
  description: string | null;
  details: BuildThreadPresentationDetail[];
  hidesTechnicalDetail: boolean;
  technicalDetail: string | null;
  isActive: boolean;
}

interface KnownStatusCopy {
  title: string;
  description?: string | null;
  kind?: BuildThreadPresentationKind;
  tone?: BuildThreadPresentationTone;
  icon?: string;
  hidesTechnicalDetail?: boolean;
}

export function presentBuildThreadEntry(
  entry: BuildThreadEntry,
  _t?: (key: string, params?: Record<string, string | number | boolean | null | undefined>, fallback?: string) => string,
): BuildThreadPresentationItem {
  const content = normalizeContent(entry.content);
  const codeCard = parseThreadCodeCard(content);

  if (codeCard) {
    return {
      id: entry.id,
      timestamp: entry.timestamp,
      sourceKind: entry.kind,
      kind: "file",
      tone: "success",
      icon: "▣",
      label: "文件",
      title:
        codeCard.fileCount === 1
          ? `已生成 ${basename(codeCard.paths[0] ?? "文件")}`
          : `已生成 ${codeCard.fileCount} 个文件`,
      description: "代码内容已整理为文件摘要，生成完成后可直接预览。",
      details: buildFileDetails(codeCard.paths),
      hidesTechnicalDetail: true,
      technicalDetail: codeCard.formattedJson,
      isActive: false,
    };
  }

  switch (entry.kind) {
    case "user":
      return presentation(entry, {
        kind: "user",
        tone: "user",
        icon: "你",
        label: "需求",
        title: "你的需求",
        description: content,
      });
    case "assistant":
      return presentAssistantEntry(entry, content);
    case "agent-event":
      return presentAgentEvent(entry, content);
    case "file":
      return presentFileEntry(entry, content);
    case "tool":
      return presentToolEntry(entry, content);
    case "system":
      return presentSystemEntry(entry, content);
    case "error": {
      const knownRuntimeFailure = knownRuntimeFailureStatus(content);
      if (knownRuntimeFailure) {
        return presentation(entry, {
          kind: "runtime",
          tone: knownRuntimeFailure.tone ?? "warning",
          icon: knownRuntimeFailure.icon ?? "!",
          label: "运行时",
          title: knownRuntimeFailure.title,
          description: knownRuntimeFailure.description ?? null,
          details: content ? [{ label: "技术摘要", value: truncate(content, 180) }] : [],
          hidesTechnicalDetail: knownRuntimeFailure.hidesTechnicalDetail ?? true,
        });
      }
      return presentation(entry, {
        kind: "error",
        tone: "danger",
        icon: "!",
        label: "问题",
        title: "创建过程遇到问题",
        description: friendlyErrorMessage(content),
        details: content ? [{ label: "技术摘要", value: truncate(content, 180) }] : [],
        hidesTechnicalDetail: true,
      });
    }
  }
}

export function mergeBuildThreadPresentationItems(
  items: BuildThreadPresentationItem[],
): BuildThreadPresentationItem[] {
  let progressItem: BuildThreadPresentationItem | null = null;
  const merged: BuildThreadPresentationItem[] = [];

  for (const item of items) {
    if (!isMergeableProgressItem(item)) {
      merged.push(item);
      continue;
    }

    if (!progressItem) {
      progressItem = item;
      merged.push(item);
      continue;
    }

    const previousIndex = merged.findIndex((candidate) => candidate.id === progressItem?.id);
    if (previousIndex >= 0) {
      merged.splice(previousIndex, 1);
    }

    progressItem = mergeProgressItems(progressItem, item);
    merged.push(progressItem);
  }

  return merged;
}

export function summarizeBuildThreadEntryForUser(
  entry: BuildThreadEntry,
  maxLength = 180,
): string {
  const item = presentBuildThreadEntry(entry);
  if (item.kind === "assistant" && item.title === "Agent 反馈" && item.description) {
    return truncate(item.description, maxLength);
  }
  const detailText = item.details
    .slice(0, 2)
    .map((detail) => detail.value)
    .join("、");
  const text = [item.title, item.description, detailText].filter(Boolean).join(" · ");
  return truncate(text, maxLength);
}

function isMergeableProgressItem(item: BuildThreadPresentationItem): boolean {
  return item.sourceKind === "agent-event" && item.kind === "progress";
}

function mergeProgressItems(
  previous: BuildThreadPresentationItem,
  next: BuildThreadPresentationItem,
): BuildThreadPresentationItem {
  return {
    ...next,
    id: previous.id,
    details: next.details.length > 0 ? next.details : previous.details,
    hidesTechnicalDetail: previous.hidesTechnicalDetail || next.hidesTechnicalDetail,
    technicalDetail: next.technicalDetail ?? previous.technicalDetail,
    isActive: next.isActive,
  };
}

function presentAssistantEntry(
  entry: BuildThreadEntry,
  content: string,
): BuildThreadPresentationItem {
  const known = knownAssistantStatus(content);
  if (known) {
    return presentation(entry, {
      kind: "assistant",
      tone: known.tone ?? "working",
      icon: known.icon ?? "AI",
      label: "Agent",
      title: known.title,
      description: known.description ?? null,
      hidesTechnicalDetail: known.hidesTechnicalDetail ?? false,
    });
  }

  if (looksLikeTechnicalPayload(content)) {
    return presentation(entry, {
      kind: "assistant",
      tone: "working",
      icon: "AI",
      label: "Agent",
      title: "正在整理实现内容",
      description: "实现细节正在处理中，界面会在准备好后自动更新。",
      hidesTechnicalDetail: true,
    });
  }

  return presentation(entry, {
    kind: "assistant",
    tone: "neutral",
    icon: "AI",
    label: "Agent",
    title: "Agent 反馈",
    description: content || "Agent 正在继续处理。",
  });
}

function presentAgentEvent(entry: BuildThreadEntry, content: string): BuildThreadPresentationItem {
  const known = knownAgentEventStatus(content);
  if (known) {
    return presentation(entry, {
      kind: known.kind ?? "progress",
      tone: known.tone ?? "working",
      icon: known.icon ?? "◌",
      label: known.kind === "runtime" ? "运行时" : "进度",
      title: known.title,
      description: known.description ?? null,
      hidesTechnicalDetail: known.hidesTechnicalDetail ?? false,
    });
  }

  return presentation(entry, {
    kind: "progress",
    tone: "working",
    icon: "◌",
    label: "进度",
    title: "正在与 Agent 协作",
    description: localizeTechnicalStatus(content),
    hidesTechnicalDetail: containsLowLevelLog(content),
  });
}

function presentFileEntry(entry: BuildThreadEntry, content: string): BuildThreadPresentationItem {
  const requestedPath = pathAfterPrefix(content, "Agent requested file write:");
  if (requestedPath) {
    return presentation(entry, {
      kind: "file",
      tone: "working",
      icon: "□",
      label: "文件",
      title: `准备写入 ${basename(requestedPath)}`,
      description: "Agent 已给出文件变更，Sofvary 正在按工作区边界处理。",
      details: [{ label: "文件", value: requestedPath }],
      hidesTechnicalDetail: true,
    });
  }

  const writtenPath = pathAfterPrefix(content, "Workspace wrote generated file:");
  if (writtenPath) {
    return presentation(entry, {
      kind: "file",
      tone: "success",
      icon: "✓",
      label: "文件",
      title: `已写入 ${basename(writtenPath)}`,
      description: "生成文件已进入当前软件工作区。",
      details: [{ label: "文件", value: writtenPath }],
    });
  }

  return presentation(entry, {
    kind: "file",
    tone: "working",
    icon: "□",
    label: "文件",
    title: "正在处理生成文件",
    description: localizeTechnicalStatus(content),
    hidesTechnicalDetail: true,
  });
}

function presentToolEntry(entry: BuildThreadEntry, content: string): BuildThreadPresentationItem {
  const requested = pathAfterPrefix(content, "Agent requested command:");
  if (requested) {
    return presentation(entry, {
      kind: "tool",
      tone: "working",
      icon: "⌁",
      label: "工具",
      title: "Agent 请求运行本地工具",
      description: "Sofvary 正在按安全策略检查这个操作。",
      details: [{ label: "工具", value: basename(requested) }],
      hidesTechnicalDetail: true,
    });
  }

  const approved = pathAfterPrefix(content, "Command approved:");
  if (approved) {
    return presentation(entry, {
      kind: "tool",
      tone: "success",
      icon: "✓",
      label: "工具",
      title: "本地工具已通过策略检查",
      description: "允许的操作会继续在 Sofvary 的工作区边界内执行。",
      details: [{ label: "工具", value: basename(approved) }],
    });
  }

  const rejected = pathAfterPrefix(content, "Command rejected:");
  if (rejected) {
    return presentation(entry, {
      kind: "tool",
      tone: "warning",
      icon: "!",
      label: "工具",
      title: "已阻止不符合策略的操作",
      description: "Sofvary 已保留工作区安全边界，当前命令不会继续执行。",
      details: [{ label: "工具", value: basename(rejected.split(":")[0] ?? rejected) }],
      hidesTechnicalDetail: true,
    });
  }

  return presentation(entry, {
    kind: "tool",
    tone: "working",
    icon: "⌁",
    label: "工具",
    title: "正在调用工具",
    description: "工具结果正在整理，技术细节已收起。",
    hidesTechnicalDetail: true,
  });
}

function presentSystemEntry(entry: BuildThreadEntry, content: string): BuildThreadPresentationItem {
  const known = knownAgentEventStatus(content);
  if (known) {
    return presentation(entry, {
      kind: known.kind ?? "system",
      tone: known.tone ?? "neutral",
      icon: known.icon ?? "◌",
      label: "系统",
      title: known.title,
      description: known.description ?? null,
      hidesTechnicalDetail: known.hidesTechnicalDetail ?? false,
    });
  }

  return presentation(entry, {
    kind: "system",
    tone: "neutral",
    icon: "◌",
    label: "系统",
    title: "系统状态已更新",
    description: localizeTechnicalStatus(content),
    hidesTechnicalDetail: containsLowLevelLog(content),
  });
}

function presentation(
  entry: BuildThreadEntry,
  options: Omit<
    BuildThreadPresentationItem,
    | "id"
    | "timestamp"
    | "sourceKind"
    | "details"
    | "hidesTechnicalDetail"
    | "technicalDetail"
    | "isActive"
  > & {
    details?: BuildThreadPresentationDetail[];
    hidesTechnicalDetail?: boolean;
    technicalDetail?: string | null;
    isActive?: boolean;
  },
): BuildThreadPresentationItem {
  const hidesTechnicalDetail = options.hidesTechnicalDetail ?? false;
  return {
    id: entry.id,
    timestamp: entry.timestamp,
    sourceKind: entry.kind,
    kind: options.kind,
    tone: options.tone,
    icon: options.icon,
    label: options.label,
    title: options.title,
    description: options.description,
    details: options.details ?? [],
    hidesTechnicalDetail,
    technicalDetail:
      options.technicalDetail ??
      (hidesTechnicalDetail ? formatTechnicalDetail(entry.content, entry.metadata) : null),
    isActive: options.isActive ?? options.tone === "working",
  };
}

function knownAssistantStatus(content: string): KnownStatusCopy | null {
  if (/^Created local .* assets from the prompt envelope$/i.test(content)) {
    return {
      title: "已根据需求准备本地软件资源",
      description: "Sofvary 正在把 Agent 的结果整理为可预览的软件。",
      tone: "success",
    };
  }
  if (/^Working on (the )?layout\.?$/i.test(content)) {
    return {
      title: "正在整理界面布局",
      description: "Agent 正在把需求转成可用的界面结构。",
    };
  }
  if (/^Preparing constrained .* output$/i.test(content)) {
    return {
      title: "正在分析需求与运行时约束",
      description: "Sofvary 会按所选 Runtime 限制生成范围。",
    };
  }
  return null;
}

function knownRuntimeFailureStatus(content: string): KnownStatusCopy | null {
  if (/^Sofvary 已自动尝试修复 \d+ 次/.test(content)) {
    return {
      title: "自动修复未完成",
      description: content,
      kind: "runtime",
      tone: "warning",
      icon: "!",
      hidesTechnicalDetail: true,
    };
  }
  if (/^Sofvary 已完成运行诊断：/.test(content)) {
    return {
      title: "运行诊断已完成",
      description: content,
      kind: "runtime",
      tone: "warning",
      icon: "!",
      hidesTechnicalDetail: true,
    };
  }
  return null;
}

function knownAgentEventStatus(content: string): KnownStatusCopy | null {
  const knownRuntimeFailure = knownRuntimeFailureStatus(content);
  if (knownRuntimeFailure) {
    return knownRuntimeFailure;
  }

  if (/^Continuing existing app .* with .*\.?$/i.test(content)) {
    return {
      title: "已载入当前软件上下文",
      description: "Sofvary 会把已有工程状态交给 Agent，在原软件上继续修改。",
      tone: "working",
      icon: "↻",
      hidesTechnicalDetail: true,
    };
  }
  if (/^Agent session started with .* adapter$/i.test(content)) {
    return {
      title: "已连接 Agent",
      description: "正在接收生成进度，关键步骤会在这里更新。",
      icon: "AI",
    };
  }
  if (/returned generated file payload$/i.test(content)) {
    return {
      title: "已接收生成结果",
      description: "生成内容已整理为文件变更，正在写入当前工作区。",
      kind: "file",
      tone: "success",
      icon: "▣",
      hidesTechnicalDetail: true,
    };
  }
  if (/thread started$/i.test(content) || /turn started$/i.test(content)) {
    return {
      title: "正在与 Agent 协作",
      description: "正在把需求转成可运行的软件结构。",
      icon: "AI",
    };
  }
  if (/turn completed$/i.test(content)) {
    return {
      title: "本轮生成步骤已完成",
      description: "正在整理结果并进入下一步。",
      tone: "success",
      icon: "✓",
    };
  }
  if (/reasoning updated$/i.test(content)) {
    return {
      title: "正在规划实现方案",
      description: "规划细节已收起，只展示可执行进度。",
      icon: "◌",
      hidesTechnicalDetail: true,
    };
  }
  if (/^Runtime diagnostic:/i.test(content)) {
    return {
      title: "检测到运行问题",
      description: "Sofvary 已记录诊断信息，并会判断是否能自动交给 Agent 修复。",
      kind: "runtime",
      tone: "warning",
      icon: "!",
      hidesTechnicalDetail: true,
    };
  }
  if (/^Runtime repair attempt \d+\/\d+:/i.test(content)) {
    return {
      title: "正在自动修复运行问题",
      description: "Sofvary 正在把可修复的运行诊断交给 Agent，并会自动重试预览。",
      kind: "runtime",
      tone: "working",
      icon: "↻",
      hidesTechnicalDetail: true,
    };
  }
  if (/^Runtime repair attempt \d+ finished:/i.test(content)) {
    return {
      title: "修复步骤已完成",
      description: "Sofvary 正在重新启动运行时验证结果。",
      kind: "runtime",
      tone: "success",
      icon: "✓",
      hidesTechnicalDetail: true,
    };
  }
  if (/^Build started:/i.test(content)) {
    return {
      title: "开始准备运行环境",
      description: runtimeTargetDescription(content),
      kind: "runtime",
      icon: "▶",
    };
  }
  if (/^Build finished:/i.test(content)) {
    return {
      title: "运行环境准备完成",
      description: runtimeTargetDescription(content),
      kind: "runtime",
      tone: "success",
      icon: "✓",
    };
  }
  if (/^Agent session completed$/i.test(content)) {
    return {
      title: "创建流程已完成",
      description: "可以查看预览，或继续描述想调整的地方。",
      tone: "success",
      icon: "✓",
    };
  }
  if (/stderr:/i.test(content)) {
    return {
      title: "检测到运行提示",
      description: "提示内容已转为安全进度信息，创建流程会继续推进。",
      tone: "warning",
      icon: "!",
      hidesTechnicalDetail: true,
    };
  }
  return null;
}

function localizeTechnicalStatus(content: string): string | null {
  if (!content) return null;
  if (containsLowLevelLog(content)) {
    return "技术细节已收起，这里只显示创建进度。";
  }
  return content;
}

function friendlyErrorMessage(content: string): string {
  if (!content) {
    return "Sofvary 已停止当前步骤，可以调整需求后继续。";
  }
  return `Sofvary 已停止当前步骤。${truncate(content, 140)}`;
}

function normalizeContent(content: string): string {
  return content
    .replace(/^Agent message:\s*/i, "")
    .replace(/^Agent plan:\s*/i, "")
    .split(/\s+/)
    .join(" ")
    .trim();
}

function buildFileDetails(paths: string[]): BuildThreadPresentationDetail[] {
  if (paths.length === 0) return [];
  const shown = paths.slice(0, 6).map((path) => ({ label: "文件", value: path }));
  if (paths.length > shown.length) {
    shown.push({ label: "更多", value: `还有 ${paths.length - shown.length} 个文件` });
  }
  return shown;
}

function pathAfterPrefix(content: string, prefix: string): string | null {
  if (!content.toLowerCase().startsWith(prefix.toLowerCase())) return null;
  const value = content.slice(prefix.length).trim();
  return value || null;
}

function basename(path: string): string {
  const normalized = path.replace(/\\/g, "/");
  return normalized.split("/").filter(Boolean).pop() ?? path;
}

function runtimeTargetDescription(content: string): string | null {
  const target = content.split(":").slice(1).join(":").trim();
  if (!target) return null;
  return `目标运行时：${target}`;
}

function containsLowLevelLog(content: string): boolean {
  return /stderr:|stdout:|stack|trace|payload|json|exception|error:/i.test(content);
}

function looksLikeTechnicalPayload(content: string): boolean {
  if (content.length > 240 && /[{}[\]<>]/.test(content)) return true;
  return (
    /```/.test(content) ||
    /^\s*[{[]/.test(content) ||
    /<\/?[a-z][\s\S]*>/i.test(content) ||
    /\b(export|import|function|const|let|class)\b/.test(content)
  );
}

function formatTechnicalDetail(
  content: string,
  metadata: Record<string, unknown> | undefined,
): string | null {
  const sections: string[] = [];
  const formattedContent = formatTechnicalContent(content);
  if (formattedContent) {
    sections.push(formattedContent);
  }

  if (metadata && Object.keys(metadata).length > 0) {
    sections.push(`metadata:\n${JSON.stringify(metadata, null, 2)}`);
  }

  const detail = sections.join("\n\n").trim();
  return detail ? limitTechnicalDetail(detail) : null;
}

function formatTechnicalContent(content: string): string | null {
  const source = content
    .replace(/^Agent message:\s*/i, "")
    .replace(/^Agent plan:\s*/i, "")
    .trim();
  if (!source) return null;

  const fenced = source.match(/```(?:json|ts|tsx|js|jsx|css|html)?\s*([\s\S]*?)```/i);
  const body = (fenced?.[1] ?? source).trim();
  if (!body) return null;

  return tryFormatJson(body) ?? tryFormatJson(extractJsonCandidateForDetail(body)) ?? body;
}

function extractJsonCandidateForDetail(content: string | null): string | null {
  if (!content) return null;
  const source = content.trim();
  if (!source) return null;
  if (
    (source.startsWith("{") && source.endsWith("}")) ||
    (source.startsWith("[") && source.endsWith("]"))
  ) {
    return source;
  }

  const firstObject = source.indexOf("{");
  const lastObject = source.lastIndexOf("}");
  if (firstObject >= 0 && lastObject > firstObject) {
    return source.slice(firstObject, lastObject + 1);
  }

  const firstArray = source.indexOf("[");
  const lastArray = source.lastIndexOf("]");
  if (firstArray >= 0 && lastArray > firstArray) {
    return source.slice(firstArray, lastArray + 1);
  }

  return null;
}

function tryFormatJson(content: string | null): string | null {
  if (!content) return null;
  try {
    return JSON.stringify(JSON.parse(content), null, 2);
  } catch {
    return null;
  }
}

function limitTechnicalDetail(content: string): string {
  const maxLength = 12_000;
  if (content.length <= maxLength) return content;
  return `${content.slice(0, maxLength)}\n...`;
}

function truncate(value: string, maxLength: number): string {
  const compact = value.split(/\s+/).join(" ").trim();
  if (compact.length <= maxLength) return compact;
  return `${compact.slice(0, Math.max(0, maxLength - 3))}...`;
}
