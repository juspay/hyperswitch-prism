import { readFileSync } from "node:fs";

export type ConnectorCredentials = {
  connector_account_details: Record<string, unknown>;
  metadata?: Record<string, unknown>;
};

export function loadConnectorCredentials(key: string): ConnectorCredentials {
  const path = process.env.CONNECTOR_AUTH_FILE_PATH;
  if (!path) {
    throw new Error(
      "Set CONNECTOR_AUTH_FILE_PATH to a connector sandbox credentials JSON file",
    );
  }

  let document: unknown;
  try {
    document = JSON.parse(readFileSync(path, "utf8"));
  } catch {
    throw new Error(
      "Cannot read connector credentials; check that the file contains valid JSON",
    );
  }

  const entry = isObject(document) ? document[key] : undefined;
  if (
    !isObject(entry) ||
    !isObject(entry.connector_account_details) ||
    (entry.metadata !== undefined && !isObject(entry.metadata))
  ) {
    throw new Error(`Missing sandbox credentials for ${key}`);
  }
  return structuredClone({
    connector_account_details: entry.connector_account_details,
    ...(isObject(entry.metadata) ? { metadata: entry.metadata } : {}),
  });
}

function isObject(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}
