import { adyen } from "./adyen.js";
import { fiservcommercehub } from "./fiservcommercehub.js";
import { scenarios } from "../scenarios/catalog.js";
import type { ConnectorDefinition } from "../framework/types.js";
import type { ScenarioId } from "../scenarios/catalog.js";

export const connectors = [adyen, fiservcommercehub] as const;

validateConnectorScenarios(connectors);

function validateConnectorScenarios(
  definitions: readonly ConnectorDefinition<ScenarioId>[],
): void {
  const scenarioIds = new Set<ScenarioId>(
    Object.values(scenarios).map((scenario) => scenario.id),
  );

  for (const connector of definitions) {
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
