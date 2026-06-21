import assert from "node:assert/strict";
import { describe, it } from "node:test";
import {
  GENERATED_APP_IFRAME_REFERRER_POLICY,
  GENERATED_APP_IFRAME_SANDBOX,
  GENERATED_APP_IFRAME_SECURITY_PROPS,
} from "./previewFrameSecurity";

describe("generated app preview iframe sandbox", () => {
  it("allows local app execution without top navigation or popup escape", () => {
    const permissions = new Set(GENERATED_APP_IFRAME_SANDBOX.split(/\s+/));

    assert.equal(permissions.has("allow-scripts"), true);
    assert.equal(permissions.has("allow-forms"), true);
    assert.equal(permissions.has("allow-same-origin"), true);
    assert.equal(permissions.has("allow-top-navigation"), false);
    assert.equal(permissions.has("allow-top-navigation-by-user-activation"), false);
    assert.equal(permissions.has("allow-popups"), false);
    assert.equal(permissions.has("allow-popups-to-escape-sandbox"), false);
    assert.equal(permissions.has("allow-modals"), false);
    assert.equal(permissions.has("allow-downloads"), false);
  });

  it("exports stable iframe security props", () => {
    assert.deepEqual(GENERATED_APP_IFRAME_SECURITY_PROPS, {
      sandbox: "allow-forms allow-same-origin allow-scripts",
      referrerPolicy: GENERATED_APP_IFRAME_REFERRER_POLICY,
    });
  });
});
