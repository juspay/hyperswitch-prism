import { connectorFromFile } from "../../framework/connector.js";
import type { ScenarioId } from "../../scenarios/catalog.js";

export default connectorFromFile<ScenarioId>(import.meta.url, {
  run: ["payments/create-confirm/card/one-time/no3ds/create-confirm"],
  credentialEntry: "connector_1",
});
