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

export type ExpectedValue =
  | string
  | number
  | boolean
  | null
  | { oneOf: readonly (string | number | boolean | null)[] }
  | { from: "connector.id" | "merchant.merchantConnectorId" };

export type PaymentRequest = {
  amount: number;
  currency: string;
  confirm: boolean;
  authentication_type: Authentication;
  capture_method: CaptureMethod;
  payment_method: "card";
  payment_method_type: "credit" | "debit";
  payment_method_data: { card: "$connector.card" };
  billing?: { address: Record<string, string> };
  browser_info?: Record<string, string | number | boolean>;
  email?: string;
  return_url?: string;
};

export type ScenarioStep = {
  name: string;
  expect: Record<string, ExpectedValue>;
} & (
  | { action: "createPayment"; request: PaymentRequest; saveAs: string }
  | {
      action: "retrievePayment";
      payment: string;
      request: { force_sync: boolean };
      waitForStatus: string;
    }
  | {
      action: "verifyPaymentRoute";
      payment: string;
      request: { attempt_status: "charged" };
    }
);

export type PaymentScenario = {
  id: string;
  title: string;
  description: string;
  paymentMethod: "card";
  authentication: Authentication;
  captureMethod: CaptureMethod;
  steps: readonly ScenarioStep[];
};

export function supports(
  connector: ConnectorDefinition,
  scenario: PaymentScenario,
): boolean {
  return (
    connector.capabilities.paymentMethods.includes(scenario.paymentMethod) &&
    connector.capabilities.authentications.includes(scenario.authentication) &&
    connector.capabilities.captureMethods.includes(scenario.captureMethod)
  );
}
