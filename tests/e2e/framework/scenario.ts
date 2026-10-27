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
  // Keys name calls/checks in a shared scenario. Overrides replace whole fields.
  requests?: Record<string, JsonObject>;
  expectations?: Record<string, JsonObject>;
};

export type ConnectorDefinition = {
  id: string;
  credentialKey: string;
  card: CardData;
  overrides?: Record<string, ScenarioOverrides>;
  exclude?: Record<string, { reason: string }>;
};

export type Scenario = {
  id: string;
  title: string;
  description: string;
};
