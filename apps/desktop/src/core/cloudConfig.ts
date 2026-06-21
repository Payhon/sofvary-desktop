const DEFAULT_SOFVARY_ORIGIN = "https://sofvary.vercel.app";

const VITE_ENV = (import.meta as ImportMeta & { env?: Record<string, string | undefined> }).env;

function readEnv(name: string): string | undefined {
  const value = VITE_ENV?.[name]?.trim();
  return value ? value : undefined;
}

function normalizeBaseUrl(value: string): string {
  return value.trim().replace(/\/+$/, "");
}

function configuredBaseUrl(primaryEnvName: string): string {
  return normalizeBaseUrl(readEnv(primaryEnvName) ?? readEnv("VITE_SOFVARY_BASE_URL") ?? DEFAULT_SOFVARY_ORIGIN);
}

export const SOFVARY_API_BASE_URL = configuredBaseUrl("VITE_SOFVARY_API_BASE_URL");
export const SOFVARY_WEB_BASE_URL = configuredBaseUrl("VITE_SOFVARY_WEB_BASE_URL");

export function sofvaryWebsiteUrl(path = "/"): string {
  const normalizedPath = path.startsWith("/") && !path.startsWith("//") ? path : "/";
  return `${SOFVARY_WEB_BASE_URL}${normalizedPath}`;
}
