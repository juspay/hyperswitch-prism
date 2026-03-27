import { Browser, BrowserContext, Frame, Locator, Page, chromium } from "playwright";
import type { BrowserDriverFactory, BrowserSession, SessionOptions } from "./browserDriver";
import type { WaitState, WaitUntil } from "../types/dsl";

class PlaywrightSession implements BrowserSession {
  constructor(
    private readonly browser: Browser,
    private readonly context: BrowserContext,
    private readonly page: Page
  ) {}

  async goto(url: string, opts?: { timeoutMs?: number; waitUntil?: WaitUntil }): Promise<void> {
    await this.page.goto(url, {
      timeout: opts?.timeoutMs,
      waitUntil: opts?.waitUntil ?? "domcontentloaded"
    });
  }

  async click(selector: string, opts?: { timeoutMs?: number }): Promise<void> {
    const locator = await this.findFirstLocator(selector, {
      timeoutMs: opts?.timeoutMs,
      state: "visible"
    });
    await locator.click({ timeout: opts?.timeoutMs });
  }

  async fill(selector: string, value: string, opts?: { timeoutMs?: number }): Promise<void> {
    const locator = await this.findFirstLocator(selector, {
      timeoutMs: opts?.timeoutMs,
      state: "visible"
    });
    await locator.fill(value, { timeout: opts?.timeoutMs });
  }

  async press(selector: string, key: string, opts?: { timeoutMs?: number }): Promise<void> {
    const locator = await this.findFirstLocator(selector, {
      timeoutMs: opts?.timeoutMs,
      state: "visible"
    });
    await locator.press(key, { timeout: opts?.timeoutMs });
  }

  async waitForSelector(
    selector: string,
    opts?: { timeoutMs?: number; state?: WaitState }
  ): Promise<void> {
    await this.findFirstLocator(selector, {
      timeoutMs: opts?.timeoutMs,
      state: opts?.state ?? "visible"
    });
  }

  async waitForUrlContains(text: string, opts?: { timeoutMs?: number }): Promise<void> {
    await this.page.waitForURL((url) => url.toString().includes(text), {
      timeout: opts?.timeoutMs
    });
  }

  async getText(selector: string, opts?: { timeoutMs?: number }): Promise<string> {
    const locator = await this.findFirstLocator(selector, {
      timeoutMs: opts?.timeoutMs,
      state: "visible"
    });
    return locator.innerText();
  }

  async isVisible(selector: string, opts?: { timeoutMs?: number }): Promise<boolean> {
    try {
      await this.findFirstLocator(selector, {
        timeoutMs: opts?.timeoutMs,
        state: "visible"
      });
      return true;
    } catch {
      return false;
    }
  }

  async getAttribute(
    selector: string,
    attribute: string,
    opts?: { timeoutMs?: number }
  ): Promise<string | null> {
    const locator = await this.findFirstLocator(selector, {
      timeoutMs: opts?.timeoutMs,
      state: "attached"
    });
    return locator.getAttribute(attribute);
  }

  async getAllText(selector: string, opts?: { timeoutMs?: number }): Promise<string[]> {
    const locators = await this.findAllLocators(selector, {
      timeoutMs: opts?.timeoutMs,
      state: "attached"
    });

    const values: string[] = [];
    for (const locator of locators) {
      const count = await locator.count();
      for (let i = 0; i < count; i++) {
        values.push(await locator.nth(i).innerText());
      }
    }

    return values;
  }

  async getAllAttribute(
    selector: string,
    attribute: string,
    opts?: { timeoutMs?: number }
  ): Promise<(string | null)[]> {
    const locators = await this.findAllLocators(selector, {
      timeoutMs: opts?.timeoutMs,
      state: "attached"
    });

    const values: (string | null)[] = [];
    for (const locator of locators) {
      const count = await locator.count();
      for (let i = 0; i < count; i++) {
        values.push(await locator.nth(i).getAttribute(attribute));
      }
    }

    return values;
  }

  currentUrl(): string {
    return this.page.url();
  }

  async evaluate(expression: string): Promise<unknown> {
    return this.page.evaluate(expression);
  }

  async screenshot(path: string, opts?: { fullPage?: boolean }): Promise<void> {
    await this.page.screenshot({ path, fullPage: opts?.fullPage ?? true });
  }

  async close(): Promise<void> {
    await this.context.close().catch(() => undefined);
    await this.browser.close().catch(() => undefined);
  }

  private locatorContexts(): Frame[] {
    const main = this.page.mainFrame();
    const others = this.page.frames().filter((frame) => frame !== main);
    return [main, ...others];
  }

  private async findFirstLocator(
    selector: string,
    opts?: { timeoutMs?: number; state?: WaitState }
  ): Promise<Locator> {
    const state = opts?.state ?? "visible";
    const timeoutMs = opts?.timeoutMs ?? 12000;
    const deadline = Date.now() + timeoutMs;

    while (Date.now() <= deadline) {
      for (const frame of this.locatorContexts()) {
        const locator = frame.locator(selector).first();
        const count = await locator.count().catch(() => 0);

        if (state === "detached") {
          if (count === 0) {
            return locator;
          }
          continue;
        }

        if (count === 0) {
          continue;
        }

        if (state === "attached") {
          return locator;
        }

        const visible = await locator.isVisible().catch(() => false);
        if (state === "visible" && visible) {
          return locator;
        }
        if (state === "hidden" && !visible) {
          return locator;
        }
      }

      await new Promise((resolve) => setTimeout(resolve, 150));
    }

    throw new Error(`locator.waitFor: Timeout ${timeoutMs}ms exceeded.`);
  }

  private async findAllLocators(
    selector: string,
    opts?: { timeoutMs?: number; state?: WaitState }
  ): Promise<Locator[]> {
    const state = opts?.state ?? "attached";
    const timeoutMs = opts?.timeoutMs ?? 12000;
    const deadline = Date.now() + timeoutMs;

    while (Date.now() <= deadline) {
      const locators: Locator[] = [];

      for (const frame of this.locatorContexts()) {
        const locator = frame.locator(selector);
        const count = await locator.count().catch(() => 0);
        if (count === 0) {
          continue;
        }

        if (state === "visible") {
          const firstVisible = await locator.first().isVisible().catch(() => false);
          if (!firstVisible) {
            continue;
          }
        }

        locators.push(locator);
      }

      if (locators.length > 0) {
        return locators;
      }

      await new Promise((resolve) => setTimeout(resolve, 150));
    }

    throw new Error(`locator.waitFor: Timeout ${timeoutMs}ms exceeded.`);
  }
}

export class PlaywrightDriverFactory implements BrowserDriverFactory {
  async createSession(options: SessionOptions): Promise<BrowserSession> {
    const browser = await chromium.launch({
      headless: options.headless,
      slowMo: options.slowMoMs
    });
    const context = await browser.newContext({ viewport: options.viewport });
    const page = await context.newPage();

    page.setDefaultTimeout(options.defaultTimeoutMs);
    page.setDefaultNavigationTimeout(options.navigationTimeoutMs);

    return new PlaywrightSession(browser, context, page);
  }
}
