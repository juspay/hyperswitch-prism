export type Json = Record<string, unknown>;

export type JsonValue =
  string | number | boolean | null | JsonValue[] | JsonObject;
export type JsonObject = { [field: string]: JsonValue };

export type CardData = {
  card_number: string;
  card_exp_month: string;
  card_exp_year: string;
  card_holder_name: string;
  card_cvc: string;
};

export type RequestOverride = {
  reason: string;
  omit?: readonly (readonly string[])[];
  patch?: JsonObject;
};

export type ExpectationOverride = {
  reason: string;
  patch: JsonObject;
};

export type ScenarioOverrides = {
  requests?: {
    createPayment?: Record<string, RequestOverride>;
  };
  expectations?: Record<string, ExpectationOverride>;
};

export type ConnectorDefinition = {
  id: string;
  credentialKey?: string;
  credentialEntry?: string;
  card?: Partial<CardData>;
  overrides?: Record<string, ScenarioOverrides>;
  exclude?: Record<string, { reason: string }>;
};

export type Scenario = {
  id: string;
  title: string;
  description: string;
  verifies: string;
  paymentMethodsEnabled: readonly JsonObject[];
};
