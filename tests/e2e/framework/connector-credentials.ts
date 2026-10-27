import { readFileSync } from "node:fs";

export type ConnectorCredentials = {
  connector_account_details: Record<string, unknown>;
  metadata?: Record<string, unknown>;
  additional_merchant_data?: Record<string, unknown>;
  connector_webhook_details?: Record<string, unknown>;
};

export function loadConnectorCredentials(
  key: string,
  entryName?: string,
): ConnectorCredentials {
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

  const connector = isObject(document) ? document[key] : undefined;
  let entry: unknown = connector;
  if (isObject(connector)) {
    if (entryName !== undefined) {
      entry = connector[entryName];
    } else if (!isObject(connector.connector_account_details)) {
      const entries = Object.values(connector).filter(
        (candidate) =>
          isObject(candidate) && isObject(candidate.connector_account_details),
      );
      if (entries.length > 1) {
        throw new Error(
          `Multiple HS credential entries for ${key}; set credentialEntry in its connector definition`,
        );
      }
      entry = entries[0];
    }
  }
  if (
    !isObject(entry) ||
    !isObject(entry.connector_account_details) ||
    (entry.metadata != null && !isObject(entry.metadata)) ||
    (entry.additional_merchant_data != null &&
      !isObject(entry.additional_merchant_data)) ||
    (entry.connector_webhook_details != null &&
      !isObject(entry.connector_webhook_details))
  ) {
    throw new Error(`Missing sandbox credentials for ${key}`);
  }
  return structuredClone({
    connector_account_details: entry.connector_account_details,
    ...(isObject(entry.metadata) ? { metadata: entry.metadata } : {}),
    ...(isObject(entry.additional_merchant_data)
      ? { additional_merchant_data: entry.additional_merchant_data }
      : {}),
    ...(isObject(entry.connector_webhook_details)
      ? { connector_webhook_details: entry.connector_webhook_details }
      : {}),
  });
}

function isObject(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}
