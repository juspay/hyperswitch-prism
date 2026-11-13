import type { RunOptions, RunRequest } from "../types/api";
import type {
  AssertTextRule,
  AssertVisibleRule,
  ClickRule,
  InitializeCashfreeSandboxRule,
  ExtractAllRule,
  ExtractRule,
  FillRule,
  GotoRule,
  PressRule,
  Rule,
  WaitForRule
} from "../types/dsl";

export class RequestValidationError extends Error {}

export const MAX_INPUT_BYTES = 1_048_576;

/** Navigation input is a destination, never a local file or browser command. */
export function navigationUrl(value: string): string {
  let url: URL;
  try {
    url = new URL(value);
  } catch {
    throw new RequestValidationError("Navigation requires an absolute HTTP(S) URL");
  }
  if (!["http:", "https:"].includes(url.protocol) || url.username || url.password) {
    throw new RequestValidationError("Only HTTP(S) URLs without embedded credentials are allowed");
  }
  return url.href;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function requiredString(obj: Record<string, unknown>, key: string, path: string): string {
  const value = obj[key];
  if (typeof value !== "string" || value.length === 0) {
    throw new RequestValidationError(`${path}.${key} must be a non-empty string`);
  }
  return value;
}

function optionalString(
  obj: Record<string, unknown>,
  key: string,
  path: string
): string | undefined {
  const value = obj[key];
  if (value === undefined) {
    return undefined;
  }
  if (typeof value !== "string") {
    throw new RequestValidationError(`${path}.${key} must be a string`);
  }
  return value;
}

function optionalNumber(
  obj: Record<string, unknown>,
  key: string,
  path: string
): number | undefined {
  const value = obj[key];
  if (value === undefined) {
    return undefined;
  }
  if (typeof value !== "number" || !Number.isFinite(value) || value <= 0 || value > 90_000) {
    throw new RequestValidationError(`${path}.${key} must be a positive number up to 90000`);
  }
  return value;
}

function optionalNonNegativeNumber(
  obj: Record<string, unknown>,
  key: string,
  path: string
): number | undefined {
  const value = obj[key];
  if (value === undefined) {
    return undefined;
  }
  if (typeof value !== "number" || !Number.isFinite(value) || value < 0) {
    throw new RequestValidationError(`${path}.${key} must be a non-negative number`);
  }
  return value;
}

function optionalBoolean(
  obj: Record<string, unknown>,
  key: string,
  path: string
): boolean | undefined {
  const value = obj[key];
  if (value === undefined) {
    return undefined;
  }
  if (typeof value !== "boolean") {
    throw new RequestValidationError(`${path}.${key} must be a boolean`);
  }
  return value;
}

function parseRule(rawRule: unknown, index: number): Rule {
  const path = `rules[${index}]`;

  if (!isRecord(rawRule)) {
    throw new RequestValidationError(`${path} must be an object`);
  }

  const action = requiredString(rawRule, "action", path);

  switch (action) {
    case "goto": {
      // Validate waitUntil enum
      const waitUntil = rawRule.waitUntil;
      if (waitUntil !== undefined) {
        const validWaitUntil = ["load", "domcontentloaded", "networkidle", "commit"];
        if (typeof waitUntil !== "string" || !validWaitUntil.includes(waitUntil)) {
          throw new RequestValidationError(
            `${path}.waitUntil must be one of: ${validWaitUntil.join(", ")}`
          );
        }
      }

      const rule: GotoRule = {
        action: "goto",
        url: rawRule.url === undefined ? undefined : navigationUrl(requiredString(rawRule, "url", path)),
        waitUntil: waitUntil as GotoRule["waitUntil"],
        timeoutMs: optionalNumber(rawRule, "timeoutMs", path)
      };
      return rule;
    }

    case "click": {
      const rule: ClickRule = {
        action: "click",
        selector: requiredString(rawRule, "selector", path),
        timeoutMs: optionalNumber(rawRule, "timeoutMs", path)
      };
      return rule;
    }

    case "fill": {
      const rule: FillRule = {
        action: "fill",
        selector: requiredString(rawRule, "selector", path),
        value: requiredString(rawRule, "value", path),
        timeoutMs: optionalNumber(rawRule, "timeoutMs", path)
      };
      return rule;
    }

    case "press": {
      const rule: PressRule = {
        action: "press",
        selector: requiredString(rawRule, "selector", path),
        key: requiredString(rawRule, "key", path),
        timeoutMs: optionalNumber(rawRule, "timeoutMs", path)
      };
      return rule;
    }

    case "waitFor": {
      const selector = optionalString(rawRule, "selector", path);
      const urlContains = optionalString(rawRule, "urlContains", path);

      if (!selector && !urlContains) {
        throw new RequestValidationError(`${path} waitFor requires selector or urlContains`);
      }

      // Validate state enum
      const state = rawRule.state;
      if (state !== undefined) {
        const validStates = ["attached", "detached", "visible", "hidden"];
        if (typeof state !== "string" || !validStates.includes(state)) {
          throw new RequestValidationError(
            `${path}.state must be one of: ${validStates.join(", ")}`
          );
        }
      }

      const rule: WaitForRule = {
        action: "waitFor",
        selector,
        urlContains,
        state: state as WaitForRule["state"],
        timeoutMs: optionalNumber(rawRule, "timeoutMs", path)
      };
      return rule;
    }

    case "assertText": {
      // Validate match enum
      const match = rawRule.match;
      if (match !== undefined) {
        const validMatches = ["equals", "contains"];
        if (typeof match !== "string" || !validMatches.includes(match)) {
          throw new RequestValidationError(
            `${path}.match must be one of: ${validMatches.join(", ")}`
          );
        }
      }

      const rule: AssertTextRule = {
        action: "assertText",
        selector: requiredString(rawRule, "selector", path),
        text: requiredString(rawRule, "text", path),
        match: match as AssertTextRule["match"],
        caseSensitive: optionalBoolean(rawRule, "caseSensitive", path),
        timeoutMs: optionalNumber(rawRule, "timeoutMs", path)
      };
      return rule;
    }

    case "assertVisible": {
      const rule: AssertVisibleRule = {
        action: "assertVisible",
        selector: requiredString(rawRule, "selector", path),
        timeoutMs: optionalNumber(rawRule, "timeoutMs", path)
      };
      return rule;
    }

    case "extract": {
      const rule: ExtractRule = {
        action: "extract",
        selector: requiredString(rawRule, "selector", path),
        as: requiredString(rawRule, "as", path),
        attribute: optionalString(rawRule, "attribute", path),
        trim: optionalBoolean(rawRule, "trim", path),
        timeoutMs: optionalNumber(rawRule, "timeoutMs", path)
      };
      return rule;
    }

    case "extractAll": {
      const rule: ExtractAllRule = {
        action: "extractAll",
        selector: requiredString(rawRule, "selector", path),
        as: requiredString(rawRule, "as", path),
        attribute: optionalString(rawRule, "attribute", path),
        trim: optionalBoolean(rawRule, "trim", path),
        timeoutMs: optionalNumber(rawRule, "timeoutMs", path)
      };
      return rule;
    }

    case "initializeCashfreeSandbox": {
      const rule: InitializeCashfreeSandboxRule = {
        action: "initializeCashfreeSandbox",
        timeoutMs: optionalNumber(rawRule, "timeoutMs", path)
      };
      return rule;
    }

    default:
      throw new RequestValidationError(`${path}.action \"${action}\" is not supported`);
  }
}

function parseOptions(raw: unknown): RunOptions | undefined {
  if (raw === undefined) {
    return undefined;
  }

  if (!isRecord(raw)) {
    throw new RequestValidationError("options must be an object");
  }

  if (Object.keys(raw).some(key => ![
    "headless", "slowMoMs", "defaultTimeoutMs", "navigationTimeoutMs", "viewport"
  ].includes(key))) {
    throw new RequestValidationError("Unsupported browser option");
  }
  if (raw.headless === false) {
    throw new RequestValidationError("The isolated worker only supports headless execution");
  }
  for (const key of ["defaultTimeoutMs", "navigationTimeoutMs", "slowMoMs"]) {
    if (typeof raw[key] === "number" && raw[key] > 90_000) {
      throw new RequestValidationError("Browser timeouts and delays cannot exceed 90000 ms");
    }
  }

  let viewport: RunOptions["viewport"];
  if (raw.viewport !== undefined) {
    if (!isRecord(raw.viewport)) {
      throw new RequestValidationError("options.viewport must be an object");
    }

    const width = raw.viewport.width;
    const height = raw.viewport.height;
    if (!Number.isInteger(width) || !Number.isInteger(height) ||
        (width as number) < 1 || (height as number) < 1 ||
        (width as number) > 1920 || (height as number) > 1080) {
      throw new RequestValidationError("Viewport must be positive integers up to 1920 x 1080");
    }

    viewport = { width: width as number, height: height as number };
  }

  return {
    headless: optionalBoolean(raw, "headless", "options"),
    slowMoMs: optionalNonNegativeNumber(raw, "slowMoMs", "options"),
    defaultTimeoutMs: optionalNumber(raw, "defaultTimeoutMs", "options"),
    navigationTimeoutMs: optionalNumber(raw, "navigationTimeoutMs", "options"),
    viewport
  };
}

export function parseRunRequest(body: unknown): RunRequest {
  if (!isRecord(body)) {
    throw new RequestValidationError("body must be an object");
  }

  if (Object.keys(body).some(key => !["url", "rules", "options", "formFields", "callbackUrl"].includes(key))) {
    throw new RequestValidationError("Unsupported job field");
  }
  const url = navigationUrl(requiredString(body, "url", "body"));

  const rulesRaw = body.rules;
  if (!Array.isArray(rulesRaw) || rulesRaw.length > 100) {
    throw new RequestValidationError("body.rules must be an array of at most 100 rules");
  }

  let formFields: Record<string, string> | undefined;
  if (body.formFields !== undefined) {
    if (!isRecord(body.formFields) || Object.keys(body.formFields).length > 100 ||
        Object.values(body.formFields).some(value => typeof value !== "string")) {
      throw new RequestValidationError("formFields must contain at most 100 string fields");
    }
    formFields = body.formFields as Record<string, string>;
  }

  const rules = rulesRaw.map((rule, index) => parseRule(rule, index));

  return {
    url,
    formFields,
    callbackUrl: body.callbackUrl === undefined ? undefined : navigationUrl(requiredString(body, "callbackUrl", "body")),
    rules,
    options: parseOptions(body.options)
  };
}
