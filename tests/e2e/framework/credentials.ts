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

  let document: Record<string, unknown>;
  try {
    document = JSON.parse(readFileSync(path, "utf8"));
  } catch {
    throw new Error(
      "Cannot read connector credentials; check that the file contains valid JSON",
    );
  }

  const entry = document[key] as ConnectorCredentials | undefined;
  if (!entry?.connector_account_details) {
    throw new Error(`Missing sandbox credentials for ${key}`);
  }
  return structuredClone(entry);
}
