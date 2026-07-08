export type WaitState = "attached" | "detached" | "visible" | "hidden";
export type WaitUntil = "load" | "domcontentloaded" | "networkidle" | "commit";

export type RuleAction =
  | "goto"
  | "click"
  | "fill"
  | "press"
  | "waitFor"
  | "assertText"
  | "assertVisible"
  | "extract"
  | "extractAll"
  | "screenshot"
  | "evaluate";

interface BaseRule {
  action: RuleAction;
  timeoutMs?: number;
}

export interface GotoRule extends BaseRule {
  action: "goto";
  url?: string;
  waitUntil?: WaitUntil;
}

export interface ClickRule extends BaseRule {
  action: "click";
  selector: string;
}

export interface FillRule extends BaseRule {
  action: "fill";
  selector: string;
  value: string;
}

export interface PressRule extends BaseRule {
  action: "press";
  selector: string;
  key: string;
}

export interface WaitForRule extends BaseRule {
  action: "waitFor";
  selector?: string;
  state?: WaitState;
  urlContains?: string;
}

export interface AssertTextRule extends BaseRule {
  action: "assertText";
  selector: string;
  text: string;
  match?: "contains" | "equals";
  caseSensitive?: boolean;
}

export interface AssertVisibleRule extends BaseRule {
  action: "assertVisible";
  selector: string;
}

export interface ExtractRule extends BaseRule {
  action: "extract";
  selector: string;
  as: string;
  attribute?: string;
  trim?: boolean;
}

export interface ExtractAllRule extends BaseRule {
  action: "extractAll";
  selector: string;
  as: string;
  attribute?: string;
  trim?: boolean;
}

export interface ScreenshotRule extends BaseRule {
  action: "screenshot";
  path?: string;
  fullPage?: boolean;
  as?: string;
}

export interface EvaluateRule extends BaseRule {
  action: "evaluate";
  /**
   * JavaScript expression to evaluate in page context. Must return a serializable value.
   *
   * **SECURITY WARNING**: This action executes arbitrary JavaScript in the browser page context.
   * If the automation engine server is accessible to untrusted clients (e.g., listening on 0.0.0.0),
   * implement authentication, authorization, or expression allowlisting to prevent malicious code execution.
   */
  expression: string;
  /** Key to store the result under in the data bag. If omitted, result is discarded. */
  as?: string;
}

export type Rule =
  | GotoRule
  | ClickRule
  | FillRule
  | PressRule
  | WaitForRule
  | AssertTextRule
  | AssertVisibleRule
  | ExtractRule
  | ExtractAllRule
  | ScreenshotRule
  | EvaluateRule;
