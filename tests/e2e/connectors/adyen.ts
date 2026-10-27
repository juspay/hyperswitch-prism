import type { ConnectorDefinition } from "../framework/scenario.js";

export const adyen = {
  id: "adyen",
  credentialKey: "adyen",
  capabilities: {
    paymentMethods: ["card"],
    authentications: ["no_three_ds"],
    captureMethods: ["automatic"],
  },
  card: {
    card_number: "4111111111111111",
    card_exp_month: "03",
    card_exp_year: "30",
    card_holder_name: "Test Customer",
    card_cvc: "737",
  },
} as const satisfies ConnectorDefinition;
