export type Authentication = "no_three_ds" | "three_ds";
export type CaptureMethod = "automatic" | "manual";
export type PaymentMethod = "card";

export type CardData = {
  card_number: string;
  card_exp_month: string;
  card_exp_year: string;
  card_holder_name: string;
  card_cvc: string;
};

export type ConnectorDefinition = {
  id: string;
  credentialKey: string;
  capabilities: {
    paymentMethods: readonly PaymentMethod[];
    authentications: readonly Authentication[];
    captureMethods: readonly CaptureMethod[];
  };
  card: CardData;
};

export type OneTimeCardScenario = {
  id: string;
  title: string;
  description: string;
  paymentMethod: "card";
  authentication: Authentication;
  captureMethod: CaptureMethod;
  amount: number;
  currency: string;
  expectedStatus: "succeeded";
};

export function supports(
  connector: ConnectorDefinition,
  scenario: OneTimeCardScenario,
): boolean {
  return (
    connector.capabilities.paymentMethods.includes(scenario.paymentMethod) &&
    connector.capabilities.authentications.includes(scenario.authentication) &&
    connector.capabilities.captureMethods.includes(scenario.captureMethod)
  );
}
