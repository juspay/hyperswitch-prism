import type {
  ExpectationOverride,
  JsonObject,
  JsonValue,
  RequestOverride,
  ScenarioOverrides,
} from "./types.js";

type RequestOperation = "createPayment";

export class ScenarioOverrideResolver {
  private readonly consumed = new Set<string>();
  private readonly usedCalls = new Set<string>();
  private readonly usedChecks = new Set<string>();

  constructor(private readonly overrides: ScenarioOverrides) {
    validateConfiguration(overrides);
  }

  request(
    operation: RequestOperation,
    callId: string,
    base: JsonObject,
  ): JsonObject {
    const key = `request:${operation}:${callId}`;
    useStableId(this.usedCalls, key, callId, "operation call");
    const configured = this.overrides.requests?.[operation]?.[callId];
    if (!configured) return cloneObject(base);
    this.consumed.add(key);
    return applyRequestOverride(base, configured, key);
  }

  expected<T extends JsonObject>(checkId: string, base: T): T {
    const key = `expectation:${checkId}`;
    useStableId(this.usedChecks, key, checkId, "expectation check");
    const configured = this.overrides.expectations?.[checkId];
    if (!configured) return cloneObject(base) as T;
    this.consumed.add(key);
    return mergeObjects(base, configured.patch) as T;
  }

  assertAllConsumed(): void {
    const configured = configuredKeys(this.overrides);
    const unused = configured.filter((key) => !this.consumed.has(key));
    if (unused.length) {
      throw new Error(`Unused connector overrides: ${unused.join(", ")}`);
    }
  }

}

export function applyRequestOverride(
  base: JsonObject,
  configured: RequestOverride,
  label = "request override",
): JsonObject {
  const result = cloneObject(base);
  for (const path of configured.omit ?? []) removePath(result, path, label);
  return configured.patch ? mergeObjects(result, configured.patch) : result;
}

function removePath(
  target: JsonObject,
  path: readonly string[],
  label: string,
): void {
  if (!path.length || path.some((part) => !part)) {
    throw new Error(`${label} contains an empty omit path`);
  }
  let current: JsonObject = target;
  for (const part of path.slice(0, -1)) {
    const value = current[part];
    if (!isObject(value)) {
      throw new Error(`${label} omit path does not exist: ${path.join(".")}`);
    }
    current = value;
  }
  const leaf = path[path.length - 1]!;
  if (!Object.hasOwn(current, leaf)) {
    throw new Error(`${label} omit path does not exist: ${path.join(".")}`);
  }
  delete current[leaf];
}

function mergeObjects(base: JsonObject, patch: JsonObject): JsonObject {
  const merged = cloneObject(base);
  for (const [field, value] of Object.entries(patch)) {
    const existing = merged[field];
    merged[field] = isObject(existing) && isObject(value)
      ? mergeObjects(existing, value)
      : cloneValue(value);
  }
  return merged;
}

function cloneObject(value: JsonObject): JsonObject {
  return Object.fromEntries(
    Object.entries(value).map(([field, item]) => [field, cloneValue(item)]),
  );
}

function cloneValue(value: JsonValue): JsonValue {
  if (Array.isArray(value)) return value.map(cloneValue);
  if (isObject(value)) return cloneObject(value);
  return value;
}

function isObject(value: JsonValue | undefined): value is JsonObject {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function validateConfiguration(overrides: ScenarioOverrides): void {
  for (const [operation, calls] of Object.entries(overrides.requests ?? {})) {
    for (const [callId, configured] of Object.entries(calls ?? {})) {
      if (!callId.trim()) throw new Error(`request:${operation} has an empty call ID`);
      validateReason(configured.reason, `request:${operation}:${callId}`);
      if (!configured.patch && !(configured.omit?.length)) {
        throw new Error(`request:${operation}:${callId} has no patch or omit`);
      }
      for (const path of configured.omit ?? []) {
        if (!path.length || path.some((part) => !part)) {
          throw new Error(`request:${operation}:${callId} contains an empty omit path`);
        }
      }
      validateJson(configured.patch, `request:${operation}:${callId}.patch`);
    }
  }
  for (const [checkId, configured] of Object.entries(
    overrides.expectations ?? {},
  )) {
    validateExpectation(configured, checkId);
  }
}

function validateExpectation(
  configured: ExpectationOverride,
  checkId: string,
): void {
  validateReason(configured.reason, `expectation:${checkId}`);
  if (!checkId.trim()) throw new Error("expectation has an empty check ID");
  if (!Object.keys(configured.patch).length) {
    throw new Error(`expectation:${checkId} has an empty patch`);
  }
  validateJson(configured.patch, `expectation:${checkId}.patch`);
}

function validateReason(reason: string, label: string): void {
  if (!reason.trim()) throw new Error(`${label} requires a nonempty reason`);
}

function validateJson(value: unknown, label: string): void {
  if (value === undefined) return;
  if (value === null || ["string", "boolean"].includes(typeof value)) {
    return;
  }
  if (typeof value === "number") {
    if (!Number.isFinite(value)) throw new Error(`${label} must contain finite numbers`);
    return;
  }
  if (Array.isArray(value)) {
    value.forEach((item, index) => validateJson(item, `${label}[${index}]`));
    return;
  }
  if (typeof value === "object") {
    const prototype = Object.getPrototypeOf(value);
    if (prototype !== Object.prototype && prototype !== null) {
      throw new Error(`${label} must contain plain JSON objects only`);
    }
    Object.entries(value).forEach(([field, item]) =>
      validateJson(item, `${label}.${field}`));
    return;
  }
  throw new Error(`${label} must contain JSON data only`);
}

function useStableId(
  used: Set<string>,
  key: string,
  id: string,
  kind: string,
): void {
  if (!id.trim()) throw new Error(`${kind} ID must not be empty`);
  if (used.has(key)) throw new Error(`${kind} ID used more than once: ${id}`);
  used.add(key);
}

function configuredKeys(overrides: ScenarioOverrides): string[] {
  const keys: string[] = [];
  for (const [operation, calls] of Object.entries(overrides.requests ?? {})) {
    for (const callId of Object.keys(calls ?? {})) {
      keys.push(`request:${operation}:${callId}`);
    }
  }
  for (const checkId of Object.keys(overrides.expectations ?? {})) {
    keys.push(`expectation:${checkId}`);
  }
  return keys;
}
