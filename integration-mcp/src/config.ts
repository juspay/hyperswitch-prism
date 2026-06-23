/**
 * Helpers for building and validating the per-connector credential config object,
 * shared by connector_requirements, generate_config, validate_config and scaffold.
 *
 * Shape (per the SDK): { connectorConfig: { [machineName]: { <field>: value } } }
 * where secret fields wrap as { value: "..." } and plain fields are bare.
 */
import type { Connector, ConnectorField } from "./data/connectors.js";
import { envVarName } from "./data/connectors.js";

/** A template value for a field: secret -> { value: placeholder }, plain -> placeholder. */
export function fieldTemplateValue(field: ConnectorField, raw: string): unknown {
  if (field.type === "string[]") return field.secret ? [{ value: raw }] : [raw];
  if (field.type === "boolean") return false;
  return field.secret ? { value: raw } : raw;
}

/** Build the inner per-connector config object (required fields only by default). */
export function buildConfigObject(
  connector: Connector,
  values: Record<string, string> | undefined,
  opts: { includeOptional?: boolean; placeholderStyle?: "env" | "angle" } = {},
): Record<string, unknown> {
  const placeholderStyle = opts.placeholderStyle ?? "angle";
  const inner: Record<string, unknown> = {};
  for (const field of connector.fields) {
    if (field.isBaseUrl) continue;
    if (!field.required && !opts.includeOptional) continue;
    const provided = values?.[field.name];
    const placeholder =
      placeholderStyle === "env"
        ? `process.env.${envVarName(connector.connector, field)}`
        : `<${field.name}>`;
    const raw = provided ?? placeholder;
    inner[field.name] = fieldTemplateValue(field, raw);
  }
  return inner;
}

export interface ValidationIssue {
  field: string;
  problem: string;
  fix: string;
}

/**
 * Normalize an arbitrary candidate into the inner per-connector object.
 * Accepts either { connectorConfig: { name: {...} } }, { name: {...} }, or the bare inner object.
 */
export function extractInner(connector: string, candidate: unknown): Record<string, unknown> | null {
  if (candidate == null || typeof candidate !== "object") return null;
  const obj = candidate as Record<string, unknown>;
  // Full shape: { connectorConfig: { <name>: {...} } }
  if (obj.connectorConfig && typeof obj.connectorConfig === "object") {
    const cc = obj.connectorConfig as Record<string, unknown>;
    const byName = cc[connector];
    if (byName && typeof byName === "object") return byName as Record<string, unknown>;
    // single-key connectorConfig
    const keys = Object.keys(cc);
    if (keys.length === 1) {
      const only = cc[keys[0]!];
      if (only && typeof only === "object") return only as Record<string, unknown>;
    }
    return null;
  }
  // { <name>: {...} }
  if (obj[connector] && typeof obj[connector] === "object") {
    return obj[connector] as Record<string, unknown>;
  }
  // bare inner object
  return obj;
}

/** Validate an inner per-connector config object against the connector's field spec. */
export function validateInner(
  connector: Connector,
  inner: Record<string, unknown>,
): ValidationIssue[] {
  const issues: ValidationIssue[] = [];
  const knownFields = new Map(connector.fields.map((f) => [f.name, f]));

  for (const field of connector.fields) {
    if (field.isBaseUrl) continue;
    const present = Object.prototype.hasOwnProperty.call(inner, field.name);
    if (!present) {
      if (field.required) {
        issues.push({
          field: field.name,
          problem: "Required field is missing.",
          fix: field.secret
            ? `Add ${field.name}: { value: "<secret>" }`
            : `Add ${field.name}: <value>`,
        });
      }
      continue;
    }
    const value = inner[field.name];
    if (field.secret) {
      // Must be { value: string }.
      if (
        value == null ||
        typeof value !== "object" ||
        Array.isArray(value) ||
        typeof (value as Record<string, unknown>).value !== "string"
      ) {
        issues.push({
          field: field.name,
          problem: "Secret field must be wrapped as an object { value: \"...\" } (SecretString).",
          fix: `Change ${field.name} to ${field.name}: { value: "<secret>" }`,
        });
      } else if (((value as Record<string, unknown>).value as string).trim() === "") {
        issues.push({
          field: field.name,
          problem: "Secret value is empty.",
          fix: `Provide a non-empty value for ${field.name}.value`,
        });
      }
    } else if (field.type === "boolean") {
      if (typeof value !== "boolean") {
        issues.push({ field: field.name, problem: "Expected a boolean.", fix: `Set ${field.name} to true/false.` });
      }
    } else if (field.type === "string[]") {
      if (!Array.isArray(value)) {
        issues.push({ field: field.name, problem: "Expected an array.", fix: `Set ${field.name} to an array.` });
      }
    } else if (typeof value !== "string") {
      issues.push({ field: field.name, problem: "Expected a string.", fix: `Set ${field.name} to a string.` });
    }
  }

  // Unknown keys.
  for (const key of Object.keys(inner)) {
    if (!knownFields.has(key)) {
      issues.push({
        field: key,
        problem: "Unknown field for this connector.",
        fix: `Remove '${key}'. Run prism_connector_requirements for the valid field list.`,
      });
    }
  }
  return issues;
}
