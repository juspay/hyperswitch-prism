import { Browser, BrowserContext, Frame, Locator, Page, chromium } from "playwright";
import type { BrowserDriverFactory, BrowserSession, SessionOptions } from "./browserDriver";
import type { WaitState, WaitUntil } from "../types/dsl";
import { navigationUrl } from "../utils/validation";
import { startNetworkGuard } from "../utils/networkGuard";

class PlaywrightSession implements BrowserSession {
  constructor(
    private readonly browser: Browser,
    private readonly context: BrowserContext,
    private readonly page: Page,
    private readonly closeNetworkGuard: () => Promise<void>,
    private readonly networkBlocked: () => boolean
  ) {}

  async goto(url: string, opts?: { timeoutMs?: number; waitUntil?: WaitUntil }): Promise<void> {
    await this.page.goto(navigationUrl(url), {
      timeout: opts?.timeoutMs,
      waitUntil: opts?.waitUntil ?? "domcontentloaded"
    });
  }

  async submitForm(url: string, fields: Record<string, string>, timeoutMs: number): Promise<void> {
    const endpoint = navigationUrl(url);
    await this.page.setContent("<!doctype html><html><body></body></html>");
    await Promise.all([
      this.page.waitForNavigation({ waitUntil: "domcontentloaded", timeout: timeoutMs }),
      this.page.evaluate(({ endpoint, fields }) => {
        const form = document.createElement("form");
        form.method = "POST";
        form.action = endpoint;
        for (const [name, value] of Object.entries(fields)) {
          const input = document.createElement("input");
          input.type = "hidden";
          input.name = name;
          input.value = value;
          form.appendChild(input);
        }
        document.body.appendChild(form);
        // A field named "submit" must not shadow the method.
        HTMLFormElement.prototype.submit.call(form);
      }, { endpoint, fields })
    ]);
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
    } catch (error) {
      // Only catch timeout errors - element not visible is expected
      // Re-throw other errors like page crashes, frame detachment
      if (error instanceof Error && error.message.includes("Timeout")) {
        return false;
      }
      throw error;
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

  assertNetworkPolicy(): void {
    if (this.networkBlocked()) throw new Error("Browser job attempted a blocked network destination");
  }

  async initializeCashfreeSandbox(): Promise<void> {
    const url = new URL(this.page.url());
    if (url.protocol !== "https:" || url.hostname !== "sandbox.cashfree.com") {
      throw new Error("Cashfree initialization is restricted to its sandbox origin");
    }
    await this.page.evaluate(() => {
      const pageWindow = window as unknown as { loadPage?: () => void };
      if (typeof pageWindow.loadPage === "function") pageWindow.loadPage();
    });
  }

  async close(): Promise<void> {
    await this.context.close().catch(() => undefined);
    await this.browser.close().catch(() => undefined);
    await this.closeNetworkGuard();
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

        // Handle all four states like findFirstLocator does
        if (state === "visible") {
          const firstVisible = await locator.first().isVisible().catch(() => false);
          if (!firstVisible) {
            continue;
          }
        } else if (state === "hidden") {
          const firstHidden = await locator.first().isHidden().catch(() => false);
          if (!firstHidden) {
            continue;
          }
        } else if (state === "detached") {
          // For detached state, check that count is 0 (already handled above)
          // If we got here, element exists, so skip this frame
          continue;
        }
        // "attached" state: element exists (count > 0), which we already verified

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
  private readonly sessions = new Set<BrowserSession>();

  async cleanup(): Promise<void> {
    await Promise.all([...this.sessions].map(session => session.close()));
    this.sessions.clear();
  }

  async createSession(options: SessionOptions): Promise<BrowserSession> {
    const guard = await startNetworkGuard();
    let browser: Browser | undefined;
    try {
      browser = await chromium.launch({
        headless: true,
        chromiumSandbox: true,
        slowMo: options.slowMoMs,
        proxy: { server: guard.server, bypass: "<-loopback>" },
        args: ["--force-webrtc-ip-handling-policy=disable_non_proxied_udp"]
      });
      const context = await browser.newContext({
        viewport: options.viewport,
        serviceWorkers: "block",
        acceptDownloads: false
      });
      const callback = options.callbackUrl ? new URL(navigationUrl(options.callbackUrl)) : undefined;
      await context.route("**/*", async route => {
        try {
          const target = new URL(navigationUrl(route.request().url()));
          if (callback && target.origin === callback.origin && target.pathname === callback.pathname) {
            await route.fulfill({ status: 200, contentType: "text/html", body: "<!doctype html><title>Payment callback</title>" });
          } else {
            await route.continue();
          }
        } catch {
          await route.abort();
        }
      });
      // Hide the `navigator.webdriver` flag so pages with casual bot checks
      // (e.g. Cashfree's UPI simulator) don't refuse to enable their own
      // submit buttons when driven by Playwright.
      await context.addInitScript(() => {
        Object.defineProperty(navigator, "webdriver", { get: () => undefined });
      });
      const configuredPages = new WeakMap<Page, Promise<void>>();
      const configureCallback = (targetPage: Page): Promise<void> => {
        const existing = configuredPages.get(targetPage);
        if (existing) return existing;
        const setup = (async () => {
          if (!callback) return;
          const cdp = await context.newCDPSession(targetPage);
          // Fetch interception sees redirect hops that Playwright route.continue()
          // may not revisit. Callback URLs are completed without network access.
          cdp.on("Fetch.requestPaused", event => {
            void (async () => {
              const target = new URL(event.request.url);
              if (target.origin === callback.origin && target.pathname === callback.pathname) {
                await cdp.send("Fetch.fulfillRequest", {
                  requestId: event.requestId,
                  responseCode: 200,
                  responseHeaders: [{ name: "Content-Type", value: "text/html" }],
                  body: Buffer.from("<!doctype html><title>Payment callback</title>").toString("base64")
                });
              } else {
                await cdp.send("Fetch.continueRequest", { requestId: event.requestId });
              }
            })().catch(() => cdp.send("Fetch.failRequest", { requestId: event.requestId, errorReason: "Aborted" }).catch(() => undefined));
          });
          await cdp.send("Fetch.enable", { patterns: [{ urlPattern: `${callback.origin}/*`, requestStage: "Request" }] });
        })();
        configuredPages.set(targetPage, setup);
        return setup;
      };
      context.on("page", targetPage => { void configureCallback(targetPage).catch(() => targetPage.close()); });
      const page = await context.newPage();
      await configureCallback(page);

      page.setDefaultTimeout(options.defaultTimeoutMs);
      page.setDefaultNavigationTimeout(options.navigationTimeoutMs);

      const session = new PlaywrightSession(browser, context, page, async () => {
        this.sessions.delete(session);
        await guard.close();
      }, guard.blocked);
      this.sessions.add(session);
      return session;
    } catch (error) {
      await browser?.close().catch(() => undefined);
      await guard.close();
      throw error;
    }
  }
}
