import assert from "node:assert/strict";
import { describe, it } from "node:test";
import {
  GENERATED_APP_IFRAME_REFERRER_POLICY,
  GENERATED_APP_IFRAME_SANDBOX,
  GENERATED_APP_IFRAME_SECURITY_PROPS,
} from "./previewFrameSecurity";
import { evaluatePreviewWatchdogDrift } from "./previewWatchdog";

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

describe("generated app preview watchdog", () => {
  it("ignores normal timer drift", () => {
    assert.deepEqual(evaluatePreviewWatchdogDrift(1, 250), {
      hitCount: 0,
      shouldSuspend: false,
    });
  });

  it("suspends after repeated long host tasks", () => {
    const first = evaluatePreviewWatchdogDrift(0, 2_100);
    const second = evaluatePreviewWatchdogDrift(first.hitCount, 2_200);

    assert.deepEqual(first, { hitCount: 1, shouldSuspend: false });
    assert.deepEqual(second, { hitCount: 2, shouldSuspend: true });
  });

  it("suspends immediately after a severe host task", () => {
    assert.deepEqual(evaluatePreviewWatchdogDrift(0, 5_100), {
      hitCount: 2,
      shouldSuspend: true,
    });
  });
});
