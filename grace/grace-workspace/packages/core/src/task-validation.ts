/**
 * Runtime validation for task fields supplied by clients. TaskDefinition is
 * only a compile-time type: these values arrive as JSON from the control
 * socket, are stored, and later reach git refs, file paths, shell commands and
 * command-line arguments.
 */

/** Connector and payment method names. No dots or slashes, so no path traversal. */
const SAFE_IDENTIFIER_RE = /^[A-Za-z0-9][A-Za-z0-9_-]{0,63}$/;
/** Model ids such as `claude-sonnet-4-5[1m]` or `provider/model:tag`. */
const SAFE_MODEL_RE = /^[A-Za-z0-9][A-Za-z0-9._:/@[\]-]{0,127}$/;
/**
 * URLs passed to add_connector.sh, which splices them into sed expressions
 * delimited by `|` and into TOML. Excludes that delimiter, quotes, whitespace
 * and shell metacharacters.
 */
const SAFE_URL_RE = /^https?:\/\/[A-Za-z0-9._~:/?#=%+,@&{}-]+$/;

export function isSafeIdentifier(value: unknown): value is string {
  return typeof value === "string" && SAFE_IDENTIFIER_RE.test(value);
}

export function isSafeScaffoldUrl(value: unknown): value is string {
  return typeof value === "string" && SAFE_URL_RE.test(value);
}

export function isValidPort(value: unknown): value is number {
  return (
    typeof value === "number" &&
    Number.isInteger(value) &&
    value >= 1 &&
    value <= 65535
  );
}

/** For sinks that interpolate a port into a command: refuse anything else. */
export function assertValidPort(value: unknown, name = "port"): number {
  if (!isValidPort(value)) {
    throw new Error(`${name} must be an integer between 1 and 65535`);
  }
  return value;
}

/** Return a problem for each unsafe field. Absent or empty fields are fine. */
export function validateTaskInputs(task: unknown): string[] {
  if (task === undefined || task === null) return [];
  if (typeof task !== "object" || Array.isArray(task)) {
    return ["task must be an object"];
  }
  const t = task as Record<string, unknown>;
  const problems: string[] = [];
  const identifierRule =
    "must start with a letter or number and contain only letters, numbers, '_' and '-' (64 characters at most)";

  if (t.targetConnectors !== undefined && t.targetConnectors !== null) {
    if (!Array.isArray(t.targetConnectors)) {
      problems.push("targetConnectors must be an array");
    } else {
      for (const connector of t.targetConnectors) {
        if (connector !== "" && !isSafeIdentifier(connector)) {
          problems.push(`targetConnectors entries ${identifierRule}`);
          break;
        }
      }
    }
  }
  if (
    t.paymentMethod !== undefined &&
    t.paymentMethod !== null &&
    t.paymentMethod !== "" &&
    !isSafeIdentifier(t.paymentMethod)
  ) {
    problems.push(`paymentMethod ${identifierRule}`);
  }
  for (const field of ["grpcPort", "dummyConnectorPort"]) {
    if (t[field] !== undefined && t[field] !== null && !isValidPort(t[field])) {
      problems.push(`${field} must be an integer between 1 and 65535`);
    }
  }
  if (
    t.runnerModel !== undefined &&
    t.runnerModel !== null &&
    t.runnerModel !== "" &&
    !(typeof t.runnerModel === "string" && SAFE_MODEL_RE.test(t.runnerModel))
  ) {
    problems.push("runnerModel contains unsupported characters");
  }
  return problems;
}
