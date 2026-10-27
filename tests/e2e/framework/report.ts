import { test } from "@playwright/test";
import type { UcsAudit } from "./ucs-audit.js";

export class Report {
  constructor(private readonly audit: UcsAudit) {}

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
