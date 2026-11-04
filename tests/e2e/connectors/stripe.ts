import { connectorFromFile } from "../framework/connector.js";
import type { ScenarioId } from "../scenarios/catalog.js";

export default connectorFromFile<ScenarioId>(import.meta.url, {
  credentialEntry: "connector_1",
});
