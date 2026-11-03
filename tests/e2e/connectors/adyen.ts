import type { ConnectorDefinition } from "../framework/types.js";
import type { ScenarioId } from "../scenarios/catalog.js";

export const adyen = {
  id: "adyen",
} as const satisfies ConnectorDefinition<ScenarioId>;
