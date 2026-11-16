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

export type ConnectorDefinition<TScenarioId extends string = string> = {
  id: string;
  run: readonly TScenarioId[];
  credentialKey?: string;
  credentialEntry?: string;
  card?: Partial<CardData>;
  overrides?: Partial<Record<TScenarioId, ScenarioOverrides>>;
  exclude?: Partial<Record<TScenarioId, { reason: string }>>;
};

export type ScenarioDefinition = {
  title: string;
  description: string;
  verifies: string;
  requirements: {
    connectorAccount: {
      paymentMethodsEnabled: readonly JsonObject[];
    };
  };
};

declare const scenarioBrand: unique symbol;

/** Constructed only by the path-to-catalog resolver. */
export type Scenario<TId extends string = string> = ScenarioDefinition & {
  id: TId;
  readonly [scenarioBrand]: true;
};
