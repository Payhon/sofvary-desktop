export interface ThreadCodeCard {
  title: string;
  fileCount: number;
  paths: string[];
  formattedJson: string;
}

export interface JsonHighlightToken {
  kind: "key" | "string" | "number" | "boolean" | "null" | "punctuation" | "plain";
  text: string;
}

interface AgentFileJson {
  relativePath?: unknown;
  path?: unknown;
  contents?: unknown;
  content?: unknown;
  text?: unknown;
}

export function parseThreadCodeCard(content: string): ThreadCodeCard | null {
  const candidate = extractJsonCandidate(content);
  if (!candidate) return null;

  let value: unknown;
  try {
    value = JSON.parse(candidate);
  } catch {
    return null;
  }

  if (!looksLikeCodePayload(value)) return null;

  const paths = collectFilePaths(value);
  return {
    title: paths[0] ?? "Agent JSON",
    fileCount: paths.length,
    paths,
    formattedJson: JSON.stringify(value, null, 2),
  };
}

export function tokenizeJsonHighlight(input: string): JsonHighlightToken[] {
  const tokens: JsonHighlightToken[] = [];
  const pattern =
    /("(?:\\u[\da-fA-F]{4}|\\[^u]|[^\\"])*"(?=\s*:)|"(?:\\u[\da-fA-F]{4}|\\[^u]|[^\\"])*"|-?\d+(?:\.\d+)?(?:[eE][+-]?\d+)?|\btrue\b|\bfalse\b|\bnull\b|[{}\[\]:,])/g;
  let lastIndex = 0;
  let match: RegExpExecArray | null;

  while ((match = pattern.exec(input)) !== null) {
    if (match.index > lastIndex) {
      tokens.push({ kind: "plain", text: input.slice(lastIndex, match.index) });
    }

    const text = match[0];
    const kind =
      text.startsWith('"') && /^\s*:/.test(input.slice(pattern.lastIndex))
        ? "key"
        : classifyJsonToken(text);
    tokens.push({ kind, text });
    lastIndex = pattern.lastIndex;
  }

  if (lastIndex < input.length) {
    tokens.push({ kind: "plain", text: input.slice(lastIndex) });
  }

  return tokens;
}

function extractJsonCandidate(content: string): string | null {
  const withoutPrefix = content.replace(/^Agent message:\s*/i, "").trim();
  const fenced = withoutPrefix.match(/```(?:json)?\s*([\s\S]*?)```/i);
  const source = (fenced?.[1] ?? withoutPrefix).trim();
  if (!source) return null;

  if (source.startsWith("{") && source.endsWith("}")) {
    return source;
  }

  const firstObject = source.indexOf("{");
  const lastObject = source.lastIndexOf("}");
  if (firstObject >= 0 && lastObject > firstObject) {
    return source.slice(firstObject, lastObject + 1);
  }

  return null;
}

function looksLikeCodePayload(value: unknown): boolean {
  if (!isRecord(value)) return false;
  const record = value;
  if (Array.isArray(record.files) && record.files.some(looksLikeFileEntry)) return true;
  if (looksLikeFileEntry(value)) return true;
  return typeof record.code === "string" || typeof record.contents === "string";
}

function looksLikeFileEntry(value: unknown): value is AgentFileJson {
  if (!isRecord(value)) return false;
  const path = value.relativePath ?? value.path;
  const contents = value.contents ?? value.content ?? value.text;
  return typeof path === "string" && typeof contents === "string";
}

function collectFilePaths(value: unknown): string[] {
  if (isRecord(value) && Array.isArray(value.files)) {
    return value.files
      .filter(looksLikeFileEntry)
      .map((file) => String(file.relativePath ?? file.path))
      .filter(Boolean);
  }

  if (looksLikeFileEntry(value)) {
    return [String(value.relativePath ?? value.path)];
  }

  return [];
}

function classifyJsonToken(text: string): JsonHighlightToken["kind"] {
  if (/^"/.test(text)) return "string";
  if (/^-?\d/.test(text)) return "number";
  if (text === "true" || text === "false") return "boolean";
  if (text === "null") return "null";
  if (/^[{}\[\]:,]$/.test(text)) return "punctuation";
  return "plain";
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
