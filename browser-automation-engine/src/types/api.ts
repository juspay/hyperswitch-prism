import type { Rule, RuleAction } from "./dsl";

export interface RunOptions {
  headless?: boolean;
  slowMoMs?: number;
  defaultTimeoutMs?: number;
  navigationTimeoutMs?: number;
  screenshotDir?: string;
  viewport?: {
    width: number;
    height: number;
  };
}

export interface RunRequest {
  url: string;
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
