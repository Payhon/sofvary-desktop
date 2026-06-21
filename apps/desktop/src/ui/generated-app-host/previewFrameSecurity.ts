export const GENERATED_APP_IFRAME_SANDBOX = "allow-forms allow-same-origin allow-scripts";
export const GENERATED_APP_IFRAME_REFERRER_POLICY = "no-referrer";

export const GENERATED_APP_IFRAME_SECURITY_PROPS = {
  sandbox: GENERATED_APP_IFRAME_SANDBOX,
  referrerPolicy: GENERATED_APP_IFRAME_REFERRER_POLICY,
} as const;
