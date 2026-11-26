import { createScenarioResolver } from "../framework/scenario.js";

/** Canonical IDs only. Each executable spec owns its metadata and setup requirements. */
export const scenarioIds = [
  "payments/cancel/full-void-authorized-payment",
  "payments/capture/overcapture-authorized-payment",
  "payments/capture/partial-capture-authorized-payment",
  "payments/create-confirm/card/one-time/no3ds/automatic-capture-create-confirm",
  "payments/create-confirm/card/one-time/no3ds/automatic-capture-create-then-confirm",
  "payments/create-confirm/card/one-time/no3ds/manual-capture-create-confirm",
  "payments/create-confirm/card/one-time/no3ds/manual-capture-create-then-confirm",
  "payments/create-confirm/card/one-time/no3ds/guest-automatic-capture-create-confirm",
  "payments/create-confirm/card/one-time/no3ds/guest-manual-capture-create-confirm",
  "payments/create-confirm/card/one-time/three-ds/automatic-capture-create-confirm",
  "payments/create-confirm/card/one-time/external-three-ds/automatic-capture-create-confirm",
  "payments/create-confirm/card/recurring/connector-mandate/no3ds/nonzero-amount-manual-capture-setup-mandate-then-recurring",
  "payments/create-confirm/card/recurring/connector-mandate/no3ds/nonzero-amount-setup-mandate-then-recurring",
  "payments/create-confirm/card/recurring/connector-mandate/no3ds/zero-amount-setup-mandate-then-recurring",
  "payments/create-confirm/card/recurring/network-transaction-id/no3ds/setup-mandate-then-recurring",
  "payments/create-confirm/card/repeat-cit/no3ds/saved-card-create-confirm",
  "payments/incremental-authorization/increase-authorized-amount",
  "payments/retrieve/force-sync-payment",
  "refunds/create/full-refund-captured-payment",
  "refunds/create/partial-refund-after-capture",
  "refunds/reverse/reverse-successful-refund-before-settlement",
] as const;

export type ScenarioId = (typeof scenarioIds)[number];

/** Binds spec declarations to the typed inventory and their physical paths. */
export const scenarioFromFile = createScenarioResolver(scenarioIds);
