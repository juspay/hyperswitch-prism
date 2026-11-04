import type { Rule, RuleAction } from "./dsl";

export interface RunOptions {
  headless?: boolean;
  slowMoMs?: number;
  defaultTimeoutMs?: number;
  navigationTimeoutMs?: number;
  viewport?: {
    width: number;
    height: number;
  };
}

export interface RunRequest {
  url: string;
  /** POST redirect fields supplied by the harness; never written to an HTML file. */
  formFields?: Record<string, string>;
  /** Completion URL intercepted locally, without contacting the callback server. */
  callbackUrl?: string;
  rules: Rule[];
  options?: RunOptions;
}

export interface StepResult {
  index: number;
  action: RuleAction;
  status: "ok" | "failed";
  durationMs: number;
  error?: string;
}

export interface RunSuccessResponse {
  success: true;
  data: Record<string, unknown>;
  finalUrl: string;
  steps: StepResult[];
  durationMs: number;
}

export interface RunFailureResponse {
  success: false;
  failedStep: number;
  error: string;
  data: Record<string, unknown>;
  finalUrl: string;
  steps: StepResult[];
  durationMs: number;
}

export type RunResponse = RunSuccessResponse | RunFailureResponse;
