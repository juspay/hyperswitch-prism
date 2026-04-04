import type { WaitState, WaitUntil } from "../types/dsl";

export interface SessionOptions {
  headless: boolean;
  slowMoMs: number;
  defaultTimeoutMs: number;
  navigationTimeoutMs: number;
  viewport: { width: number; height: number };
}

export interface BrowserSession {
  goto(url: string, opts?: { timeoutMs?: number; waitUntil?: WaitUntil }): Promise<void>;
  click(selector: string, opts?: { timeoutMs?: number }): Promise<void>;
  fill(selector: string, value: string, opts?: { timeoutMs?: number }): Promise<void>;
  press(selector: string, key: string, opts?: { timeoutMs?: number }): Promise<void>;
  waitForSelector(
    selector: string,
    opts?: { timeoutMs?: number; state?: WaitState }
  ): Promise<void>;
  waitForUrlContains(text: string, opts?: { timeoutMs?: number }): Promise<void>;
  getText(selector: string, opts?: { timeoutMs?: number }): Promise<string>;
  isVisible(selector: string, opts?: { timeoutMs?: number }): Promise<boolean>;
  getAttribute(
    selector: string,
    attribute: string,
    opts?: { timeoutMs?: number }
  ): Promise<string | null>;
  getAllText(selector: string, opts?: { timeoutMs?: number }): Promise<string[]>;
  getAllAttribute(
    selector: string,
    attribute: string,
    opts?: { timeoutMs?: number }
  ): Promise<(string | null)[]>;
  currentUrl(): string;
  evaluate(expression: string): Promise<unknown>;
  screenshot(path: string, opts?: { fullPage?: boolean }): Promise<void>;
  close(): Promise<void>;
}

export interface BrowserDriverFactory {
  createSession(options: SessionOptions): Promise<BrowserSession>;
}
