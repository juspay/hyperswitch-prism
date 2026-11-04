import { connectorFromFile } from "../framework/connector.js";
import type { ScenarioId } from "../scenarios/catalog.js";

export default connectorFromFile<ScenarioId>(import.meta.url, {
  card: {
    card_number: "4012000098765439",
    card_exp_month: "12",
    card_exp_year: "28",
    card_holder_name: "Test Customer",
    card_cvc: "999",
  },
});
