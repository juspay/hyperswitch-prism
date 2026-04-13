# Google Pay Token Generator

Generates real encrypted Google Pay payment tokens for connector testing
(Cybersource, Stripe, Adyen, Nuvei, Checkout.com, etc.) using a fully
automated Playwright + WebKit flow — **no human interaction required** after
the initial Google sign-in.

---

## How it works

1. A static HTML page (`gpay-token-gen.html`) is deployed to Netlify (HTTPS
   required — Google's `pay.js` refuses to initialise from `localhost`).
2. The CLI script (`src/gpay-token-gen.ts`) launches a **WebKit** (Safari
   engine) browser via Playwright.
   - WebKit is used because `loadPaymentData()` opens a real catchable
     `window.open()` popup in WebKit/Safari, whereas in Chrome it opens a
     browser-native Payment Handler overlay that Playwright cannot see into.
3. The script navigates to the hosted page with gateway config passed as URL
   query parameters.
4. It catches the Google Pay popup (`pay.google.com`) via
   `context.waitForEvent('page')`, selects a test card, and clicks the Pay
   button — all automated.
5. The resulting `PaymentData` object (including the encrypted token) is
   printed to stdout and optionally saved to a file.
6. The Google login session is persisted in a local storage-state file so
   subsequent runs are fully headless.

---

## Prerequisites

| Requirement | Version |
|---|---|
| Node.js | 18 or later |
| npm | 9 or later |
| Netlify account | free tier — sign up at https://app.netlify.com/signup |
| Google account | any personal Gmail works in TEST mode |

---

## Step 1 — Deploy the HTML page to Netlify

The HTML page must be served over HTTPS. Netlify provides this for free.
Google's `pay.js` refuses to load from `localhost`, so a real HTTPS URL is
required.

### Automatic setup via `make setup-connector-tests` (recommended)

The setup script handles everything — login, site creation, deploy, and saving
the URL. Just run:

```bash
make setup-connector-tests
```

When it reaches the Netlify step, it will:

1. Print a one-time authorization URL in the terminal
2. You open that URL in your browser and click **"Authorize"** (one click —
   no forms if you are already logged in to netlify.com)
3. The script detects the authorization automatically and continues
4. The deployed URL is saved to `.env.connector-tests` — all future runs skip
   this step entirely

If you already have `NETLIFY_AUTH_TOKEN` set in your environment, the browser
step is skipped and the deploy runs fully headlessly.

**To skip Google Pay tests entirely** (no Netlify needed):

```bash
SKIP_NETLIFY_DEPLOY=1 make setup-connector-tests
```

---

### Manual setup (alternative)

If you prefer to set things up yourself outside of the setup script:

**Option A — Personal Access Token (headless/CI-friendly)**

1. Go to https://app.netlify.com/user/applications
2. Under **Personal access tokens**, click **New access token**
3. Give it a name (e.g. `integration-tests`) and copy the token
4. Export it and deploy:

```bash
export NETLIFY_AUTH_TOKEN=<your-token>
cd browser-automation-engine
netlify deploy --prod
```

**Option B — Interactive browser login**

```bash
cd browser-automation-engine
npm install -g netlify-cli
netlify login          # opens browser for OAuth
netlify deploy --prod
```

---

Your page is now live at:
```
https://your-site-name.netlify.app/gpay/gpay-token-gen.html
```

You only need to redeploy if you change `gpay-token-gen.html`.

---

## Step 2 — Install Playwright WebKit (one-time)

```bash
# From browser-automation-engine/
npm install
npm run install:browsers
```

---

## Step 3 — Configure your gateway

Edit the relevant file in `gpay/configs/` and replace the placeholder merchant
ID with your real (or sandbox) value:

```jsonc
// gpay/configs/cybersource.json
{
  "gateway": "cybersource",
  "gatewayMerchantId": "YOUR_CYBERSOURCE_MERCHANT_ID",
  "merchantName": "Test Merchant",
  "amount": "10.00",
  "currency": "USD",
  "countryCode": "US",
  "cardNetworks": ["VISA", "MASTERCARD", "AMEX", "DISCOVER"],
  "authMethods": ["PAN_ONLY", "CRYPTOGRAM_3DS"]
}
```

Supported connectors out of the box: `cybersource`, `stripe`, `adyen`,
`checkout`, `nuvei`.

---

## Step 4 — First run (sign into Google)

The first time you run the tool, the browser opens **headed** (visible) so you
can sign into your Google account. The session is then saved and reused on all
subsequent runs.

```bash
# From browser-automation-engine/
npm run gpay -- \
  --config gpay/configs/cybersource.json \
  --url https://your-site-name.netlify.app/gpay-token-gen.html \
  --headed \
  --pretty
```

1. A browser window opens and navigates to the hosted page.
2. The Google Pay button renders — the script clicks it automatically.
3. The Google Pay popup opens — sign into your Google account if prompted.
4. The script selects the first available test card and clicks **Continue**.
5. The popup closes and the token appears in stdout.
6. Your session is saved to `gpay/.webkit-profile/storage-state.json`.

---

## Step 5 — Subsequent automated runs (headless)

```bash
npm run gpay -- \
  --config gpay/configs/cybersource.json \
  --url https://your-site-name.netlify.app/gpay-token-gen.html \
  --headless \
  --output token.json \
  --pretty
```

No browser window appears. The token is written to `token.json`.

---

## CLI flag reference

| Flag | Default | Description |
|---|---|---|
| `--config <path>` | **required** | Path to a gateway config JSON file |
| `--url <url>` | — | Hosted page URL (strongly recommended). If omitted, a local HTTP server is started but `pay.js` likely won't work. |
| `--headed` | on | Show the browser window |
| `--headless` | off | Hide the browser window |
| `--output <path>` | — | Write full PaymentData JSON to this file |
| `--screenshots <dir>` | `gpay/screenshots` | Directory for debug screenshots |
| `--profile <path>` | `gpay/.webkit-profile` | Directory for the persistent browser session |
| `--timeout <ms>` | `120000` | Total timeout for the entire flow |
| `--popup-timeout <ms>` | `20000` | Timeout waiting for the GPay popup to open |
| `--pretty` | off | Pretty-print JSON output |
| `--help` | — | Print usage |

---

## Output format

```jsonc
{
  "gateway": "cybersource",
  "gatewayMerchantId": "your-merchant-id",
  "paymentData": {
    // Full Google PaymentData object returned by loadPaymentData()
    "apiVersion": 2,
    "apiVersionMinor": 0,
    "paymentMethodData": {
      "type": "CARD",
      "description": "Visa •••• 1234",
      "info": { "cardNetwork": "VISA", "cardDetails": "1234" },
      "tokenizationData": {
        "type": "PAYMENT_GATEWAY",
        "token": "{ /* encrypted token string or JSON */ }"
      }
    }
  },
  "token": {
    // tokenizationData.token parsed as JSON if possible, otherwise raw string
  }
}
```

The `token` field is what you pass to your connector's payment API.

---

## Adding a new gateway

1. Create `gpay/configs/<connector>.json`:
   ```json
   {
     "gateway": "<gateway-name-as-per-google-pay-docs>",
     "gatewayMerchantId": "<your-merchant-id>",
     "merchantName": "Test Merchant",
     "amount": "1.00",
     "currency": "USD",
     "countryCode": "US",
     "cardNetworks": ["VISA", "MASTERCARD"],
     "authMethods": ["PAN_ONLY", "CRYPTOGRAM_3DS"]
   }
   ```
2. Run:
   ```bash
   npm run gpay -- --config gpay/configs/<connector>.json --url <hosted-url> --pretty
   ```

The `gateway` value must match the identifier in
[Google's gateway documentation](https://developers.google.com/pay/api/web/guides/tutorial).

---

## Troubleshooting

### Google Pay button does not render

- The page **must** be on HTTPS. Use `--url` with the Netlify URL — never
  `localhost`.
- Check the `gpay/screenshots/main-page-loaded-*.png` screenshot for the
  `#status` text. If it says `pay.js load error`, check the
  `Content-Security-Policy` header in `netlify.toml`.

### Popup did not open (`popup-timeout` error)

- Run with `--headed` and watch the browser. The most common cause is that the
  Google account session has expired.
- Delete `gpay/.webkit-profile/storage-state.json` and run headed again to
  re-authenticate.

### Session expired between runs

```bash
rm gpay/.webkit-profile/storage-state.json
npm run gpay -- --config gpay/configs/cybersource.json \
               --url https://your-site.netlify.app/gpay-token-gen.html \
               --headed --pretty
```

### Pay button inside popup not found

- Check the screenshots in `gpay/screenshots/` — especially
  `gpay-popup-initial-*.png` and `gpay-popup-before-pay-*.png`.
- Google's popup DOM is obfuscated and selectors may change. The script tries
  ~15 selectors + a full button dump to the log before giving up.
- If the popup opens but the button is not clicked, the script logs all button
  texts — use that to add a new entry to `textCandidates` in
  `src/gpay-token-gen.ts`.

### TypeScript errors

```bash
npm run check
```
