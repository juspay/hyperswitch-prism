import { fileURLToPath } from "node:url";
import type { Scenario, ScenarioDefinition } from "./types.js";

/** Binds scenario construction to the one catalog supplied by its catalog module. */
export function createScenarioResolver<const T extends readonly string[]>(
  catalog: T,
): (moduleUrl: string, definition: ScenarioDefinition) => Scenario<T[number]> {
  return (moduleUrl, definition) =>
    resolveScenarioFromFile(moduleUrl, catalog, definition);
}

function resolveScenarioFromFile<const T extends readonly string[]>(
  moduleUrl: string,
  catalog: T,
  definition: ScenarioDefinition,
): Scenario<T[number]> {
  const file = fileURLToPath(moduleUrl).replaceAll("\\", "/");
  const marker = "/scenarios/";
  const markerAt = file.lastIndexOf(marker);
  if (markerAt < 0) {
    throw new Error(
      "Scenario spec must be located under the scenarios directory",
    );
  }

  const relativeFile = file.slice(markerAt + marker.length);
  const suffix = [".spec.ts", ".spec.js"].find((item) =>
    relativeFile.endsWith(item),
  );
  if (!suffix) {
    throw new Error(
      `Scenario file must end in .spec.ts or .spec.js: ${relativeFile}`,
    );
  }

  const id = relativeFile.slice(0, -suffix.length);
  if (!catalog.includes(id)) {
    throw new Error(`No catalog entry for scenario: ${id}`);
  }

  for (const field of ["title", "description", "verifies"] as const) {
    if (!definition[field].trim()) {
      throw new Error(`Scenario ${id} requires a nonempty ${field}`);
    }
  }
  return { ...definition, id } as Scenario<T[number]>;
}
