import { safeInvoke } from "../../platform/tauriClient";
import { SOFVARY_API_BASE_URL } from "../cloudConfig";

export interface AccountUser {
  id: string;
  email: string;
  username: string;
  displayName: string;
  bio: string;
  avatarUrl: string | null;
  role: "user" | "admin";
  status: "active" | "disabled";
  plan: string;
  createdAt: string;
  updatedAt: string;
}

export interface AuthTokens {
  accessToken: string;
  refreshToken: string;
  expiresAt: string;
}

export interface AuthResponse {
  user: AccountUser;
  tokens: AuthTokens;
}

export class AccountApiError extends Error {
  constructor(
    readonly status: number,
    message: string,
  ) {
    super(message);
    this.name = "AccountApiError";
  }
}

export async function loginAccount(email: string, password: string): Promise<AuthResponse> {
  return accountRequest<AuthResponse>("/v1/auth/login", {
    method: "POST",
    body: JSON.stringify({ email, password }),
  });
}

export async function registerAccount(input: {
  email: string;
  password: string;
  username?: string;
  displayName?: string;
}): Promise<AuthResponse> {
  return accountRequest<AuthResponse>("/v1/auth/register", {
    method: "POST",
    body: JSON.stringify(input),
  });
}

export async function refreshAccount(refreshToken: string): Promise<AuthResponse> {
  return accountRequest<AuthResponse>("/v1/auth/refresh", {
    method: "POST",
    body: JSON.stringify({ refreshToken }),
  });
}

export async function logoutAccount(refreshToken: string): Promise<void> {
  await accountRequest<{ ok: true }>("/v1/auth/logout", {
    method: "POST",
    body: JSON.stringify({ refreshToken }),
  });
}

export async function getAccountMe(accessToken: string): Promise<{ user: AccountUser }> {
  return accountRequest<{ user: AccountUser }>("/v1/users/me", {}, accessToken);
}

export async function saveRefreshToken(refreshToken: string): Promise<void> {
  await safeInvoke<void>("set_account_refresh_token", { payload: { refreshToken } });
}

export async function getSavedRefreshToken(): Promise<string | null> {
  return safeInvoke<string | null>("get_account_refresh_token");
}

export async function clearRefreshToken(): Promise<void> {
  await safeInvoke<void>("clear_account_refresh_token");
}

export async function openSofvaryWebsite(path = "/"): Promise<void> {
  await safeInvoke<void>("open_sofvary_website", { path });
}

async function accountRequest<T>(path: string, init: RequestInit = {}, accessToken?: string): Promise<T> {
  const headers = new Headers(init.headers);
  if (init.body && !headers.has("Content-Type")) {
    headers.set("Content-Type", "application/json");
  }
  if (accessToken) {
    headers.set("Authorization", `Bearer ${accessToken}`);
  }
  const response = await fetch(`${SOFVARY_API_BASE_URL}${path}`, { ...init, headers });
  if (!response.ok) {
    const payload = await readJson(response);
    throw new AccountApiError(response.status, payload?.error?.message ?? response.statusText);
  }
  return (await readJson(response)) as T;
}

async function readJson(response: Response): Promise<any> {
  const text = await response.text();
  if (!text) {
    return null;
  }
  try {
    return JSON.parse(text);
  } catch {
    return null;
  }
}
