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

export type JsonValue =
  | string
  | number
  | boolean
  | null
  | JsonValue[]
  | { [field: string]: JsonValue };

// HS owns request validation. Additional API fields pass through unchanged.
export type PaymentRequest = {
  [field: string]: JsonValue | undefined;
  amount: number;
  currency: string;
  confirm: boolean;
  authentication_type: Authentication;
  capture_method: CaptureMethod;
  payment_method: "card";
  payment_method_type: "credit" | "debit";
  payment_method_data: { [field: string]: JsonValue; card: "$connector.card" };
  billing?: { [field: string]: JsonValue };
  browser_info?: { [field: string]: JsonValue };
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
  steps: readonly [ScenarioStep, ...ScenarioStep[]];
};

export function supports(
  connector: ConnectorDefinition,
  scenario: PaymentScenario,
): boolean {
  return scenario.steps.every((step) => {
    if (step.action !== "createPayment") return true;
    return (
      connector.capabilities.paymentMethods.includes(
        step.request.payment_method,
      ) &&
      connector.capabilities.authentications.includes(
        step.request.authentication_type,
      ) &&
      connector.capabilities.captureMethods.includes(
        step.request.capture_method,
      )
    );
  });
}
