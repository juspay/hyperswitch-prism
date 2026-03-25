/**
 * Apple Pay Token Generator — Real Safari / SafariDriver edition
 *
 * Architecture:
 *   1. Starts a local merchant-validation server (Node.js HTTP) that proxies Apple's
 *      merchant validation endpoint using your Apple merchant cert + key (mTLS).
 *   2. Drives real Safari via SafariDriver (W3C WebDriver) — the only browser that
 *      exposes window.ApplePaySession for web-based Apple Pay automation.
 *   3. Navigates to the hosted HTTPS Apple Pay page and clicks the button (automated).
 *   4. Handles merchant validation automatically via our local server.
 *   5. PAUSES for user to approve the payment with Touch ID / Face ID / passcode.
 *   6. Captures the PKPaymentToken and prints/saves it in connector-service format.
 *
 * One-time Safari setup (on each Mac):
 *   1. Open Safari → Preferences → Advanced → check "Show Develop menu in menu bar"
 *   2. Safari menu bar → Develop → Allow Remote Automation
 *   3. Run once: sudo safaridriver --enable
 *
 * Usage:
 *   npm run apay -- \
 *     --cert merchant.pem --key merchant.key \
 *     --merchant-id merchant.com.example \
 *     --url https://shimmering-pegasus-24c886.netlify.app/applepay/apay-token-gen.html \
 *     [--creds ~/Downloads/creds.json --connector stripe] \
 *     [--output token.json] [--pretty]
 */

import fs from "fs/promises";
import path from "path";
import http from "http";
import https from "https";
import { Builder, By, until, WebDriver } from "selenium-webdriver";
import safari from "selenium-webdriver/safari.js";

// ── Types ─────────────────────────────────────────────────────────────────────

interface ApayConfig {
  merchantId: string;
  merchantName: string;
  amount: string;
  currency: string;
  countryCode: string;
  supportedNetworks: string[];
  merchantCapabilities: string[];
  initiativeContext: string;
  /** Path to Apple merchant certificate PEM file */
  certPath: string;
  /** Path to Apple merchant private key PEM file */
  keyPath: string;
}

interface CliOptions {
  configPath?: string;
  credsPath?: string;
  connector?: string;
  certPath: string;
  keyPath: string;
  hostedUrl?: string;
  outputPath?: string;
  screenshotDir?: string;
  timeout: number;
  pretty: boolean;
  validationPort: number;
  merchantIdOverride?: string;
}

// ── CLI parsing ───────────────────────────────────────────────────────────────

function printUsage(): void {
  const lines = [
    "Apple Pay Token Generator (semi-automated — real Safari via SafariDriver)",
    "",
    "One-time Safari setup (on each Mac):",
    "  1. Safari → Preferences → Advanced → check 'Show Develop menu in menu bar'",
    "  2. Safari menu bar → Develop → Allow Remote Automation",
    "  3. Run once: sudo safaridriver --enable",
    "",
    "Usage (standalone config):",
    "  npm run apay -- --config <path> --cert <pem> --key <key> --url <url>",
    "",
    "Usage (creds.json):",
    "  npm run apay -- --creds <path> --connector <name> --cert <pem> --key <key>",
    "                  --merchant-id <id> --url <url>",
    "",
    "Required:",
    "  --cert <path>           Path to Apple merchant certificate (.pem)",
    "  --key  <path>           Path to Apple merchant private key (.key/.pem)",
    "  --url  <url>            Hosted Apple Pay page URL (HTTPS, registered Apple Pay domain)",
    "",
    "Config options (use one of --config or --creds+--connector):",
    "  --config <path>         Path to a standalone apay config JSON",
    "  --creds  <path>         Path to a creds.json connector credentials file",
    "  --connector <name>      Connector name to look up in creds.json (e.g. stripe)",
    "",
    "Optional:",
    "  --merchant-id <id>      Apple merchant identifier (e.g. merchant.com.yourcompany.test)",
    "                          Required when using --creds (not stored in creds.json)",
    "  --amount <n>            Payment amount (default: 1.00)",
    "  --currency <code>       Currency code (default: USD)",
    "  --output <path>         Write full token JSON to this file",
    "  --screenshots <dir>     Directory for debug screenshots",
    "  --validation-port <n>   Port for local merchant validation server (default: 7777)",
    "  --timeout <ms>          Total timeout in ms (default: 300000 — 5 min for user auth)",
    "  --pretty                Pretty-print JSON output",
    "  --help                  Print this message",
    "",
    "Semi-automation note:",
    "  Everything is automated EXCEPT the final device authentication step.",
    "  After the Apple Pay sheet appears, approve the payment with:",
    "    - Touch ID / Face ID on your Mac",
    "    - Or your device passcode as fallback",
    "  The script will wait up to --timeout ms for your approval.",
    "",
    "Cert setup (one-time, from Apple Developer portal):",
    "  1. Create a Merchant ID at developer.apple.com",
    "  2. openssl genrsa -out merchant.key 2048",
    "     openssl req -new -key merchant.key -out merchant.csr",
    "  3. Upload CSR to Apple, download merchant.cer",
    "  4. openssl x509 -inform der -in merchant.cer -out merchant.pem",
    "  5. Pass: --cert merchant.pem --key merchant.key",
    "",
    "Example configs are in applepay/configs/",
  ];
  console.log(lines.join("\n"));
}

function parseArgs(argv: string[]): CliOptions {
  let configPath: string | undefined;
  let credsPath: string | undefined;
  let connector: string | undefined;
  let certPath = "";
  let keyPath = "";
  let hostedUrl: string | undefined;
  let outputPath: string | undefined;
  let screenshotDir: string | undefined;
  let timeout = 300_000; // 5 minutes
  let pretty = false;
  let validationPort = 7777;
  let merchantIdOverride: string | undefined;

  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i];
    if (arg === "--help" || arg === "-h")     { printUsage(); process.exit(0); }
    if (arg === "--config")                   { configPath = argv[++i]; continue; }
    if (arg === "--creds")                    { credsPath = argv[++i]; continue; }
    if (arg === "--connector")                { connector = argv[++i]; continue; }
    if (arg === "--cert")                     { certPath = argv[++i]; continue; }
    if (arg === "--key")                      { keyPath = argv[++i]; continue; }
    if (arg === "--url")                      { hostedUrl = argv[++i]; continue; }
    if (arg === "--merchant-id")              { merchantIdOverride = argv[++i]; continue; }
    if (arg === "--output")                   { outputPath = argv[++i]; continue; }
    if (arg === "--screenshots")              { screenshotDir = argv[++i]; continue; }
    if (arg === "--timeout")                  { timeout = Number(argv[++i]); continue; }
    if (arg === "--validation-port")          { validationPort = Number(argv[++i]); continue; }
    if (arg === "--pretty")                   { pretty = true; continue; }
    // Ignore unknown flags gracefully
  }

  if (!certPath)  throw new Error("--cert <path> is required (Apple merchant certificate PEM)");
  if (!keyPath)   throw new Error("--key <path> is required (Apple merchant private key)");
  if (!hostedUrl) throw new Error("--url <url> is required (hosted Apple Pay page on HTTPS)");
  if (!configPath && !credsPath) {
    throw new Error("Either --config or --creds (with --connector) is required.");
  }
  if (credsPath && !connector) {
    throw new Error("--connector is required when using --creds");
  }

  return {
    configPath,
    credsPath,
    connector,
    certPath,
    keyPath,
    hostedUrl,
    outputPath,
    screenshotDir,
    timeout,
    pretty,
    validationPort,
    merchantIdOverride,
  };
}

// ── Config loading ────────────────────────────────────────────────────────────

async function loadConfig(configPath: string): Promise<ApayConfig> {
  const raw = await fs.readFile(configPath, "utf8");
  const parsed = JSON.parse(raw) as Partial<ApayConfig>;
  if (!parsed.merchantId) throw new Error("Config must include 'merchantId'");
  return {
    merchantId:           parsed.merchantId,
    merchantName:         parsed.merchantName         ?? "Test Merchant",
    amount:               parsed.amount               ?? "1.00",
    currency:             parsed.currency             ?? "USD",
    countryCode:          parsed.countryCode          ?? "US",
    supportedNetworks:    parsed.supportedNetworks    ?? ["visa", "masterCard", "amex", "discover"],
    merchantCapabilities: parsed.merchantCapabilities ?? ["supports3DS"],
    initiativeContext:    parsed.initiativeContext    ?? "",
    certPath:             parsed.certPath             ?? "",
    keyPath:              parsed.keyPath              ?? "",
  };
}

async function loadConfigFromCreds(credsPath: string, connector: string): Promise<ApayConfig> {
  const raw = await fs.readFile(credsPath, "utf8");
  const creds = JSON.parse(raw);

  const rawEntry = creds[connector];
  if (!rawEntry) throw new Error(`No entries found for connector "${connector}" in ${credsPath}`);

  const entry: Record<string, unknown> = Array.isArray(rawEntry) ? rawEntry[0] : rawEntry;
  const meta = (entry as any)?.metadata;

  const apay = meta?.apple_pay_combined?.simplified
    ?? meta?.apple_pay?.simplified
    ?? meta?.apple_pay_combined
    ?? meta?.apple_pay;
  if (!apay) throw new Error(`No metadata.apple_pay or metadata.apple_pay_combined found for "${connector}"`);

  const sessionData = apay.session_token_data  ?? {};
  const requestData = apay.payment_request_data ?? {};

  return {
    merchantId:           "", // must be supplied via --merchant-id
    merchantName:         requestData.label ?? "Test Merchant",
    amount:               "1.00",
    currency:             "USD",
    countryCode:          sessionData.merchant_business_country ?? "US",
    supportedNetworks:    requestData.supported_networks    ?? ["visa", "masterCard", "amex", "discover"],
    merchantCapabilities: requestData.merchant_capabilities ?? ["supports3DS"],
    initiativeContext:    sessionData.initiative_context    ?? "",
    certPath:             "",
    keyPath:              "",
  };
}

function buildQueryParams(config: ApayConfig, validationPort: number): string {
  const params = new URLSearchParams();
  params.set("merchantId",           config.merchantId);
  params.set("merchantName",         config.merchantName);
  params.set("amount",               config.amount);
  params.set("currency",             config.currency);
  params.set("countryCode",          config.countryCode);
  params.set("supportedNetworks",    config.supportedNetworks.join(","));
  params.set("merchantCapabilities", config.merchantCapabilities.join(","));
  params.set("initiativeContext",    config.initiativeContext);
  params.set("validationUrl",        `http://localhost:${validationPort}/applepay/validate`);
  return params.toString();
}

// ── Merchant validation server ────────────────────────────────────────────────

function startValidationServer(
  certPem: string,
  keyPem: string,
  port: number,
): Promise<http.Server> {
  const server = http.createServer(async (req, res) => {
    res.setHeader("Access-Control-Allow-Origin", "*");
    res.setHeader("Access-Control-Allow-Headers", "Content-Type");

    if (req.method === "OPTIONS") {
      res.writeHead(204); res.end(); return;
    }

    if (req.method === "POST" && req.url === "/applepay/validate") {
      let body = "";
      req.on("data", (chunk) => { body += chunk; });
      req.on("end", async () => {
        try {
          const { validationURL, merchantId, displayName, initiativeContext } = JSON.parse(body);
          console.log(`[apay] Merchant validation → ${validationURL}`);

          const payload = JSON.stringify({
            merchantIdentifier: merchantId,
            displayName:        displayName,
            initiative:         "web",
            initiativeContext:  initiativeContext,
          });

          const appleUrl = new URL(validationURL);
          const options: https.RequestOptions = {
            hostname: appleUrl.hostname,
            port:     443,
            path:     appleUrl.pathname + appleUrl.search,
            method:   "POST",
            cert:     certPem,
            key:      keyPem,
            headers: {
              "Content-Type":   "application/json",
              "Content-Length": Buffer.byteLength(payload),
            },
          };

          const appleRes = await new Promise<{ status: number; body: string }>((resolve, reject) => {
            const appleReq = https.request(options, (appleResp) => {
              let data = "";
              appleResp.on("data", (c) => { data += c; });
              appleResp.on("end", () => resolve({ status: appleResp.statusCode ?? 0, body: data }));
            });
            appleReq.on("error", reject);
            appleReq.write(payload);
            appleReq.end();
          });

          if (appleRes.status !== 200) {
            console.error(`[apay] Apple validation HTTP ${appleRes.status}: ${appleRes.body}`);
            res.writeHead(502, { "Content-Type": "application/json" });
            res.end(JSON.stringify({ error: `Apple returned ${appleRes.status}`, detail: appleRes.body }));
            return;
          }

          console.log("[apay] Merchant validation successful");
          res.writeHead(200, { "Content-Type": "application/json" });
          res.end(appleRes.body);
        } catch (err) {
          console.error("[apay] Validation server error:", err);
          res.writeHead(500, { "Content-Type": "application/json" });
          res.end(JSON.stringify({ error: "Internal server error" }));
        }
      });
      return;
    }

    res.writeHead(404); res.end("Not found");
  });

  return new Promise((resolve, reject) => {
    server.listen(port, "127.0.0.1", () => {
      console.log(`[apay] Merchant validation server listening on http://localhost:${port}`);
      resolve(server);
    });
    server.on("error", reject);
  });
}

// ── Screenshot helper ─────────────────────────────────────────────────────────

async function screenshot(driver: WebDriver, dir: string, name: string): Promise<void> {
  try {
    await fs.mkdir(dir, { recursive: true });
    const data = await driver.takeScreenshot();
    const p = path.join(dir, `${name}-${Date.now()}.png`);
    await fs.writeFile(p, data, "base64");
    console.log(`[apay] Screenshot: ${p}`);
  } catch {
    // Non-fatal
  }
}

// ── Poll helper ───────────────────────────────────────────────────────────────

async function waitForCondition(
  driver: WebDriver,
  script: string,
  timeoutMs: number,
  pollMs = 500,
): Promise<unknown> {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const val = await driver.executeScript(script);
    if (val) return val;
    await new Promise((r) => setTimeout(r, pollMs));
  }
  throw new Error(`Timed out after ${timeoutMs}ms waiting for condition`);
}

// ── Main Apple Pay automation ─────────────────────────────────────────────────

async function run(): Promise<void> {
  const argv = process.argv.slice(2);
  const options = parseArgs(argv);

  const apayDir = path.resolve(__dirname, "..", "applepay");
  const defaultScreenshotDir = path.join(apayDir, "screenshots");

  // Load config
  let config: ApayConfig;
  if (options.configPath) {
    config = await loadConfig(path.resolve(options.configPath));
  } else {
    config = await loadConfigFromCreds(path.resolve(options.credsPath!), options.connector!);
  }

  // CLI cert/key always wins
  config.certPath = path.resolve(options.certPath);
  config.keyPath  = path.resolve(options.keyPath);

  // CLI merchant-id override
  if (options.merchantIdOverride) {
    config.merchantId = options.merchantIdOverride;
  }
  if (!config.merchantId) {
    throw new Error(
      "merchantId is required. Pass --merchant-id <your-apple-merchant-id> " +
      "(e.g. merchant.com.yourcompany.test) or add it to your config file."
    );
  }

  const screenshotDir = options.screenshotDir
    ? path.resolve(options.screenshotDir)
    : defaultScreenshotDir;

  console.log(`[apay] Merchant ID:        ${config.merchantId}`);
  console.log(`[apay] Merchant Name:      ${config.merchantName}`);
  console.log(`[apay] Amount:             ${config.amount} ${config.currency}`);
  console.log(`[apay] Networks:           ${config.supportedNetworks.join(", ")}`);
  console.log(`[apay] Initiative context: ${config.initiativeContext}`);
  console.log(`[apay] Cert:               ${config.certPath}`);
  console.log(`[apay] Key:                ${config.keyPath}`);
  console.log(`[apay] Browser:            Real Safari (SafariDriver)`);
  console.log(`[apay] Flow timeout:       ${options.timeout}ms`);

  // Read cert + key
  const certPem = await fs.readFile(config.certPath, "utf8");
  const keyPem  = await fs.readFile(config.keyPath,  "utf8");

  // Start merchant validation server
  const server = await startValidationServer(certPem, keyPem, options.validationPort);

  // Build full URL
  const queryString = buildQueryParams(config, options.validationPort);
  const fullUrl = `${options.hostedUrl}?${queryString}`;
  console.log(`\n[apay] Navigating to: ${fullUrl}\n`);

  // Launch real Safari via SafariDriver
  // SafariDriver must be enabled first: sudo safaridriver --enable
  // And in Safari: Develop → Allow Remote Automation
  const safariOptions = new safari.Options();
  // SafariDriver does not support headless mode — Safari always shows a window
  const driver: WebDriver = await new Builder()
    .forBrowser("safari")
    .setSafariOptions(safariOptions)
    .build();

  try {
    // Navigate to hosted page
    await driver.get(fullUrl);
    await screenshot(driver, screenshotDir, "apay-page-loaded");

    // Check if ApplePaySession is available
    const apayAvailable = await driver.executeScript(
      "return typeof window.ApplePaySession !== 'undefined';"
    ) as boolean;

    if (!apayAvailable) {
      throw new Error(
        "ApplePaySession is not available in this Safari instance.\n" +
        "Make sure you are running on macOS with Apple Pay configured in Wallet.\n" +
        "Also ensure Safari → Develop → Allow Remote Automation is enabled."
      );
    }
    console.log("[apay] ApplePaySession is available in Safari.");

    // Wait for the Apple Pay button to be ready
    console.log("[apay] Waiting for Apple Pay button to be ready...");
    await waitForCondition(
      driver,
      "return window.__apayReady === true || window.__apayDone === true;",
      30_000,
    );

    const pageStatus = await driver.executeScript(
      "var el = document.getElementById('status'); return el ? el.textContent : '';"
    ) as string;
    console.log(`[apay] Page status: ${pageStatus}`);
    await screenshot(driver, screenshotDir, "apay-button-ready");

    // Check for early error (ApplePaySession not available etc.)
    const earlyError = await driver.executeScript("return window.__apayError;") as string | null;
    if (earlyError) throw new Error(`Apple Pay page error: ${earlyError}`);

    // Click the Apple Pay button — automated
    console.log("[apay] Clicking Apple Pay button...");
    const btnClicked = await driver.executeScript(`
      var apBtn = document.querySelector("apple-pay-button");
      if (apBtn) { apBtn.click(); return "apple-pay-button"; }
      var fallback = document.getElementById("apay-btn-fallback");
      if (fallback && !fallback.disabled) { fallback.click(); return "fallback-btn"; }
      return null;
    `) as string | null;

    if (!btnClicked) throw new Error("Apple Pay button not found or disabled on page");
    console.log(`[apay] Clicked: ${btnClicked}`);
    await screenshot(driver, screenshotDir, "apay-sheet-triggered");

    // ── SEMI-AUTOMATION PAUSE ─────────────────────────────────────────────────
    console.log("\n" + "=".repeat(60));
    console.log("[apay] Apple Pay sheet is open.");
    console.log("[apay] ACTION REQUIRED: Approve the payment on your device.");
    console.log("[apay]   - Touch ID / Face ID on your Mac");
    console.log("[apay]   - Or confirm on your iPhone (Continuity)");
    console.log("[apay]   - Or enter your device passcode as fallback");
    console.log(`[apay] Waiting up to ${options.timeout / 1000}s for approval...`);
    console.log("=".repeat(60) + "\n");

    // Poll until done
    await waitForCondition(
      driver,
      "return window.__apayDone === true;",
      options.timeout,
      1000,
    );

    await screenshot(driver, screenshotDir, "apay-after-auth");

    const apayResult  = await driver.executeScript("return window.__apayResult;") as object | null;
    const apayError   = await driver.executeScript("return window.__apayError;")  as string | null;

    if (apayError) throw new Error(`Apple Pay failed: ${apayError}`);
    if (!apayResult) throw new Error("Apple Pay was cancelled or returned no token");

    // Build output
    const output = {
      connector:  options.connector ?? "unknown",
      merchantId: config.merchantId,
      ...(apayResult as object),
    };

    const json = options.pretty
      ? JSON.stringify(output, null, 2)
      : JSON.stringify(output);

    console.log("\n[apay] Token generated successfully!\n");
    console.log(json);

    if (options.outputPath) {
      await fs.writeFile(path.resolve(options.outputPath), json, "utf8");
      console.log(`\n[apay] Token saved to: ${options.outputPath}`);
    }

  } finally {
    await driver.quit().catch(() => undefined);
    server.close();
    console.log("[apay] Done.");
  }
}

run().catch((err) => {
  console.error(`\n[apay] FATAL: ${err.message ?? err}`);
  process.exit(1);
});
