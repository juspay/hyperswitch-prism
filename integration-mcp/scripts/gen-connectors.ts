/**
 * Build-time codegen.
 *
 * Parses the canonical hyperswitch-prism proto (`payment.proto`) and emits two
 * committed data files consumed at runtime by the MCP tools:
 *
 *   src/data/connectors.generated.json  — every connector's exact credential
 *                                          field shape (camelCase names, secret
 *                                          vs plain, required vs optional).
 *   src/data/enums.generated.json       — PaymentStatus / RefundStatus /
 *                                          CaptureMethod / AuthenticationType /
 *                                          Environment / Currency name->number.
 *
 * The proto is NOT shipped in the published npm package — these JSON files are.
 * Run via `npm run gen:connectors` (wired into `prebuild`).
 *
 * Source of truth, so the server never hallucinates a field name or status code.
 */
import { readFileSync, writeFileSync, existsSync, mkdirSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const root = resolve(__dirname, "..");
const dataDir = join(root, "src", "data");

// ---------------------------------------------------------------------------
// Locate the proto.
// ---------------------------------------------------------------------------
const PROTO_CANDIDATES = [
  process.env.PRISM_PROTO_PATH,
  resolve(root, "..", "crates/types-traits/grpc-api-types/proto/payment.proto"),
  resolve(root, "../../crates/types-traits/grpc-api-types/proto/payment.proto"),
].filter((p): p is string => Boolean(p));

function findProto(): string | null {
  for (const candidate of PROTO_CANDIDATES) {
    if (existsSync(candidate)) return candidate;
  }
  return null;
}

// Messages that match `*Config` but are not connectors. Empty for now; the
// oneof drives the connector set, so stray Config messages are simply ignored.
const IGNORE_MESSAGES = new Set<string>([]);

interface ConnectorField {
  name: string; // camelCase, as used in the JS/TS connectorConfig
  protoName: string; // snake_case, as in proto
  type: "string" | "boolean" | "string[]";
  secret: boolean; // wrap as { value: "..." }
  required: boolean;
  tag: number;
  isBaseUrl: boolean;
}

interface GeneratedConnector {
  connector: string; // machine name (oneof field name)
  configMessage: string; // e.g. "StripeConfig"
  interfaceName: string; // e.g. "IStripeConfig"
  fields: ConnectorField[];
  supportsBaseUrl: boolean;
}

function snakeToCamel(s: string): string {
  return s.replace(/_([a-z0-9])/g, (_m, c: string) => c.toUpperCase());
}

function titleCase(machine: string): string {
  return machine
    .split("_")
    .map((p) => p.charAt(0).toUpperCase() + p.slice(1))
    .join(" ");
}

// ---------------------------------------------------------------------------
// Parse the ConnectorSpecificConfig oneof: maps ConfigMessage -> machine name.
// ---------------------------------------------------------------------------
function parseOneof(proto: string): Map<string, string> {
  const msgMatch = proto.match(/message\s+ConnectorSpecificConfig\s*\{([\s\S]*?)\n\}/);
  if (!msgMatch) throw new Error("Could not locate ConnectorSpecificConfig message");
  const oneofBody = msgMatch[1];
  const map = new Map<string, string>(); // ConfigMessage -> machineName
  const lineRe = /^\s*([A-Za-z0-9]+Config)\s+([a-z0-9_]+)\s*=\s*\d+\s*;/gm;
  let m: RegExpExecArray | null;
  while ((m = lineRe.exec(oneofBody)) !== null) {
    map.set(m[1], m[2]);
  }
  return map;
}

// ---------------------------------------------------------------------------
// Parse a single `message XConfig { ... }` body into fields.
// ---------------------------------------------------------------------------
const FIELD_RE =
  /^\s*(optional\s+|repeated\s+)?(SecretString|string|bool)\s+([a-z0-9_]+)\s*=\s*(\d+)\s*;/;

function parseConfigMessage(proto: string, configMessage: string): ConnectorField[] | null {
  // Non-greedy body up to the first closing brace at column 0 (`\n}`).
  const re = new RegExp(`message\\s+${configMessage}\\s*\\{([\\s\\S]*?)\\n\\}`);
  const match = proto.match(re);
  if (!match) return null;
  const body = match[1];
  const fields: ConnectorField[] = [];
  for (const rawLine of body.split("\n")) {
    const fm = rawLine.match(FIELD_RE);
    if (!fm) continue;
    const modifier = (fm[1] || "").trim();
    const protoType = fm[2];
    const protoName = fm[3];
    const tag = Number(fm[4]);
    const repeated = modifier === "repeated";
    const optional = modifier === "optional";
    const secret = protoType === "SecretString";
    let type: ConnectorField["type"] = "string";
    if (repeated) type = "string[]";
    else if (protoType === "bool") type = "boolean";
    const name = snakeToCamel(protoName);
    fields.push({
      name,
      protoName,
      type,
      secret,
      required: !optional && !repeated,
      tag,
      isBaseUrl: tag === 50 && name === "baseUrl",
    });
  }
  return fields;
}

// ---------------------------------------------------------------------------
// Parse a numeric proto enum into a name->number map.
// ---------------------------------------------------------------------------
function parseEnum(proto: string, enumName: string): Record<string, number> {
  const re = new RegExp(`enum\\s+${enumName}\\s*\\{([\\s\\S]*?)\\n\\}`);
  const match = proto.match(re);
  if (!match) throw new Error(`Could not locate enum ${enumName}`);
  const body = match[1];
  const out: Record<string, number> = {};
  // Values may wrap across lines (e.g. `NAME =\n  18;`). Strip comments first.
  const cleaned = body.replace(/\/\/[^\n]*/g, " ").replace(/\/\*[\s\S]*?\*\//g, " ");
  const entryRe = /([A-Z][A-Z0-9_]*)\s*=\s*(\d+)\s*;/g;
  let m: RegExpExecArray | null;
  while ((m = entryRe.exec(cleaned)) !== null) {
    out[m[1]] = Number(m[2]);
  }
  return out;
}

// ---------------------------------------------------------------------------
// Overlay merge (curated display names, test cards, payment methods, docs).
// ---------------------------------------------------------------------------
interface OverlayEntry {
  displayName?: string;
  paymentMethods?: string[];
  sandboxCards?: { brand: string; number: string; expMonth: string; expYear: string; cvc: string }[];
  docsUrl?: string;
  notes?: string;
  fieldDocs?: Record<string, { description?: string; example?: string }>;
}

const DEFAULT_SANDBOX_CARDS = [
  { brand: "Visa", number: "4111111111111111", expMonth: "12", expYear: "2030", cvc: "123" },
  { brand: "Mastercard", number: "5555555555554444", expMonth: "12", expYear: "2030", cvc: "123" },
];

function loadOverlay(): Record<string, OverlayEntry> {
  const path = join(dataDir, "connectors.overlay.json");
  if (!existsSync(path)) return {};
  return JSON.parse(readFileSync(path, "utf8")) as Record<string, OverlayEntry>;
}

// ---------------------------------------------------------------------------
// Main.
// ---------------------------------------------------------------------------
function main(): void {
  const protoPath = findProto();
  mkdirSync(dataDir, { recursive: true });

  if (!protoPath) {
    const genPath = join(dataDir, "connectors.generated.json");
    if (existsSync(genPath)) {
      console.warn(
        "[gen-connectors] payment.proto not found; keeping committed connectors.generated.json.\n" +
          "  (Set PRISM_PROTO_PATH to regenerate.)",
      );
      return;
    }
    throw new Error(
      "[gen-connectors] payment.proto not found and no committed JSON exists.\n" +
        `  Looked in:\n    ${PROTO_CANDIDATES.join("\n    ")}\n` +
        "  Set PRISM_PROTO_PATH to the proto location.",
    );
  }

  const proto = readFileSync(protoPath, "utf8");
  // Some enums (Environment) live in a sibling proto. Concatenate for enum parsing.
  const sdkConfigPath = join(dirname(protoPath), "sdk_config.proto");
  const enumSource = existsSync(sdkConfigPath)
    ? proto + "\n" + readFileSync(sdkConfigPath, "utf8")
    : proto;
  const oneof = parseOneof(proto);
  const overlay = loadOverlay();

  const connectors: (GeneratedConnector & {
    displayName: string;
    paymentMethods: string[];
    sandboxCards: OverlayEntry["sandboxCards"];
    docsUrl: string | null;
    notes: string | null;
    fieldDocs: Record<string, { description?: string; example?: string }>;
  })[] = [];

  const seenCamel = new Set<string>();
  for (const [configMessage, machine] of oneof) {
    if (IGNORE_MESSAGES.has(configMessage)) continue;
    const fields = parseConfigMessage(proto, configMessage);
    if (!fields) {
      console.warn(`[gen-connectors] no body found for ${configMessage}; skipping`);
      continue;
    }
    // camelCase collision guard within a connector.
    const localCamel = new Set<string>();
    for (const f of fields) {
      if (localCamel.has(f.name)) {
        throw new Error(`[gen-connectors] camelCase collision '${f.name}' in ${configMessage}`);
      }
      localCamel.add(f.name);
    }
    seenCamel.add(machine);
    const ov = overlay[machine] ?? {};
    connectors.push({
      connector: machine,
      configMessage,
      interfaceName: `I${configMessage}`,
      fields,
      supportsBaseUrl: fields.some((f) => f.isBaseUrl),
      displayName: ov.displayName ?? titleCase(machine),
      paymentMethods: ov.paymentMethods ?? ["card"],
      sandboxCards: ov.sandboxCards ?? DEFAULT_SANDBOX_CARDS,
      docsUrl: ov.docsUrl ?? null,
      notes: ov.notes ?? null,
      fieldDocs: ov.fieldDocs ?? {},
    });
  }

  // Overlay-key sanity: every overlay key must match a real connector.
  for (const key of Object.keys(overlay)) {
    if (!seenCamel.has(key)) {
      throw new Error(`[gen-connectors] overlay key '${key}' has no matching connector in proto`);
    }
  }

  connectors.sort((a, b) => a.connector.localeCompare(b.connector));

  if (connectors.length < 80 || connectors.length > 130) {
    throw new Error(`[gen-connectors] connector count ${connectors.length} outside expected band (80-130)`);
  }

  // Enums.
  const enums = {
    PaymentStatus: parseEnum(enumSource, "PaymentStatus"),
    RefundStatus: parseEnum(enumSource, "RefundStatus"),
    CaptureMethod: parseEnum(enumSource, "CaptureMethod"),
    AuthenticationType: parseEnum(enumSource, "AuthenticationType"),
    Environment: parseEnum(enumSource, "Environment"),
    Currency: parseEnum(enumSource, "Currency"),
  };

  // Sanity-check a few well-known status codes survived the parse.
  const expect = (cond: boolean, msg: string) => {
    if (!cond) throw new Error(`[gen-connectors] enum check failed: ${msg}`);
  };
  expect(enums.PaymentStatus.AUTHORIZED === 6, "PaymentStatus.AUTHORIZED===6");
  expect(enums.PaymentStatus.CHARGED === 8, "PaymentStatus.CHARGED===8");
  expect(enums.PaymentStatus.FAILURE === 21, "PaymentStatus.FAILURE===21");
  expect(enums.RefundStatus.REFUND_SUCCESS === 4, "RefundStatus.REFUND_SUCCESS===4");
  expect(enums.CaptureMethod.MANUAL === 2, "CaptureMethod.MANUAL===2");
  expect(enums.AuthenticationType.NO_THREE_DS === 2, "AuthenticationType.NO_THREE_DS===2");
  expect(enums.Currency.USD > 0, "Currency.USD present");

  const generatedFile = {
    _comment:
      "GENERATED by scripts/gen-connectors.ts from payment.proto. Do not edit by hand. Run `npm run gen:connectors`.",
    connectorCount: connectors.length,
    connectors,
  };

  writeFileSync(
    join(dataDir, "connectors.generated.json"),
    JSON.stringify(generatedFile, null, 2) + "\n",
  );
  writeFileSync(
    join(dataDir, "enums.generated.json"),
    JSON.stringify(
      { _comment: "GENERATED from payment.proto. Do not edit by hand.", ...enums },
      null,
      2,
    ) + "\n",
  );

  const fieldCount = connectors.reduce((n, c) => n + c.fields.length, 0);
  const withOverlay = connectors.filter((c) => Boolean(overlay[c.connector])).length;
  console.log(
    `[gen-connectors] ${connectors.length} connectors, ${fieldCount} fields, ${withOverlay} with overlay`,
  );
  console.log(`[gen-connectors] proto: ${protoPath}`);
}

main();
