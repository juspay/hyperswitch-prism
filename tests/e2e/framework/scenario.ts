import { fileURLToPath } from "node:url";
import type { Scenario, ScenarioDefinition } from "./types.js";

type ScenarioCatalog = Record<string, ScenarioDefinition>;

/** Defines implemented scenario metadata keyed by canonical scenario path. */
export function defineScenarios<const T extends ScenarioCatalog>(catalog: T): T {
  return catalog;
}

/** Binds scenario construction to the one catalog supplied by its catalog module. */
export function createScenarioResolver<const T extends ScenarioCatalog>(
  catalog: T,
): (moduleUrl: string) => Scenario<Extract<keyof T, string>> {
  return (moduleUrl) => resolveScenarioFromFile(moduleUrl, catalog);
}

function resolveScenarioFromFile<const T extends ScenarioCatalog>(
  moduleUrl: string,
  catalog: T,
): Scenario<Extract<keyof T, string>> {
  const file = fileURLToPath(moduleUrl).replaceAll("\\", "/");
  const marker = "/scenarios/";
  const markerAt = file.lastIndexOf(marker);
  if (markerAt < 0) {
    throw new Error(`Scenario spec is outside the scenarios directory: ${file}`);
  }

  const relativeFile = file.slice(markerAt + marker.length);
  const suffix = [".spec.ts", ".spec.js"].find((item) =>
    relativeFile.endsWith(item)
  );
  if (!suffix) {
    throw new Error(`Scenario file must end in .spec.ts or .spec.js: ${relativeFile}`);
  }

  const id = relativeFile.slice(0, -suffix.length);
  if (!Object.hasOwn(catalog, id)) {
    throw new Error(`No catalog entry for scenario: ${id}`);
  }

  return { id, ...catalog[id] } as Scenario<Extract<keyof T, string>>;
}
