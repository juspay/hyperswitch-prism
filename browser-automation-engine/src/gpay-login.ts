/**
 * Google Pay — One-time Google Account Login
 *
 * Opens a WebKit browser to accounts.google.com so the user can sign in
 * interactively. Saves the session (cookies + localStorage) to
 * gpay/.webkit-profile/storage-state.json, which gpay-token-gen.ts then
 * reuses for all subsequent GPay token generation runs.
 *
 * Usage:
 *   npm run gpay:login                # headed (default)
 *   npm run gpay:login -- --profile <dir>  # custom profile directory
 *
 * This is intended to be run once during setup (make setup-connector-tests).
 * The saved session persists across runs until Google expires it (typically
 * weeks/months). Re-run this command if GPay tests start showing sign-in
 * screens again.
 */

import fs from "node:fs/promises";
import path from "node:path";
import { webkit } from "playwright";

// ── CLI parsing ───────────────────────────────────────────────────────────────

interface LoginOptions {
  profileDir: string;
  timeout: number;
}

function parseArgs(argv: string[]): LoginOptions {
  const gpayDir = path.resolve(__dirname, "..", "gpay");
  let profileDir = path.join(gpayDir, ".webkit-profile");
  let timeout = 300_000; // 5 minutes to sign in

  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === "--help") {
      console.log(
        [
          "Google Pay — One-time Google Account Login",
          "",
          "Usage: npm run gpay:login [-- options]",
          "",
          "Options:",
          "  --profile <dir>    Browser profile directory (default: gpay/.webkit-profile)",
          "  --timeout <ms>     Max time to wait for sign-in (default: 300000 = 5 min)",
          "  --help             Show this help",
        ].join("\n")
      );
      process.exit(0);
    }
    if (arg === "--profile") {
      profileDir = argv[++i];
      if (!profileDir || profileDir.startsWith("--"))
        throw new Error("--profile requires a directory path");
      profileDir = path.resolve(profileDir);
      continue;
    }
    if (arg === "--timeout") {
      timeout = Number(argv[++i]);
      if (!Number.isFinite(timeout) || timeout <= 0)
        throw new Error("--timeout must be a positive number");
      continue;
    }
    throw new Error(`Unknown argument: ${arg}`);
  }

  return { profileDir, timeout };
}

// ── Main ──────────────────────────────────────────────────────────────────────

async function main(): Promise<void> {
  const options = parseArgs(process.argv.slice(2));
  const storageStatePath = path.join(options.profileDir, "storage-state.json");

  // Check if session already exists and is likely valid
  try {
    const stat = await fs.stat(storageStatePath);
    const ageMs = Date.now() - stat.mtimeMs;
    const ageDays = Math.floor(ageMs / (1000 * 60 * 60 * 24));
    console.log(
      `[gpay:login] Existing session found (${ageDays} day${ageDays !== 1 ? "s" : ""} old): ${storageStatePath}`
    );
    console.log(
      "[gpay:login] This will open a browser so you can verify/refresh your Google sign-in."
    );
    console.log();
  } catch {
    console.log("[gpay:login] No existing session found — starting fresh.");
    console.log();
  }

  await fs.mkdir(options.profileDir, { recursive: true });

  console.log("[gpay:login] Opening WebKit browser for Google sign-in...");
  console.log("[gpay:login] Please sign in to your Google account in the browser window.");
  console.log(
    `[gpay:login] You have ${Math.round(options.timeout / 60_000)} minutes to complete sign-in.`
  );
  console.log();

  // Load existing session if available (so user can just verify)
  let storageState: string | undefined;
  try {
    await fs.access(storageStatePath);
    storageState = storageStatePath;
  } catch {
    // No existing session
  }

  const browser = await webkit.launch({ headless: false });
  const context = await browser.newContext({
    viewport: { width: 1280, height: 900 },
    ...(storageState ? { storageState } : {}),
    javaScriptEnabled: true,
  });

  const page = await context.newPage();

  try {
    // Navigate to Google account page — this will either show sign-in or
    // the account dashboard if already logged in.
    await page.goto("https://accounts.google.com", {
      waitUntil: "networkidle",
      timeout: 30_000,
    });

    // Detect current state
    const pageText = await page
      .evaluate(() => document.body?.innerText ?? "")
      .catch(() => "");
    const currentUrl = page.url();
    const currentHostname = (() => { try { return new URL(currentUrl).hostname; } catch { return ""; } })();

    const isSignedIn =
      (currentHostname === "myaccount.google.com" ||
       currentHostname === "accounts.google.com") ||
      pageText.includes("Google Account") && (
        pageText.includes("Personal info") ||
        pageText.includes("Security") ||
        pageText.includes("Welcome")
      );

    if (isSignedIn) {
      console.log("[gpay:login] Already signed in to Google!");
      console.log(
        "[gpay:login] Saving session. Close the browser window or press Ctrl+C to finish."
      );
    } else {
      console.log(
        "[gpay:login] Sign-in page detected. Please complete sign-in in the browser window."
      );
      console.log("[gpay:login] Waiting for sign-in to complete...");

      // Wait until the URL changes to myaccount.google.com or similar
      // indicating successful sign-in
      const deadline = Date.now() + options.timeout;
      let signedIn = false;

      while (Date.now() < deadline && !signedIn) {
        const url = page.url();
        const hostname = (() => { try { return new URL(url).hostname; } catch { return ""; } })();
        const text = await page
          .evaluate(() => document.body?.innerText ?? "")
          .catch(() => "");

        signedIn =
          hostname === "myaccount.google.com" ||
          (hostname === "accounts.google.com" &&
            !url.includes("signin") &&
            !url.includes("ServiceLogin") &&
            (text.includes("Personal info") ||
              text.includes("Security") ||
              text.includes("Welcome")));

        if (!signedIn) {
          await new Promise((r) => setTimeout(r, 1000));
        }
      }

      if (!signedIn) {
        throw new Error(
          `Sign-in not completed within ${Math.round(options.timeout / 60_000)} minutes. ` +
            "Run again with --timeout to increase the time limit."
        );
      }

      console.log("[gpay:login] Sign-in detected!");
    }

    // Also navigate to pay.google.com to ensure cookies are set for that domain
    console.log("[gpay:login] Visiting pay.google.com to capture payment cookies...");
    try {
      await page.goto("https://pay.google.com", {
        waitUntil: "networkidle",
        timeout: 15_000,
      });
      // Brief pause to let cookies settle
      await page.waitForTimeout(2000);
    } catch {
      console.log(
        "[gpay:login] Warning: Could not load pay.google.com — GPay tests may still require popup sign-in."
      );
    }

    // Save session
    await context.storageState({ path: storageStatePath });
    console.log();
    console.log(`[gpay:login] Session saved to: ${storageStatePath}`);
    console.log("[gpay:login] GPay token generation will now use this session automatically.");
    console.log();
    console.log(
      "[gpay:login] If GPay tests later show a sign-in screen, re-run: npm run gpay:login"
    );
  } finally {
    await context.close().catch(() => undefined);
    await browser.close().catch(() => undefined);
  }
}

main().catch((error) => {
  const message = error instanceof Error ? error.message : String(error);
  console.error(`[gpay:login] Error: ${message}`);
  process.exit(1);
});
