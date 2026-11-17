import { createScenarioResolver } from "../framework/scenario.js";

/** Canonical IDs only. Each executable spec owns its metadata and setup requirements. */
export const scenarioIds = [
  "payments/capture/full-capture",
  "payments/create-confirm/card/one-time/no3ds/automatic-capture-create-then-confirm",
  "payments/create-confirm/card/one-time/no3ds/manual-capture-create-confirm",
  "payments/create-confirm/card/one-time/no3ds/manual-capture-create-then-confirm",
  "payments/create-confirm/card/one-time/no3ds/guest-automatic-capture-create-confirm",
  "payments/create-confirm/card/one-time/no3ds/guest-manual-capture-create-confirm",
  "payments/create-confirm/card/one-time/no3ds/create-confirm",
] as const;

export type ScenarioId = (typeof scenarioIds)[number];

/** Binds spec declarations to the typed inventory and their physical paths. */
export const scenarioFromFile = createScenarioResolver(scenarioIds);
