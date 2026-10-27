import { expect, test, type TestInfo } from "@playwright/test";
import type { Json } from "./hyperswitch.js";

export class Checks {
  constructor(private readonly info: TestInfo) {}

  async fields(name: string, actual: Json, expected: Json): Promise<void> {
    const comparisons = Object.fromEntries(
      Object.entries(expected).map(([field, value]) => [
        field,
        {
          expected: value,
          actual: Object.hasOwn(actual, field)
            ? actual[field]
            : "<missing field>",
        },
      ]),
    );
    await this.attach(name, comparisons);
    await test.step(name, async () => {
      for (const [field, value] of Object.entries(expected)) {
        expect(actual[field], `${name}: ${field}`).toEqual(value);
      }
    });
  }

  async oneOf(
    name: string,
    actual: string,
    expected: readonly string[],
  ): Promise<void> {
    await this.attach(name, { expected, actual });
    await test.step(name, async () => {
      expect(expected, name).toContain(actual);
    });
  }

  async waitForStatus(
    name: string,
    retrieve: () => Promise<Json>,
    expected: string,
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
            message: name,
          },
        )
        .toBe(expected);
      return response!;
    } finally {
      await this.attach(name, {
        expected,
        actual: response?.status ?? "unavailable",
      });
    }
  }

  private async attach(name: string, comparisons: object): Promise<void> {
    await this.info.attach(name, {
      body: JSON.stringify(comparisons, null, 2),
      contentType: "application/json",
    });
  }
}
