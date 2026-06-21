import type { AccountUser, AuthTokens } from "./accountClient";

export type AccountStatusKind = "idle" | "loading" | "signed-in" | "signed-out" | "error";

export interface AccountState {
  kind: AccountStatusKind;
  user: AccountUser | null;
  tokens: AuthTokens | null;
  detail?: string;
}

export function accountInitials(user: Pick<AccountUser, "displayName" | "username" | "email"> | null): string {
  if (!user) {
    return "?";
  }
  const label = user.displayName || user.username || user.email;
  return label
    .split(/[\s._-]+/)
    .filter(Boolean)
    .slice(0, 2)
    .map((part) => part[0]?.toUpperCase() ?? "")
    .join("") || "?";
}

export function accountStatusLine(state: AccountState): string {
  switch (state.kind) {
    case "loading":
      return "Syncing Sofvary account...";
    case "signed-in":
      return state.user ? `${state.user.displayName || state.user.username} · ${state.user.plan}` : "Signed in";
    case "signed-out":
      return "Sign in to sync marketplace access.";
    case "error":
      return state.detail ?? "Account sync failed.";
    default:
      return "Account ready.";
  }
}

export function shouldRefreshAccount(tokens: AuthTokens | null, now = Date.now()): boolean {
  if (!tokens) {
    return false;
  }
  return Date.parse(tokens.expiresAt) - now < 5 * 60 * 1000;
}
