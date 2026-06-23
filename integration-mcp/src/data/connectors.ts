/**
 * Runtime accessors over the proto-derived connector data.
 *
 * JSON is loaded via fs (not `import ... with { type: "json" }`) so the module
 * works identically under tsx (src/) and compiled (dist/) on Node 18+, where
 * import attributes are inconsistently available.
 */
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

export type FieldType = "string" | "boolean" | "string[]";

export interface ConnectorField {
  name: string; // camelCase, as used in connectorConfig
  protoName: string; // snake_case
  type: FieldType;
  secret: boolean; // wrap as { value: "..." }
  required: boolean;
  tag: number;
  isBaseUrl: boolean;
}

export interface SandboxCard {
  brand: string;
  number: string;
  expMonth: string;
  expYear: string;
  cvc: string;
}

export interface Connector {
  connector: string; // machine name (connectorConfig key)
  configMessage: string;
  interfaceName: string;
  fields: ConnectorField[];
  supportsBaseUrl: boolean;
  displayName: string;
  paymentMethods: string[];
  sandboxCards: SandboxCard[];
  docsUrl: string | null;
  notes: string | null;
  fieldDocs: Record<string, { description?: string; example?: string }>;
}

interface GeneratedFile {
  connectorCount: number;
  connectors: Connector[];
}

interface EnumsFile {
  PaymentStatus: Record<string, number>;
  RefundStatus: Record<string, number>;
  CaptureMethod: Record<string, number>;
  AuthenticationType: Record<string, number>;
  Environment: Record<string, number>;
  Currency: Record<string, number>;
}

function loadJson<T>(relative: string): T {
  const url = new URL(relative, import.meta.url);
  return JSON.parse(readFileSync(fileURLToPath(url), "utf8")) as T;
}

const generated = loadJson<GeneratedFile>("./connectors.generated.json");
export const ENUMS = loadJson<EnumsFile>("./enums.generated.json");

const BY_NAME = new Map<string, Connector>();
for (const c of generated.connectors) BY_NAME.set(c.connector, c);

export const CONNECTOR_COUNT = generated.connectors.length;

export function listConnectors(): Connector[] {
  return generated.connectors;
}

export function getConnector(name: string): Connector | undefined {
  return BY_NAME.get(name.trim().toLowerCase());
}

/** Case-insensitive substring search over machine name and display name. */
export function searchConnectors(query: string): Connector[] {
  const q = query.trim().toLowerCase();
  if (!q) return generated.connectors;
  return generated.connectors.filter(
    (c) =>
      c.connector.includes(q) ||
      c.displayName.toLowerCase().includes(q) ||
      c.configMessage.toLowerCase().includes(q),
  );
}

/** Fields that a caller must supply (excludes optional + base_url). */
export function requiredFields(c: Connector): ConnectorField[] {
  return c.fields.filter((f) => f.required && !f.isBaseUrl);
}

/** Optional non-baseUrl fields. */
export function optionalFields(c: Connector): ConnectorField[] {
  return c.fields.filter((f) => !f.required && !f.isBaseUrl);
}

/** SCREAMING_SNAKE env var name for a connector field, e.g. stripe.apiKey -> STRIPE_API_KEY. */
export function envVarName(connector: string, field: ConnectorField): string {
  return `${connector.toUpperCase()}_${field.protoName.toUpperCase()}`;
}

/** Currency code -> numeric enum value (e.g. "USD" -> 146). */
export function currencyCode(code: string): number | undefined {
  return ENUMS.Currency[code.trim().toUpperCase()];
}

/** Suggested closest connector names for a miss (cheap Levenshtein-ish prefix match). */
export function suggestConnectors(name: string, limit = 5): string[] {
  const q = name.trim().toLowerCase();
  if (!q) return [];
  const scored = generated.connectors
    .map((c) => {
      const n = c.connector;
      let score = 0;
      if (n.startsWith(q) || q.startsWith(n)) score = 3;
      else if (n.includes(q) || q.includes(n)) score = 2;
      else if (n[0] === q[0]) score = 1;
      return { n, score };
    })
    .filter((s) => s.score > 0)
    .sort((a, b) => b.score - a.score)
    .slice(0, limit)
    .map((s) => s.n);
  return scored;
}
