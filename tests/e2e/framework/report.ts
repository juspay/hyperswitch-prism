import { test, type TestInfo } from "@playwright/test";
import type { UcsAudit } from "./ucs-audit.js";

export class Report {
  constructor(
    private readonly audit: UcsAudit,
    private readonly info: TestInfo,
  ) {}

  // Callers select report-safe values; native Playwright assertions own matching.
  async check<A, E>(
    name: string,
    comparison: { actual: A; expected: E },
    assertion: (values: { actual: A; expected: E }) => void | Promise<void>,
  ): Promise<void> {
    await test.step(name, async () => {
      try {
        await assertion(comparison);
      } finally {
        await this.info.attach(name, {
          body: JSON.stringify(
            comparison,
            (_key, value) => value === undefined ? "<missing field>" : value,
            2,
          ),
          contentType: "application/json",
        });
      }
    });
  }

  // The scenario owns the call and checks. Evidence follows them, even on failure.
  async step<T>(name: string, body: () => Promise<T>): Promise<T> {
    return test.step(name, async () => {
      try {
        return await body();
      } finally {
        await this.audit.attachPending();
      }
    });
  }
}
