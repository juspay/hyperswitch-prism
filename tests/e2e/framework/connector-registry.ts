import { readdir } from "node:fs/promises";
import { dirname, extname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { scenarios, type ScenarioId } from "../scenarios/catalog.js";
import { isConnector, type Connector } from "./connector.js";

export async function loadConnectors(): Promise<Connector<ScenarioId>[]> {
  const connectorsDirectory = join(dirname(fileURLToPath(import.meta.url)), "../connectors");
  const runtimeExtension = extname(fileURLToPath(import.meta.url));
  const directories = (await readdir(connectorsDirectory, { withFileTypes: true }))
    .filter((entry) => entry.isDirectory())
    .map((entry) => entry.name).sort();
  const unexpectedFiles = (await readdir(connectorsDirectory, { withFileTypes: true }))
    .filter((entry) => entry.isFile() && extname(entry.name) === runtimeExtension);
  if (unexpectedFiles.length) {
    throw new Error("Connector definitions must be inside connectors/<id>/merchant_XX.ts");
  }
  const files = (await Promise.all(directories.map(async (directory) =>
    (await readdir(join(connectorsDirectory, directory), { withFileTypes: true }))
      .filter((entry) => entry.isFile() && extname(entry.name) === runtimeExtension && !entry.name.endsWith(".d.ts"))
      .map((entry) => join(directory, entry.name))
  ))).flat().sort();

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
    const variantId = `${connector.id}/${connector.merchantVariant}`;
    if (connectorIds.has(variantId)) {
      throw new Error(`Duplicate merchant variant: ${variantId}`);
    }
    connectorIds.add(variantId);

    const overridden = Object.keys(connector.overrides ?? {}) as ScenarioId[];
    const excluded = Object.keys(connector.exclude ?? {}) as ScenarioId[];
    const runnable = connector.run;
    for (const scenarioId of runnable) {
      if (!scenarioIds.has(scenarioId)) throw new Error(`${variantId} references unknown scenario ${scenarioId}`);
      if (connector.exclude?.[scenarioId]) throw new Error(`${variantId}/${scenarioId} cannot run and be excluded`);
    }
    if (new Set(runnable).size !== runnable.length) throw new Error(`${variantId} repeats a scenario`);
    for (const scenarioId of scenarioIds) {
      if (!runnable.includes(scenarioId) && !connector.exclude?.[scenarioId]) {
        throw new Error(`${variantId}/${scenarioId} must run or be excluded with a reason`);
      }
    }

    for (const scenarioId of [...overridden, ...excluded]) {
      if (!scenarioIds.has(scenarioId)) {
        throw new Error(`${connector.id} references unknown scenario ${scenarioId}`);
      }
    }

    for (const scenarioId of overridden) {
      if (!runnable.includes(scenarioId)) throw new Error(`${variantId}/${scenarioId} override requires run`);
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
