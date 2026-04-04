/**
 * Google Pay Token Generator — WebKit edition
 *
 * Uses WebKit (Safari engine) where Google Pay opens as a real catchable popup
 * window instead of Chrome's browser-native overlay (which is inaccessible to
 * Playwright's CDP instrumentation).
 *
 * Flow:
 *   1. Serve gpay-token-gen.html locally via a tiny HTTP server
 *   2. Open a WebKit browser with a persistent profile (preserves Google login)
 *   3. Navigate to the GPay page with gateway config as URL params
 *   4. Set up context.waitForEvent('page') listener BEFORE clicking the button
 *   5. Click the Google Pay button on the main page
 *   6. The GPay popup opens at pay.google.com — Playwright catches it
 *   7. Attempt automated card selection + Pay/Continue button click inside popup
 *      (with screenshot-based discovery fallback if selectors miss)
 *   8. Poll the main page for window.__gpayDone / window.__gpayResult
 *   9. Extract and output the PaymentData / token
 *
 * Usage:
 *   npm run gpay -- --config gpay/configs/cybersource.json [--headless] [--output token.json]
 */

import fs from "node:fs/promises";
import path from "node:path";
import { webkit, type BrowserContext, type Page } from "playwright";

// ── Types ─────────────────────────────────────────────────────────────────────

interface GPayConfig {
  gateway: string;
  gatewayMerchantId: string;
  merchantName?: string;
  merchantId?: string;
  amount?: string;
  currency?: string;
  countryCode?: string;
  cardNetworks?: string[];
  authMethods?: string[];
  environment?: string;
  /** Extra key/value pairs to include verbatim in tokenizationSpecification.parameters
   *  (e.g. "stripe:publishableKey", "stripe:version" for the Stripe gateway) */
  extraTokenizationParams?: Record<string, string>;
}

interface CliOptions {
  /** Connector name to look up inside creds.json — required, passed as --connector */
  connector: string;
  /** Index of the credentials entry to use when creds.json has multiple entries (default: 0) */
  credsIndex: number;
  outputPath?: string;
  screenshotDir?: string;
  headed: boolean;
  profileDir?: string;
  /** Total timeout (ms) to wait for the full GPay flow to complete */
  timeout: number;
  /** Timeout (ms) to wait for the popup to open after clicking the button */
  popupTimeout: number;
  pretty: boolean;
}

// ── CLI parsing ───────────────────────────────────────────────────────────────

function printUsage(): void {
  const lines = [
    "Google Pay Token Generator",
    "",
    "Usage:",
    "  npm run gpay -- --connector <name> [options]",
    "",
    "Required parameter:",
    "  --connector <name>      Connector name to look up in creds.json (e.g. authorizedotnet)",
    "",
    "Required environment variables:",
    "  CONNECTOR_AUTH_FILE_PATH   Path to creds.json (fallback: UCS_CREDS_PATH)",
    "  GPAY_HOSTED_URL            Hosted Google Pay page URL",
    "                             Example: https://shimmering-pegasus-24c886.netlify.app/gpay/gpay-token-gen.html",
    "",
    "Optional parameters:",
    "  --creds-index <n>       Which creds entry to use when multiple exist (default: 0)",
    "  --headed                Run browser with visible UI (default)",
    "  --headless              Run browser without UI",
    "  --output <path>         Write full PaymentData JSON to file",
    "  --screenshots <dir>     Directory to save debug screenshots (default: gpay/screenshots)",
    "  --profile <path>        Browser profile dir for persistent Google login",
    "                          (default: gpay/.webkit-profile)",
    "  --timeout <ms>          Total flow timeout in ms (default: 120000)",
    "  --popup-timeout <ms>    Timeout to wait for GPay popup to open (default: 20000)",
    "  --pretty                Pretty-print JSON output",
    "  --help                  Show this help",
    "",
    "Examples:",
    "  export CONNECTOR_AUTH_FILE_PATH=~/Downloads/creds.json",
    "  export GPAY_HOSTED_URL=https://shimmering-pegasus-24c886.netlify.app/gpay/gpay-token-gen.html",
    "  npm run gpay -- --connector authorizedotnet --pretty",
    "  npm run gpay -- --connector cybersource --output token.json",
    "  npm run gpay -- --connector stripe --creds-index 1 --pretty",
  ];
  console.log(lines.join("\n"));
}

function parseCliOptions(argv: string[]): CliOptions {
  let connector: string | undefined;
  let credsIndex = 0;
  let outputPath: string | undefined;
  let screenshotDir: string | undefined;
  let headed = true;
  let profileDir: string | undefined;
  let timeout = 120_000;
  let popupTimeout = 20_000;
  let pretty = false;

  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];

    if (arg === "--help") {
      printUsage();
      process.exit(0);
    }
    if (arg === "--connector") {
      connector = argv[++i];
      if (!connector || connector.startsWith("--")) throw new Error("--connector requires a connector name");
      continue;
    }
    if (arg === "--creds-index") {
      credsIndex = Number(argv[++i]);
      if (!Number.isFinite(credsIndex) || credsIndex < 0) throw new Error("--creds-index must be a non-negative integer");
      continue;
    }
    if (arg === "--output") {
      outputPath = argv[++i];
      if (!outputPath || outputPath.startsWith("--")) throw new Error("--output requires a file path");
      continue;
    }
    if (arg === "--screenshots") {
      screenshotDir = argv[++i];
      if (!screenshotDir || screenshotDir.startsWith("--")) throw new Error("--screenshots requires a directory path");
      continue;
    }
    if (arg === "--headed") { headed = true; continue; }
    if (arg === "--headless") { headed = false; continue; }
    if (arg === "--profile") {
      profileDir = argv[++i];
      if (!profileDir || profileDir.startsWith("--")) throw new Error("--profile requires a directory path");
      continue;
    }
    if (arg === "--timeout") {
      timeout = Number(argv[++i]);
      if (!Number.isFinite(timeout) || timeout <= 0) throw new Error("--timeout must be a positive number");
      continue;
    }
    if (arg === "--popup-timeout") {
      popupTimeout = Number(argv[++i]);
      if (!Number.isFinite(popupTimeout) || popupTimeout <= 0) throw new Error("--popup-timeout must be a positive number");
      continue;
    }
    if (arg === "--pretty") { pretty = true; continue; }

    throw new Error(`Unknown argument: ${arg}`);
  }

  if (!connector) {
    throw new Error("--connector is required. Usage: npm run gpay -- --connector <name>\nRun --help for full usage.");
  }

  return { connector, credsIndex, outputPath, screenshotDir, headed, profileDir, timeout, popupTimeout, pretty };
}

/** Reads required env vars, throws a descriptive error if any are missing. */
function readEnvConfig(): { credsPath: string; hostedUrl: string } {
  const credsPath =
    process.env["CONNECTOR_AUTH_FILE_PATH"] ??
    process.env["UCS_CREDS_PATH"];

  const hostedUrl = process.env["GPAY_HOSTED_URL"];

  const missing: string[] = [];
  if (!credsPath) missing.push("  CONNECTOR_AUTH_FILE_PATH  (or fallback: UCS_CREDS_PATH)  — path to creds.json");
  if (!hostedUrl) missing.push("  GPAY_HOSTED_URL  — hosted Google Pay page URL (e.g. https://your-site.netlify.app/gpay/gpay-token-gen.html)");

  if (missing.length > 0) {
    throw new Error(
      `Missing required environment variable(s):\n${missing.join("\n")}\n\n` +
      `Set them before running:\n` +
      `  export CONNECTOR_AUTH_FILE_PATH=~/Downloads/creds.json\n` +
      `  export GPAY_HOSTED_URL=https://shimmering-pegasus-24c886.netlify.app/gpay/gpay-token-gen.html`
    );
  }

  return { credsPath: credsPath!, hostedUrl: hostedUrl! };
}

// ── Config loading ────────────────────────────────────────────────────────────

/**
 * Load GPayConfig from a connector-service creds.json file.
 *
 * Expected creds.json structure:
 * {
 *   "<connector>": [{
 *     "metadata": {
 *       "google_pay": {
 *         "merchant_info": {
 *           "merchant_id": { "value": "..." },
 *           "merchant_name": "..."
 *         },
 *         "allowed_payment_methods": [{
 *           "parameters": {
 *             "allowed_auth_methods": [...],
 *             "allowed_card_networks": [...]
 *           },
 *           "tokenization_specification": {
 *             "parameters": {
 *               "gateway": "...",
 *               "gateway_merchant_id": "..."
 *             }
 *           }
 *         }]
 *       }
 *     }
 *   }]
 * }
 */
async function loadConfigFromCreds(credsPath: string, connector: string, index: number): Promise<GPayConfig> {
  const raw = await fs.readFile(credsPath, "utf8");
  const creds = JSON.parse(raw);

  const raw_entry = creds[connector];
  if (!raw_entry) {
    throw new Error(`No entries found for connector "${connector}" in ${credsPath}`);
  }

  // creds.json can have either an array of entries or a single object
  let entry: Record<string, unknown>;
  if (Array.isArray(raw_entry)) {
    if (raw_entry.length === 0) {
      throw new Error(`No entries found for connector "${connector}" in ${credsPath}`);
    }
    if (index >= raw_entry.length) {
      throw new Error(`--creds-index ${index} is out of range — "${connector}" has ${raw_entry.length} entr${raw_entry.length === 1 ? "y" : "ies"}`);
    }
    entry = raw_entry[index] as Record<string, unknown>;
  } else {
    entry = raw_entry as Record<string, unknown>;
  }

  const gpay = (entry as any)?.metadata?.google_pay;
  if (!gpay) {
    throw new Error(`No metadata.google_pay found for "${connector}" in ${credsPath}`);
  }

  const pm = gpay.allowed_payment_methods?.[0];
  if (!pm) {
    throw new Error(`No allowed_payment_methods found for "${connector}"`);
  }

  const tokenSpec = pm.tokenization_specification?.parameters;
  if (!tokenSpec?.gateway) {
    throw new Error(`tokenization_specification.parameters.gateway missing for "${connector}"`);
  }

  // gatewayMerchantId: prefer gateway_merchant_id.
  // Stripe does NOT use gatewayMerchantId — it uses stripe:publishableKey + stripe:version
  // as extra tokenization parameters instead.
  const gatewayMerchantId: string = tokenSpec.gateway_merchant_id ?? "";

  // Collect any extra tokenization params (all keys except "gateway" and "gateway_merchant_id").
  // This handles Stripe's "stripe:publishableKey" / "stripe:version" etc.
  const STANDARD_TOKEN_KEYS = new Set(["gateway", "gateway_merchant_id"]);
  const extraTokenizationParams: Record<string, string> = {};
  for (const [k, v] of Object.entries(tokenSpec)) {
    if (!STANDARD_TOKEN_KEYS.has(k) && typeof v === "string") {
      extraTokenizationParams[k] = v;
    }
  }

  const merchantId: string =
    gpay.merchant_info?.merchant_id?.value ??
    gpay.merchant_info?.merchant_id ??
    "";
  const merchantName: string = gpay.merchant_info?.merchant_name ?? "";

  const cardNetworks: string[] = (pm.parameters?.allowed_card_networks ?? [])
    .map((n: string) => n.toUpperCase());
  const authMethods: string[] = pm.parameters?.allowed_auth_methods ?? ["PAN_ONLY", "CRYPTOGRAM_3DS"];

  return {
    gateway: tokenSpec.gateway,
    gatewayMerchantId,
    merchantId: merchantId || undefined,
    merchantName: merchantName || undefined,
    cardNetworks: cardNetworks.length > 0 ? cardNetworks : undefined,
    authMethods,
    // Defaults for payment amount — can be overridden via a --config file
    amount: "1.00",
    currency: "USD",
    countryCode: "US",
    extraTokenizationParams: Object.keys(extraTokenizationParams).length > 0 ? extraTokenizationParams : undefined,
  };
}

function buildQueryParams(config: GPayConfig): string {
  const params = new URLSearchParams();
  params.set("gateway", config.gateway);
  if (config.gatewayMerchantId) params.set("gatewayMerchantId", config.gatewayMerchantId);
  if (config.merchantName) params.set("merchantName", config.merchantName);
  if (config.merchantId) params.set("merchantId", config.merchantId);
  if (config.amount) params.set("amount", config.amount);
  if (config.currency) params.set("currency", config.currency);
  if (config.countryCode) params.set("countryCode", config.countryCode);
  if (config.cardNetworks) params.set("cardNetworks", config.cardNetworks.join(","));
  if (config.authMethods) params.set("authMethods", config.authMethods.join(","));
  if (config.environment) params.set("environment", config.environment);
  if (config.extraTokenizationParams && Object.keys(config.extraTokenizationParams).length > 0) {
    params.set("tokenParams", JSON.stringify(config.extraTokenizationParams));
  }
  return params.toString();
}

// ── Screenshot helper ─────────────────────────────────────────────────────────

async function screenshot(page: Page, dir: string, name: string): Promise<void> {
  await fs.mkdir(dir, { recursive: true });
  const p = path.join(dir, `${name}-${Date.now()}.png`);
  await page.screenshot({ path: p, fullPage: true }).catch(() => undefined);
  console.log(`[gpay] Screenshot: ${p}`);
}

// ── Google Pay popup automation ───────────────────────────────────────────────

/**
 * Selectors to try inside the Google Pay popup, in priority order.
 *
 * These are empirical — Google's popup DOM is obfuscated and not officially
 * documented. The list is ordered from most to least stable.
 *
 * Card item selectors (click one to select the test card):
 */
const CARD_ITEM_SELECTORS = [
  // Specific payment instrument rows observed in pay.google.com TEST mode
  "[data-instrument-id]",
  "[jsaction*='click'][data-item-id]",
  // Stripe test mode: card list items (div[role="button"][jsname="wQNmvb"])
  // These are the clickable card rows in the full card picker
  "div[role='button'][jsname='wQNmvb']",
  // Generic list items that look like card rows
  "li[role='option']",
  "li[role='radio']",
  "div[role='option']",
  "div[role='radio']",
  // Observed obfuscated class patterns (may change)
  ".WpHeLc",
  ".JMQmEb",
  // Fallback: any clickable list item with card-like text
  "li",
];

/**
 * Selectors for the "Continue" / "Pay" button in the popup.
 */
const PAY_BUTTON_SELECTORS = [
  // Aria / role based
  "button[jsname='LgbsSe']",
  "button[data-primary-action]",
  "button[aria-label*='Pay']",
  "button[aria-label*='Continue']",
  "button[aria-label*='pay']",
  "button[aria-label*='continue']",
  // Text based (most resilient — locator.getByText)
  // handled separately below
  // Generic primary-looking button
  "button[type='submit']",
];

/**
 * Attempt to automate the Google Pay popup:
 * 1. Wait for popup to fully load
 * 2. Take a discovery screenshot
 * 3. Try to select the first test card
 * 4. Try to click the Pay/Continue button
 */
async function automateGPayPopup(
  popup: Page,
  screenshotDir: string,
  timeoutMs: number
): Promise<void> {
  console.log(`[gpay] Popup URL: ${popup.url()}`);

  try {
    await popup.waitForLoadState("domcontentloaded", { timeout: timeoutMs });
  } catch { /* continue */ }

  // The payment sheet renders inside an iframe — wait for it to appear
  console.log("[gpay] Waiting for payment sheet iframe...");
  try {
    await popup.waitForSelector("iframe", { timeout: 30_000 });
  } catch { /* no iframe — content may be in main frame */ }

  // Wait for the buyflow2 iframe content to fully load (not just the <iframe> tag).
  // Poll until the frame shows real content (a Pay button with non-zero rect, or
  // non-empty non-"loading" body text). Timeout after 30s.
  console.log("[gpay] Waiting for buyflow2 iframe content to load...");
  const buyflow2Ready = await (async () => {
    const deadline = Date.now() + 30_000;
    while (Date.now() < deadline) {
      const buyflow2Frame = popup.frames().find(f => f.url().includes("buyflow2"));
      if (buyflow2Frame) {
        // Check for a Pay button with non-zero rect (sheet fully rendered)
        const btnVisible = await buyflow2Frame.evaluate(() => {
          const btns = Array.from(document.querySelectorAll("button[jsname='LgbsSe'], button"))
            .filter(el => {
              const txt = ((el as HTMLElement).innerText ?? (el as HTMLElement).textContent ?? "").trim();
              return txt === "Pay" || txt.startsWith("Pay ");
            }) as HTMLElement[];
          return btns.some(btn => {
            const r = btn.getBoundingClientRect();
            return r.width > 0 && r.height > 0;
          });
        }).catch(() => false);
        if (btnVisible) {
          console.log("[gpay] buyflow2 Pay button with non-zero rect detected — sheet ready");
          return true;
        }
        // Also check if there's meaningful body text (not just "loading")
        const bodyText = await buyflow2Frame.evaluate(() => document.body?.innerText?.trim() ?? "").catch(() => "");
        if (bodyText && bodyText !== "loading" && bodyText.length > 20) {
          console.log(`[gpay] buyflow2 body text detected (${bodyText.length} chars) — sheet loaded`);
          return true;
        }
      }
      await popup.waitForTimeout(500);
    }
    return false;
  })();
  if (!buyflow2Ready) {
    console.log("[gpay] buyflow2 iframe did not become ready within 30s — proceeding anyway");
  }

  await screenshot(popup, screenshotDir, "gpay-popup-initial");

  // ── Dump text from ALL frames (main + iframes) for debugging ─────────────
  const allText = await (async () => {
    const parts: string[] = [];
    // Main frame
    const main = await popup.evaluate(() => document.body?.innerText ?? "").catch(() => "");
    if (main.trim()) parts.push(`[main] ${main.trim()}`);
    // All frames
    for (const frame of popup.frames()) {
      if (frame === popup.mainFrame()) continue;
      const ft = await frame.evaluate(() => document.body?.innerText ?? "").catch(() => "");
      if (ft.trim()) parts.push(`[frame:${frame.url().slice(0, 80)}] ${ft.trim()}`);
    }
    return parts.join("\n");
  })();
  console.log(`[gpay] All frames text (first 600 chars):\n${allText.slice(0, 600)}`);

  // ── Detect sign-in screen (may be in main frame or iframe) ───────────────
  const isSignInPage = allText.includes("Sign in") && allText.includes("Google Account");
  if (isSignInPage) {
    console.log("[gpay] Google sign-in screen detected. Please sign in in the popup window.");
    console.log("[gpay] Waiting up to 5 minutes for sign-in to complete...");
    try {
      await popup.waitForFunction(
        () => {
          const t = document.body?.innerText ?? "";
          return !t.includes("Email or phone") && !t.includes("Use your Google Account");
        },
        { timeout: 300_000 }
      );
      console.log("[gpay] Sign-in complete. Waiting for payment sheet...");
      await popup.waitForTimeout(4000);
      await screenshot(popup, screenshotDir, "gpay-popup-after-signin");
    } catch {
      throw new Error("Timed out waiting for Google sign-in (5 min). Run with --headed and sign in.");
    }
  }

  // ── Helper: try a click in main frame then each iframe ──────────────────
  // Tries Playwright pointer-event click first, then falls back to JS click.
  // For card-row selectors (div[role='button'][jsname='wQNmvb']), the first
  // match is always the account/header button — skip it and click nth(1).
  async function tryClickInAllFrames(
    cssSelector: string,
    description: string,
  ): Promise<boolean> {
    const allF = [popup.mainFrame(), ...popup.frames().filter(f => f !== popup.mainFrame())];
    const isCardRowSelector = cssSelector === "div[role='button'][jsname='wQNmvb']";
    for (const frame of allF) {
      // Strategy A: Playwright real click (full pointer event chain)
      try {
        const loc = isCardRowSelector
          ? frame.locator(cssSelector).nth(1)   // nth(0) = account button, nth(1) = first card
          : frame.locator(cssSelector).first();
        const exists = await loc.count().catch(() => 0);
        if (exists > 0) {
          await loc.click({ timeout: 5000, force: true });
          console.log(`[gpay] Playwright click: ${description} in frame ${frame.url().slice(0, 80)}`);
          return true;
        }
      } catch { /* try JS fallback */ }
      // Strategy B: JS evaluate click (skips account button for card-row selectors)
      try {
        const clicked = await frame.evaluate((args) => {
          const { sel, skipFirst } = args;
          const els = Array.from(document.querySelectorAll(sel)) as HTMLElement[];
          const el = skipFirst ? els[1] : els[0];
          if (!el) return false;
          el.dispatchEvent(new MouseEvent("click", { bubbles: true, cancelable: true, view: window }));
          return true;
        }, { sel: cssSelector, skipFirst: isCardRowSelector }).catch(() => false);
        if (clicked) {
          console.log(`[gpay] JS click: ${description} in frame ${frame.url().slice(0, 80)}`);
          return true;
        }
      } catch { /* next frame */ }
    }
    return false;
  }

  const allFrames = [popup.mainFrame(), ...popup.frames().filter(f => f !== popup.mainFrame())];

  // ── Helper: try to click the Pay button in buyflow2 frame ─────────────────
  // Returns true if the click was fired on a Pay button with non-zero size.
  // Strategy priority:
  //   1. frameLocator().click() — Playwright's canonical cross-origin iframe click
  //   2. frame.locator().click() — direct frame locator click
  //   3. Full PointerEvent sequence via frame.evaluate — synthesize trusted-ish events
  //   4. popup.mouse.click() at computed absolute coords — raw mouse fallback
  async function tryClickPayButton(): Promise<boolean> {
    const buyflow2Frame = popup.frames().find(f => f.url().includes("buyflow2"));

    // Strategy 1: Use Playwright frameLocator — the right API for cross-origin iframe clicks.
    // frameLocator() handles the cross-origin boundary correctly in WebKit.
    try {
      const paymentIframeLoc = popup.frameLocator("iframe[src*='payments.google.com']").first();
      const payBtnLoc = paymentIframeLoc
        .locator("button[jsname='LgbsSe'], button")
        .filter({ hasText: /^Pay/ })
        .first();
      const count = await payBtnLoc.count().catch(() => 0);
      if (count > 0) {
        const rect = await payBtnLoc.boundingBox().catch(() => null);
        // Also get popup viewport size to sanity-check coordinates
        const vpSize = await popup.evaluate(() => ({
          vw: window.innerWidth, vh: window.innerHeight,
          outerW: window.outerWidth, outerH: window.outerHeight,
        })).catch(() => null);
        console.log(`[gpay] frameLocator Pay button boundingBox: ${JSON.stringify(rect)}, popup viewport: ${JSON.stringify(vpSize)}`);
        if (rect && rect.width > 0 && rect.height > 0) {
          // If button bottom is outside viewport, scroll the iframe into position
          const btnBottom = rect.y + rect.height;
          if (vpSize && btnBottom > vpSize.vh) {
            console.log(`[gpay] Button bottom (${btnBottom}) > viewport height (${vpSize.vh}) — setting viewport taller`);
            await popup.setViewportSize({ width: vpSize.vw || 600, height: Math.ceil(btnBottom) + 50 });
            await popup.waitForTimeout(300);
          }
          await popup.bringToFront();

          // ── ROOT CAUSE FIX ────────────────────────────────────────────────────
          // Google's Closure jsaction framework only fires when event.target IS
          // the BUTTON element (jsname='LgbsSe'). Without this fix, Playwright's
          // hit-test lands on a child div/span, making event.target a descendant —
          // and Closure skips the handler.
          //
          // Fix: set pointer-events:none on all button descendants so the browser's
          // hit-test lands on the BUTTON itself, making event.target === button.
          if (buyflow2Frame) {
            await buyflow2Frame.evaluate(() => {
              const btn = Array.from(document.querySelectorAll("button[jsname='LgbsSe'], button"))
                .filter(b => {
                  const t = ((b as HTMLElement).innerText ?? b.textContent ?? "").trim();
                  return (t === "Pay" || t.startsWith("Pay ")) &&
                    b.getBoundingClientRect().width > 0;
                })[0] as HTMLElement | undefined;
              if (!btn) return;
              Array.from(btn.querySelectorAll("*")).forEach((child) => {
                (child as HTMLElement).style.pointerEvents = "none";
              });
            }).catch(() => undefined);
            await popup.waitForTimeout(200);
          }
          // ── END ROOT CAUSE FIX ────────────────────────────────────────────────

          // Strategy 1a: tap() — fires pointerdown+touchstart+pointerup+click
          // all with isTrusted:true on the BUTTON, triggering the jsaction handler.
          const isPopupClosedAfterTap = await (async () => {
            try {
              await payBtnLoc.tap({ timeout: 5000 });
              console.log("[gpay] Strategy 1a: tap() fired — waiting for popup to close");
              await popup.waitForTimeout(2000);
              return popup.isClosed();
            } catch (e) {
              const msg = String(e);
              if (msg.includes("closed") || msg.includes("destroyed")) return true; // popup already closed — success
              console.log(`[gpay] Strategy 1a tap failed: ${e}`);
              return false;
            }
          })();
          if (isPopupClosedAfterTap) return true;

          // Strategy 1b: standard click() fallback
          const isPopupClosedAfterClick = await (async () => {
            try {
              await payBtnLoc.click({ timeout: 5000 });
              console.log("[gpay] Strategy 1b: click() fired — waiting for popup to close");
              await popup.waitForTimeout(2000);
              return popup.isClosed();
            } catch (e) {
              const msg = String(e);
              if (msg.includes("closed") || msg.includes("destroyed")) return true;
              console.log(`[gpay] Strategy 1b click failed: ${e}`);
              return false;
            }
          })();
          if (isPopupClosedAfterClick) return true;

          console.log("[gpay] tap() and click() fired but popup still open (may be a gateway config error)");
        }
      } else {
        console.log("[gpay] frameLocator: no Pay button found in payments.google.com iframe");
      }
    } catch (e) {
      const msg = String(e);
      if (msg.includes("closed") || msg.includes("destroyed")) return true;
      console.log(`[gpay] Pay button click failed: ${e}`);
    }

    return popup.isClosed();
  }

  // ── Step 1: Try Pay immediately — card may already be pre-selected ─────────
  // In many sheets (Stripe, Cybersource) the confirmation pane is shown on open
  // with a card already selected. Clicking a card row first navigates *into* the
  // card picker detail view (wrong direction) and collapses the Pay button.
  console.log("[gpay] Step 1: Attempting Pay click before card selection (card may be pre-selected)...");
  await popup.waitForTimeout(1500); // let animations settle after iframe load
  await screenshot(popup, screenshotDir, "gpay-popup-before-pay");

  let payClicked = await tryClickPayButton();

  // ── Step 2 (fallback): Select a card, then try Pay again ─────────────────
  // Only runs if Step 1 found no visible Pay button — some sheets require explicit
  // card selection before the confirmation pane (and Pay button) appear.
  if (!payClicked) {
    console.log("[gpay] No visible Pay button found — attempting card selection as fallback...");
    let cardSelected = false;
    for (const sel of CARD_ITEM_SELECTORS) {
      if (await tryClickInAllFrames(sel, `card (${sel})`)) {
        cardSelected = true;
        console.log("[gpay] Card clicked — waiting up to 8s for Pay button to become visible...");
        const deadline = Date.now() + 8000;
        while (Date.now() < deadline) {
          for (const fr of allFrames) {
            const r = await fr.evaluate(() => {
              const btn = document.querySelector("button[jsname='LgbsSe']") as HTMLElement | null;
              if (!btn) return null;
              const rect = btn.getBoundingClientRect();
              return { w: rect.width, h: rect.height };
            }).catch(() => null);
            if (r && r.w > 0 && r.h > 0) {
              console.log(`[gpay] Pay button visible after card selection: ${JSON.stringify(r)}`);
              break;
            }
          }
          await popup.waitForTimeout(500);
        }
        await screenshot(popup, screenshotDir, "gpay-popup-after-card-select");
        break;
      }
    }
    if (!cardSelected) {
      console.log("[gpay] No card selector matched — sheet may have auto-selected a card");
    }

    await popup.waitForTimeout(1000);
    payClicked = await tryClickPayButton();
  }

  // ── Step 3 (last resort): JS synthetic click on Pay button ──────────────────
  // Real mouse.click (above) is authoritative. Only reach here if tryClickPayButton
  // found no visible Pay button at all — try JS click as a final attempt.
  if (!payClicked) {
    console.log("[gpay] WARNING: No visible Pay button found via mouse click — trying JS synthetic click...");
    for (const frame of allFrames) {
      try {
        const clicked = await frame.evaluate(() => {
          const btns = Array.from(document.querySelectorAll("button[jsname='LgbsSe'], button"))
            .filter(el => {
              const txt = ((el as HTMLElement).innerText ?? (el as HTMLElement).textContent ?? "").trim();
              return txt === "Pay" || txt.startsWith("Pay ");
            }) as HTMLElement[];
          if (btns.length === 0) return false;
          btns[0].click();
          return true;
        }).catch(() => false);
        if (clicked) {
          console.log(`[gpay] JS synthetic click on Pay button in frame ${frame.url().slice(0, 80)}`);
          payClicked = true;
          break;
        }
      } catch { /* next frame */ }
    }
  }

  if (!payClicked) {
    console.log("[gpay] WARNING: Could not find Pay/Continue button — popup may close on its own");
  }

  // Wait for popup to close (navigation away = payment submitted) or timeout
  console.log("[gpay] Waiting for popup to close after Pay click...");
  try {
    await popup.waitForEvent("close", { timeout: 30_000 });
    console.log("[gpay] Popup closed — payment submitted");
  } catch {
    // Popup didn't close — take a screenshot to see current state
    console.log("[gpay] Popup did not close within 30s — taking screenshot");
    await screenshot(popup, screenshotDir, "gpay-popup-after-pay").catch(() => undefined);
  }
}

// ── Poll for GPay result ──────────────────────────────────────────────────────

async function waitForGPayResult(page: Page, timeoutMs: number): Promise<Record<string, unknown>> {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const done = await page.evaluate("window.__gpayDone").catch(() => false);
    if (done) {
      const error = await page.evaluate("window.__gpayError").catch(() => null);
      if (error) throw new Error(`Google Pay error: ${error}`);
      const result = await page.evaluate("window.__gpayResult").catch(() => null);
      return result as Record<string, unknown>;
    }
    await new Promise((r) => setTimeout(r, 500));
  }
  throw new Error(`Timed out after ${timeoutMs}ms waiting for Google Pay result`);
}

// ── Main ──────────────────────────────────────────────────────────────────────

async function main(): Promise<void> {
  const options = parseCliOptions(process.argv.slice(2));
  const { credsPath, hostedUrl } = readEnvConfig();

  const gpayDir = path.resolve(__dirname, "..", "gpay");
  const defaultProfileDir = path.join(gpayDir, ".webkit-profile");
  const defaultScreenshotDir = path.join(gpayDir, "screenshots");

  const credsFullPath = path.resolve(credsPath);

  const config = await loadConfigFromCreds(credsFullPath, options.connector, options.credsIndex);
  const profileDir = options.profileDir ? path.resolve(options.profileDir) : defaultProfileDir;
  const screenshotDir = options.screenshotDir ? path.resolve(options.screenshotDir) : defaultScreenshotDir;

  console.log(`[gpay] Connector:          ${options.connector}`);
  console.log(`[gpay] Creds file:         ${credsFullPath}`);
  console.log(`[gpay] Gateway:            ${config.gateway}`);
  if (config.gatewayMerchantId) {
    console.log(`[gpay] Gateway Merchant ID: ${config.gatewayMerchantId}`);
  }
  if (config.extraTokenizationParams) {
    console.log(`[gpay] Extra token params:  ${JSON.stringify(config.extraTokenizationParams)}`);
  }
  console.log(`[gpay] Amount:             ${config.amount ?? "10.00"} ${config.currency ?? "USD"}`);
  console.log(`[gpay] Browser:            WebKit (Safari engine)`);
  console.log(`[gpay] Profile dir:        ${profileDir}`);
  console.log(`[gpay] Screenshot dir:     ${screenshotDir}`);
  console.log(`[gpay] Headed:             ${options.headed}`);
  console.log(`[gpay] Flow timeout:       ${options.timeout}ms`);
  console.log(`[gpay] Hosted URL:         ${hostedUrl}`);
  console.log();

  await fs.mkdir(profileDir, { recursive: true });
  await fs.mkdir(screenshotDir, { recursive: true });

  const queryParams = buildQueryParams(config);

  // Strip any trailing query string from the hosted URL so we can append ours
  const baseUrl = hostedUrl.split("?")[0];
  const pageUrl = `${baseUrl}?${queryParams}`;
  console.log(`[gpay] Using hosted URL: ${pageUrl}`);

  // WebKit persistent context
  // Note: WebKit's persistent context API differs from Chromium — use storageState instead
  // because WebKit doesn't support launchPersistentContext as reliably cross-platform.
  // We use a storageState file to persist cookies/localStorage (Google login session).
  const storageStatePath = path.join(profileDir, "storage-state.json");
  let storageState: string | undefined;
  try {
    await fs.access(storageStatePath);
    storageState = storageStatePath;
    console.log("[gpay] Using saved browser session (storage state)");
  } catch {
    console.log("[gpay] WARNING: No saved Google session found.");
    console.log("[gpay]   GPay will require manual sign-in during the test flow.");
    console.log("[gpay]   To avoid this, run one-time setup first:");
    console.log("[gpay]");
    console.log("[gpay]     cd browser-automation-engine && npm run gpay:login");
    console.log("[gpay]");
    console.log("[gpay]   This opens a browser for you to sign in to Google once.");
    console.log("[gpay]   The session is reused for all future GPay runs.");
    console.log();
  }

  let context: BrowserContext | undefined;
  let browser: Awaited<ReturnType<typeof webkit.launch>> | undefined;

  try {
    browser = await webkit.launch({
      headless: !options.headed,
    });

    context = await browser.newContext({
      viewport: { width: 1280, height: 900 },
      ...(storageState ? { storageState } : {}),
      javaScriptEnabled: true,
      // hasTouch: true enables touch event support so locator.tap() works.
      // Touch events may route differently than mouse events through WebKit's
      // cross-process iframe boundary — worth trying for the Pay button.
      hasTouch: true,
      // Explicitly allow popups — WebKit blocks window.open() by default unless
      // triggered by a trusted user gesture. Setting this to "allow" ensures
      // pay.google.com popup is never suppressed.
      permissions: [],
      bypassCSP: false,
    });

    // Grant popup permission for the hosted origin so WebKit never blocks
    // the pay.google.com window.open() call with OR_BIBED_15.
    const origin = new URL(hostedUrl.split("?")[0]).origin;
    if (origin) {
      await context.grantPermissions([], { origin });
    }
    // might do, and ensure pay.google.com is always considered "allowed".
    // Also route all pages in the context to allow popups via page.addInitScript.
    context.on("page", (p) => {
      p.addInitScript(() => {
        // Prevent any JS-level popup blockers from firing
        const _open = window.open.bind(window);
        window.open = function(...args) { return _open(...args); };
      }).catch(() => undefined);
    });

    const page = await context.newPage();

    // Capture console messages and page errors for debugging
    page.on("console", (msg) => {
      if (msg.type() === "error" || msg.type() === "warning") {
        console.log(`[gpay] browser ${msg.type()}: ${msg.text()}`);
      }
    });
    page.on("pageerror", (err) => {
      console.log(`[gpay] page error: ${err.message}`);
    });
    page.on("requestfailed", (req) => {
      console.log(`[gpay] request failed: ${req.url()} — ${req.failure()?.errorText}`);
    });

    console.log("[gpay] Navigating to Google Pay page...");
    await page.goto(pageUrl, { waitUntil: "networkidle", timeout: 30_000 });
    await screenshot(page, screenshotDir, "main-page-loaded");

    // Wait for the Google Pay button to render (or status to show an error)
    console.log("[gpay] Waiting for Google Pay button...");

    // First wait for status to change from "loading" (indicates pay.js finished one way or another)
    try {
      await page.waitForFunction(
        () => {
          const status = document.getElementById("status");
          const text = status?.textContent ?? "";
          return !text.includes("Loading pay.js") && text.length > 0;
        },
        { timeout: 20_000 }
      );
    } catch {
      // Status may not change — continue anyway
    }

    // Check current status text
    const statusText = await page.evaluate(
      () => document.getElementById("status")?.textContent ?? "(no status)"
    );
    console.log(`[gpay] Page status: ${statusText}`);

    // Check for early error
    const earlyError = await page.evaluate("window.__gpayError").catch(() => null);
    const earlyDone = await page.evaluate("window.__gpayDone").catch(() => false);
    if (earlyDone && earlyError) {
      throw new Error(`Google Pay initialization failed: ${earlyError}`);
    }

    // Now wait for the button to appear in #gpay-container
    const gpayButtonSelector = [
      "#gpay-container button",
      "#gpay-container [role='button']",
      "#gpay-container .gpay-button-fill",
      "#gpay-container .gpay-button",
      "#gpay-container svg",           // GPay button is sometimes SVG-based
      "#gpay-container div",           // Last resort
    ].join(", ");

    try {
      await page.waitForSelector(gpayButtonSelector, { state: "visible", timeout: 15_000 });
    } catch {
      // Fallback: wait for anything in the container
      await page.waitForSelector("#gpay-container *", { state: "visible", timeout: 10_000 });
    }

    await page.waitForTimeout(1500);
    await screenshot(page, screenshotDir, "main-page-button-ready");

    // Log what's in the container for debug
    const containerHtml = await page.evaluate(
      () => document.getElementById("gpay-container")?.innerHTML ?? "(empty)"
    );
    console.log(`[gpay] #gpay-container innerHTML (first 300 chars): ${containerHtml.slice(0, 300)}`);

    // ── Set up popup listener BEFORE clicking (race condition otherwise) ──────
    console.log("[gpay] Setting up popup listener...");
    const popupPromise = context.waitForEvent("page", { timeout: options.popupTimeout });

    // ── Click the Google Pay button ───────────────────────────────────────────
    // Use page.evaluate to dispatch a native click inside the page JS context.
    // WebKit requires the window.open() call to originate from a real user-gesture
    // stack — a synthetic Playwright click dispatched from CDP is sometimes treated
    // as non-trusted, causing OR_BIBED_15. A click() call inside evaluate() is
    // considered trusted by WebKit because it runs in the page's own JS context.
    console.log("[gpay] Clicking Google Pay button (native JS click)...");
    const nativeClicked = await page.evaluate(() => {
      const selectors = [
        "#gpay-container button",
        "#gpay-container [role='button']",
        "#gpay-container .gpay-button-fill",
        "#gpay-container .gpay-button",
        "#gpay-container svg",
        "#gpay-container div",
        "#gpay-container",
      ];
      for (const sel of selectors) {
        const el = document.querySelector(sel) as HTMLElement | null;
        if (el) { el.click(); return sel; }
      }
      return null;
    });

    if (nativeClicked) {
      console.log(`[gpay] Native click on: ${nativeClicked}`);
    } else {
      // Final fallback — synthetic Playwright click
      await page.locator("#gpay-container").click({ timeout: 5000 });
      console.log("[gpay] Clicked #gpay-container (Playwright fallback)");
    }

    // ── Wait for popup ────────────────────────────────────────────────────────
    console.log("[gpay] Waiting for Google Pay popup...");
    let popup: Page;
    try {
      popup = await popupPromise;
      console.log(`[gpay] Popup opened: ${popup.url()}`);
    } catch (e) {
      // Popup may not open in TEST mode if pay.js resolves immediately without showing UI
      // (e.g. if there's no Google account signed in and pay.js short-circuits)
      // Check if we already have a result
      const done = await page.evaluate("window.__gpayDone").catch(() => false);
      if (done) {
        console.log("[gpay] No popup opened — pay.js resolved synchronously (no Google account?)");
      } else {
        throw new Error(
          `Google Pay popup did not open within ${options.popupTimeout}ms. ` +
          `This usually means: (1) you are not signed into Google in WebKit (run once with --headed ` +
          `to sign in), or (2) pay.js failed to initialize. Check screenshot: ${screenshotDir}`
        );
      }
      popup = null as unknown as Page;
    }

    // ── Automate the popup if it opened ──────────────────────────────────────
    if (popup) {
      await automateGPayPopup(popup, screenshotDir, options.timeout);
    }

    // ── Wait for the result on the main page ──────────────────────────────────
    console.log("[gpay] Waiting for payment result...");
    const paymentData = await waitForGPayResult(page, options.timeout);

    // ── Save storage state for next run ──────────────────────────────────────
    await context.storageState({ path: storageStatePath });
    console.log(`[gpay] Session saved to ${storageStatePath}`);

    // ── Extract token ─────────────────────────────────────────────────────────
    const tokenizationData = (paymentData as any)?.paymentMethodData?.tokenizationData;
    const rawToken = tokenizationData?.token;

    console.log("[gpay] Token generated successfully!");
    console.log();

    const output = {
      gateway: config.gateway,
      gatewayMerchantId: config.gatewayMerchantId,
      paymentData,
      token: rawToken ? (tryParseJson(rawToken) ?? rawToken) : null,
    };

    const jsonOutput = JSON.stringify(output, null, options.pretty ? 2 : undefined);
    console.log(jsonOutput);

    if (options.outputPath) {
      const outPath = path.resolve(options.outputPath);
      await fs.mkdir(path.dirname(outPath), { recursive: true });
      await fs.writeFile(outPath, JSON.stringify(output, null, 2));
      console.log(`\n[gpay] Written to: ${outPath}`);
    }
  } finally {
    // Close context and browser to ensure clean shutdown
    if (context) await context.close().catch(() => undefined);
    if (browser) await browser.close().catch(() => undefined);
  }
}

function tryParseJson(value: string): unknown {
  try { return JSON.parse(value); } catch { return undefined; }
}

main().catch((error) => {
  const message = error instanceof Error ? error.message : String(error);
  console.error(`[gpay] Error: ${message}`);
  process.exit(1);
});
