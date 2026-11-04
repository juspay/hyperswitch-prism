import { fileURLToPath } from "node:url";
import type { ConnectorDefinition } from "./types.js";

const connectorBrand: unique symbol = Symbol("e2e.connector");

/** Constructed only from a connector definition module's own file URL. */
export type Connector<TScenarioId extends string = string> =
  ConnectorDefinition<TScenarioId> & {
    readonly [connectorBrand]: true;
  };

type ConnectorData<TScenarioId extends string> = Omit<
  ConnectorDefinition<TScenarioId>,
  "id"
>;

/** Derives the connector ID from connectors/<id>.ts. */
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
  if (relativeFile.includes("/")) {
    throw new Error("Connector definitions must be direct children of connectors");
  }
  const suffix = [".ts", ".js"].find((item) => relativeFile.endsWith(item));
  if (!suffix) {
    throw new Error("Connector definition must be a TypeScript or JavaScript module");
  }

  const id = relativeFile.slice(0, -suffix.length);
  if (!id || id === "index") {
    throw new Error("Connector definition filename must identify a connector");
  }

  return {
    id,
    ...data,
    [connectorBrand]: true,
  };
}

export function isConnector(value: unknown): value is Connector {
  return typeof value === "object"
    && value !== null
    && connectorBrand in value;
}
