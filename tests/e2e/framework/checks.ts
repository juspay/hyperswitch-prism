import { expect, type TestInfo } from "@playwright/test";
import type { ExpectedValue } from "./scenario.js";
import type { Json } from "./hyperswitch.js";

export async function waitForStatus(
  name: string,
  retrieve: () => Promise<Json>,
  expected: string,
  info: TestInfo,
): Promise<Json> {
  let response: Json | undefined;
  try {
    await expect
      .poll(
        async () => {
          response = await retrieve();
          return response.status;
        },
        {
          intervals: [1000, 3000, 5000],
          timeout: 90_000,
          message: `Wait for payment status: ${expected}`,
        },
      )
      .toBe(expected);
    return response!;
  } finally {
    await info.attach(`${name} / status`, {
      body: JSON.stringify(
        { expected, actual: response?.status ?? "unavailable" },
        null,
        2,
      ),
      contentType: "application/json",
    });
  }
}

export async function verifyChecks(
  name: string,
  expectations: Record<string, ExpectedValue>,
  response: Json,
  references: Record<"connector.id" | "merchant.merchantConnectorId", string>,
  info: TestInfo,
): Promise<void> {
  const checks = Object.fromEntries(
    Object.entries(expectations).map(([field, value]) => {
      const expected =
        value !== null && typeof value === "object" && "from" in value
          ? references[value.from]
          : value;
      return [field, { expected, actual: response[field] }];
    }),
  );
  await info.attach(`${name} / checks`, {
    body: JSON.stringify(checks, null, 2),
    contentType: "application/json",
  });
  for (const [field, check] of Object.entries(checks)) {
    const expected = check.expected;
    if (
      expected !== null &&
      typeof expected === "object" &&
      "oneOf" in expected
    ) {
      expect(expected.oneOf, `${name}: ${field}`).toContain(check.actual);
    } else {
      expect(check.actual, `${name}: ${field}`).toEqual(expected);
    }
  }
}
