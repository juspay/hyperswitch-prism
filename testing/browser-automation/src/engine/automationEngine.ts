import path from "node:path";
import type { BrowserDriverFactory, BrowserSession } from "../drivers/browserDriver";
import type { RunRequest, RunResponse, StepResult } from "../types/api";
import { executeRule } from "./interpreter";

const DEFAULT_TIMEOUT_MS = 10_000;
const DEFAULT_NAV_TIMEOUT_MS = 20_000;
const DEFAULT_VIEWPORT = { width: 1366, height: 768 };

function errorMessage(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }
  return String(error);
}

export class AutomationEngine {
  constructor(private readonly driverFactory: BrowserDriverFactory) {}

  async cleanup(): Promise<void> {
    // Close any open browser instances managed by the driver factory
    if ("cleanup" in this.driverFactory && typeof this.driverFactory.cleanup === "function") {
      await this.driverFactory.cleanup();
    }
  }

  async run(input: RunRequest): Promise<RunResponse> {
    const startedAt = Date.now();
    const steps: StepResult[] = [];
    const data: Record<string, unknown> = {};

    const headless = input.options?.headless ?? true;
    const slowMoMs = input.options?.slowMoMs ?? 0;
    const defaultTimeoutMs = input.options?.defaultTimeoutMs ?? DEFAULT_TIMEOUT_MS;
    const navigationTimeoutMs = input.options?.navigationTimeoutMs ?? DEFAULT_NAV_TIMEOUT_MS;
    const screenshotDir = input.options?.screenshotDir ?? path.join(process.cwd(), "screenshots");
    const viewport = input.options?.viewport ?? DEFAULT_VIEWPORT;

    let session: BrowserSession | undefined;

    try {
      session = await this.driverFactory.createSession({
        headless,
        slowMoMs,
        defaultTimeoutMs,
        navigationTimeoutMs,
        viewport
      });

      await session.goto(input.url, {
        timeoutMs: navigationTimeoutMs,
        waitUntil: "domcontentloaded"
      });

      for (let index = 0; index < input.rules.length; index += 1) {
        const rule = input.rules[index];
        const stepStartedAt = Date.now();

        try {
          await executeRule(session, rule, {
            requestUrl: input.url,
            stepIndex: index,
            data,
            screenshotDir
          });

          steps.push({
            index,
            action: rule.action,
            status: "ok",
            durationMs: Date.now() - stepStartedAt
          });
        } catch (error) {
          const message = errorMessage(error);
          steps.push({
            index,
            action: rule.action,
            status: "failed",
            error: message,
            durationMs: Date.now() - stepStartedAt
          });

          return {
            success: false,
            failedStep: index,
            error: message,
            data,
            finalUrl: session.currentUrl(),
            steps,
            durationMs: Date.now() - startedAt
          };
        }
      }

      return {
        success: true,
        data,
        finalUrl: session.currentUrl(),
        steps,
        durationMs: Date.now() - startedAt
      };
    } catch (error) {
      return {
        success: false,
        failedStep: steps.length > 0 ? steps[steps.length - 1].index : -1,
        error: errorMessage(error),
        data,
        finalUrl: session?.currentUrl() ?? input.url,
        steps,
        durationMs: Date.now() - startedAt
      };
    } finally {
      if (session) {
        await session.close().catch(() => undefined);
      }
    }
  }
}
