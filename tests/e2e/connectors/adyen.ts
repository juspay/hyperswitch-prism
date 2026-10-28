import type { ConnectorDefinition } from "../framework/types.js";

export const adyen = {
  id: "adyen",
  credentialKey: "adyen",
} as const satisfies ConnectorDefinition;
