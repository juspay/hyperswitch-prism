import type { ConnectorDefinition } from "../framework/types.js";

export const adyen = {
  id: "adyen",
} as const satisfies ConnectorDefinition;
