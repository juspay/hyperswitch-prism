import type { RunOptions, RunRequest } from "../types/api";
import type {
  AssertTextRule,
  AssertVisibleRule,
  ClickRule,
  EvaluateRule,
  ExtractAllRule,
  ExtractRule,
  FillRule,
  GotoRule,
  PressRule,
  Rule,
  ScreenshotRule,
  WaitForRule
} from "../types/dsl";

export class RequestValidationError extends Error {}

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
  if (typeof value !== "number" || !Number.isFinite(value) || value <= 0) {
    throw new RequestValidationError(`${path}.${key} must be a positive number`);
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
      const rule: GotoRule = {
        action: "goto",
        url: optionalString(rawRule, "url", path),
        waitUntil: rawRule.waitUntil as GotoRule["waitUntil"],
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

      const rule: WaitForRule = {
        action: "waitFor",
        selector,
        urlContains,
        state: rawRule.state as WaitForRule["state"],
        timeoutMs: optionalNumber(rawRule, "timeoutMs", path)
      };
      return rule;
    }

    case "assertText": {
      const rule: AssertTextRule = {
        action: "assertText",
        selector: requiredString(rawRule, "selector", path),
        text: requiredString(rawRule, "text", path),
        match: rawRule.match as AssertTextRule["match"],
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

    case "screenshot": {
      const rule: ScreenshotRule = {
        action: "screenshot",
        path: optionalString(rawRule, "path", path),
        as: optionalString(rawRule, "as", path),
        fullPage: optionalBoolean(rawRule, "fullPage", path),
        timeoutMs: optionalNumber(rawRule, "timeoutMs", path)
      };
      return rule;
    }

    case "evaluate": {
      const rule: EvaluateRule = {
        action: "evaluate",
        expression: requiredString(rawRule, "expression", path),
        as: optionalString(rawRule, "as", path),
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

  let viewport: RunOptions["viewport"];
  if (raw.viewport !== undefined) {
    if (!isRecord(raw.viewport)) {
      throw new RequestValidationError("options.viewport must be an object");
    }

    const width = raw.viewport.width;
    const height = raw.viewport.height;
    if (typeof width !== "number" || typeof height !== "number") {
      throw new RequestValidationError("options.viewport.width and height must be numbers");
    }

    viewport = { width, height };
  }

  return {
    headless: optionalBoolean(raw, "headless", "options"),
    slowMoMs: optionalNonNegativeNumber(raw, "slowMoMs", "options"),
    defaultTimeoutMs: optionalNumber(raw, "defaultTimeoutMs", "options"),
    navigationTimeoutMs: optionalNumber(raw, "navigationTimeoutMs", "options"),
    screenshotDir: optionalString(raw, "screenshotDir", "options"),
    viewport
  };
}

export function parseRunRequest(body: unknown): RunRequest {
  if (!isRecord(body)) {
    throw new RequestValidationError("body must be an object");
  }

  const url = requiredString(body, "url", "body");

  const rulesRaw = body.rules;
  if (!Array.isArray(rulesRaw)) {
    throw new RequestValidationError("body.rules must be an array");
  }

  const rules = rulesRaw.map((rule, index) => parseRule(rule, index));

  return {
    url,
    rules,
    options: parseOptions(body.options)
  };
}
