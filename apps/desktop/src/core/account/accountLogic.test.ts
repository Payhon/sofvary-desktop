import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { accountInitials, accountStatusLine, shouldRefreshAccount } from "./accountLogic";

describe("accountLogic", () => {
  it("formats initials and account status", () => {
    const user = {
      id: "user_1",
      email: "payhon@example.com",
      username: "payhon",
      displayName: "Pay Hon",
      bio: "",
      avatarUrl: null,
      role: "user" as const,
      status: "active" as const,
      plan: "pro",
      createdAt: "2026-06-19T00:00:00.000Z",
      updatedAt: "2026-06-19T00:00:00.000Z",
    };

    assert.equal(accountInitials(user), "PH");
    assert.equal(accountStatusLine({ kind: "signed-in", user, tokens: null }), "Pay Hon · pro");
    assert.equal(accountStatusLine({ kind: "signed-out", user: null, tokens: null }), "Sign in to sync marketplace access.");
  });

  it("refreshes close to token expiry", () => {
    assert.equal(
      shouldRefreshAccount(
        {
          accessToken: "access",
          refreshToken: "refresh",
          expiresAt: new Date(1_000_000).toISOString(),
        },
        800_000,
      ),
      true,
    );
  });
});
