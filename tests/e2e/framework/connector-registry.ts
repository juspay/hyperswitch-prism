import { readdir } from "node:fs/promises";
import { dirname, extname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { scenarios, type ScenarioId } from "../scenarios/catalog.js";
import { isConnector, type Connector } from "./connector.js";

export async function loadConnectors(): Promise<Connector<ScenarioId>[]> {
  const connectorsDirectory = join(dirname(fileURLToPath(import.meta.url)), "../connectors");
  const runtimeExtension = extname(fileURLToPath(import.meta.url));
  const files = (await readdir(connectorsDirectory, { withFileTypes: true }))
    .filter((entry) =>
      entry.isFile()
      && extname(entry.name) === runtimeExtension
      && !entry.name.endsWith(".d.ts")
    )
    .map((entry) => entry.name)
    .sort();

  const connectors = await Promise.all(files.map(async (file) => {
    const module: unknown = await import(pathToFileURL(join(connectorsDirectory, file)).href);
    const definition = isObject(module) ? module.default : undefined;
    if (!isConnector(definition)) {
      throw new Error(`Connector module must default-export connectorFromFile(): ${file}`);
    }
    return definition as Connector<ScenarioId>;
  }));

  validateConnectorScenarios(connectors);
  return connectors;
}

function validateConnectorScenarios(
  definitions: readonly Connector<ScenarioId>[],
): void {
  const scenarioIds = new Set<ScenarioId>(Object.keys(scenarios) as ScenarioId[]);
  const connectorIds = new Set<string>();

  for (const connector of definitions) {
    if (connectorIds.has(connector.id)) {
      throw new Error(`Duplicate connector definition: ${connector.id}`);
    }
    connectorIds.add(connector.id);

    const overridden = Object.keys(connector.overrides ?? {}) as ScenarioId[];
    const excluded = Object.keys(connector.exclude ?? {}) as ScenarioId[];

    for (const scenarioId of [...overridden, ...excluded]) {
      if (!scenarioIds.has(scenarioId)) {
        throw new Error(`${connector.id} references unknown scenario ${scenarioId}`);
      }
    }

    for (const scenarioId of overridden) {
      if (connector.exclude?.[scenarioId]) {
        throw new Error(
          `${connector.id}/${scenarioId} cannot be both overridden and excluded`,
        );
      }
    }

    for (const [scenarioId, exclusion] of Object.entries(
      connector.exclude ?? {},
    )) {
      if (!exclusion.reason.trim()) {
        throw new Error(
          `${connector.id}/${scenarioId} exclusion requires a reason`,
        );
      }
    }
  }
}

function isObject(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object";
}
