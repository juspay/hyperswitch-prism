import { connectorFromFile } from "../../framework/connector.js";
import type { ScenarioId } from "../../scenarios/catalog.js";

export default connectorFromFile<ScenarioId>(import.meta.url, {
  run: ["payments/create-confirm/card/one-time/no3ds/create-confirm"],
  card: {
    card_number: "4242424242424242",
    card_exp_month: "06",
    card_exp_year: "28",
    card_cvc: "123",
  },
});
