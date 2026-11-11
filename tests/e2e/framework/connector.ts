import { fileURLToPath } from "node:url";
import type { ConnectorDefinition } from "./types.js";

const connectorBrand: unique symbol = Symbol("e2e.connector");

/** Constructed only from a connector definition module's own file URL. */
export type Connector<TScenarioId extends string = string> =
  ConnectorDefinition<TScenarioId> & {
    readonly merchantVariant: string;
    readonly [connectorBrand]: true;
  };

type ConnectorData<TScenarioId extends string> = Omit<
  ConnectorDefinition<TScenarioId>,
  "id"
>;

/** Derives the connector and synthetic merchant variant from connectors/<id>/merchant_XX.ts. */
export function connectorFromFile<TScenarioId extends string>(
  moduleUrl: string,
  data: ConnectorData<TScenarioId>,
): Connector<TScenarioId> {
  const file = fileURLToPath(moduleUrl).replaceAll("\\", "/");
  const marker = "/connectors/";
  const markerAt = file.lastIndexOf(marker);
  if (markerAt < 0) {
    throw new Error("Connector definition must be located under connectors");
  }

  const relativeFile = file.slice(markerAt + marker.length);
  const parts = relativeFile.split("/");
  if (parts.length !== 2 || !parts[0]) {
    throw new Error("Connector definition must be connectors/<id>/merchant_XX.ts");
  }
  const suffix = [".ts", ".js"].find((item) => relativeFile.endsWith(item));
  if (!suffix) {
    throw new Error("Connector definition must be a TypeScript or JavaScript module");
  }

  const merchantVariant = parts[1].slice(0, -suffix.length);
  if (!/^merchant_[0-9]{2}$/.test(merchantVariant)) {
    throw new Error("Merchant variant filename must be merchant_XX.ts");
  }

  return {
    id: parts[0],
    merchantVariant,
    ...data,
    [connectorBrand]: true,
  };
}

export function isConnector(value: unknown): value is Connector {
  return typeof value === "object"
    && value !== null
    && connectorBrand in value;
}
