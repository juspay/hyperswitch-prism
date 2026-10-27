import { test } from "@playwright/test";
import { connectors } from "../../../../connectors/index.js";
import { runOneTimeCardScenario } from "../../../../framework/runner.js";
import {
  supports,
  type OneTimeCardScenario,
} from "../../../../framework/scenario.js";

const scenario = {
  id: "no3ds-automatic",
  title: "One-time card payment — No3DS, automatic capture",
  description:
    "Create a server-side card payment, confirm it separately, and verify successful automatic capture.",
  paymentMethod: "card",
  authentication: "no_three_ds",
  captureMethod: "automatic",
  amount: 6000,
  currency: "USD",
  expectedStatus: "succeeded",
} as const satisfies OneTimeCardScenario;

for (const connector of connectors.filter((item) => supports(item, scenario))) {
  test(`${connector.id} / ${scenario.title} @one-time @server @card`, async ({}, info) => {
    await runOneTimeCardScenario(connector, scenario, info);
  });
}
