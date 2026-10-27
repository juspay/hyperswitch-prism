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

export type ScenarioOverrides = {
  // Request differences only; the shared scenario owns its expected behavior.
  requests?: { createPayment?: JsonObject };
};

export type ConnectorDefinition = {
  id: string;
  credentialKey: string;
  credentialEntry?: string;
  card: CardData;
  overrides?: Record<string, ScenarioOverrides>;
  exclude?: Record<string, { reason: string }>;
};

export type Scenario = {
  id: string;
  title: string;
  description: string;
};
