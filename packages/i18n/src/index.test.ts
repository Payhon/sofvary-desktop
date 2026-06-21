import assert from "node:assert/strict";
import { describe, it } from "node:test";
import {
  DEFAULT_LOCALE,
  createTranslator,
  normalizeLocale,
  resolveLocaleFromHeader,
  translate,
  translateText,
} from "./index";

describe("@sofvary/i18n", () => {
  it("normalizes supported and regional locales with English default", () => {
    assert.equal(DEFAULT_LOCALE, "en");
    assert.equal(normalizeLocale("en"), "en");
    assert.equal(normalizeLocale("en-US"), "en");
    assert.equal(normalizeLocale("zh"), "zh-CN");
    assert.equal(normalizeLocale("zh_CN"), "zh-CN");
    assert.equal(normalizeLocale("fr"), "en");
    assert.equal(normalizeLocale(null), "en");
  });

  it("resolves Accept-Language quality order", () => {
    assert.equal(resolveLocaleFromHeader("fr;q=0.9, zh-CN;q=0.8, en;q=0.1"), "zh-CN");
    assert.equal(resolveLocaleFromHeader("en-US;q=0.4, zh;q=0.9"), "zh-CN");
  });

  it("translates namespaced keys and interpolates params", () => {
    assert.equal(
      translate("home.catalogCopy", "en", { appCount: 4, packCount: 10 }, "web"),
      "Browse 4 starter capsules and 10 pack references that show how intent becomes local, inspectable software.",
    );
    assert.equal(translate("validation.required", "zh-CN", { field: "email" }, "api"), "email 必填");
    const t = createTranslator("zh-CN", "desktop");
    assert.equal(t("build.status.planning"), "正在分析意图");
  });

  it("supports legacy text lookup for incremental UI migration", () => {
    assert.equal(translateText("刷新", "en", "desktop"), "Refresh");
    assert.equal(translateText("刷新", "zh-CN", "desktop"), "刷新");
    assert.equal(translateText("raw", "en", "desktop"), "raw");
  });
});
