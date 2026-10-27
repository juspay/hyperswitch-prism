import type { ConnectorDefinition } from "../framework/types.js";

export const adyen = {
  id: "adyen",
  credentialKey: "adyen",
  card: {
    card_number: "4111111111111111",
    card_exp_month: "03",
    card_exp_year: "30",
    card_holder_name: "Test Customer",
    card_cvc: "737",
  },
} as const satisfies ConnectorDefinition;
