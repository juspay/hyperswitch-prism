# Apple Pay Token Generator — Continuation Notes

This document captures the full state of the Apple Pay token generator so work can resume
without losing context.

---

## Current Status

**Semi-complete. Blocked on two things the developer must obtain:**

1. Apple Developer account → Apple merchant certificate + private key
2. One-time Safari SafariDriver setup on the Mac being used

The code is fully written and compiles cleanly. Nothing needs to be rewritten.

---

## What Is Built

| File | Status | Notes |
|------|--------|-------|
| `src/applepay-token-gen.ts` | Done | Full automation script using real Safari/SafariDriver |
| `applepay/apay-token-gen.html` | Done | Hosted Apple Pay page with full ApplePaySession flow |
| `applepay/configs/stripe.json` | Done | Example config for Stripe connector |
| `applepay/configs/cybersource.json` | Done | Example config for Cybersource connector |
| `applepay/.well-known/` | Empty | Needs `apple-developer-merchantid-domain-association` file from Apple Developer portal |
| `netlify.toml` | Done | Publishes root dir; CSP headers allow Apple endpoints; deployed |

**Deployed URLs:**
- Apple Pay page: `https://shimmering-pegasus-24c886.netlify.app/applepay/apay-token-gen.html`
- Google Pay page: `https://shimmering-pegasus-24c886.netlify.app/gpay/gpay-token-gen.html`

---

## Architecture

```
npm run apay -- [flags]
        │
        ├─ Reads config from --config <json>
        │  OR --creds ~/Downloads/creds.json --connector <name>
        │
        ├─ Starts local HTTP server on port 7777 (merchant validation proxy)
        │     POST /applepay/validate
        │       ← receives { validationURL, merchantId, displayName, initiativeContext }
        │       → calls Apple's validationURL with mTLS (your cert + key)
        │       ← returns merchantSession JSON to browser
        │
        ├─ Launches real Safari via SafariDriver (W3C WebDriver)
        │     (ApplePaySession is only available in real Safari on macOS)
        │
        ├─ Navigates to hosted HTTPS page with config as URL query params
        │
        ├─ Clicks the Apple Pay button (automated)
        │
        ├─ Page fires onvalidatemerchant → POSTs to localhost:7777 (automated)
        │
        ├─ [MANUAL STEP] User approves with Touch ID / Face ID / passcode
        │
        └─ Captures PKPaymentToken → prints/saves in connector-service format
```

### Why Real Safari (not Playwright WebKit)

Playwright bundles a stripped-down WebKit build that does **not** expose
`window.ApplePaySession`. This was confirmed in both headed and headless modes.
Apple Pay web requires the full Safari browser on macOS — only real Safari has the
payment APIs. The script therefore uses `selenium-webdriver` + `safaridriver`
(Apple's own WebDriver implementation, ships with macOS).

---

## Blockers Before First Run

### Blocker 1 — Apple Developer Account & Merchant Certificate

Apple Pay merchant validation requires a certificate issued by Apple.
You need an **Apple Developer Program membership** (paid, $99/year).

**Steps:**

```bash
# 1. Generate a private key and CSR
openssl genrsa -out merchant.key 2048
openssl req -new -key merchant.key -out merchant.csr \
  -subj "/CN=merchant.com.yourcompany.test"

# 2. Go to https://developer.apple.com/account/
#    → Certificates, Identifiers & Profiles
#    → Identifiers → Merchant IDs → Register a Merchant ID
#    (e.g. merchant.com.yourcompany.test)

# 3. Under the Merchant ID → Create Certificate
#    Upload merchant.csr → download merchant.cer

# 4. Convert DER → PEM
openssl x509 -inform der -in merchant.cer -out merchant.pem

# You now have: merchant.pem  merchant.key
```

### Blocker 2 — Apple Pay Domain Registration

The hosted page domain must be registered with Apple.

**Steps:**

1. In Apple Developer portal → your Merchant ID → Add Domain
2. Apple provides a file to place at:
   `https://<your-domain>/.well-known/apple-developer-merchantid-domain-association`
3. Place that file at:
   `browser-automation-engine/applepay/.well-known/apple-developer-merchantid-domain-association`
4. Redeploy Netlify: `cd browser-automation-engine && npx netlify-cli deploy --prod`
5. Verify: Apple will fetch the file during domain registration

The domain to register is `shimmering-pegasus-24c886.netlify.app`
(or whatever domain is in `initiative_context` in `creds.json`).

### Blocker 3 — One-time SafariDriver Setup (per Mac)

```bash
# 1. Open Safari → Settings → Advanced → check "Show features for web developers"
# 2. Safari menu bar → Develop → Allow Remote Automation
# 3. Enable SafariDriver (run once, requires password):
sudo safaridriver --enable
```

This is a one-time step per Mac. SafariDriver ships with macOS — no extra install needed.

---

## Run Commands

### Using creds.json (recommended)

```bash
cd browser-automation-engine

npm run apay -- \
  --cert /path/to/merchant.pem \
  --key  /path/to/merchant.key \
  --merchant-id merchant.com.yourcompany.test \
  --creds ~/Downloads/creds.json \
  --connector stripe \
  --url https://shimmering-pegasus-24c886.netlify.app/applepay/apay-token-gen.html \
  --pretty
```

### Using a standalone config file

```bash
npm run apay -- \
  --config applepay/configs/stripe.json \
  --cert /path/to/merchant.pem \
  --key  /path/to/merchant.key \
  --url https://shimmering-pegasus-24c886.netlify.app/applepay/apay-token-gen.html \
  --pretty
```

### Save token to file

```bash
npm run apay -- \
  --creds ~/Downloads/creds.json --connector cybersource \
  --cert merchant.pem --key merchant.key \
  --merchant-id merchant.com.yourcompany.test \
  --url https://shimmering-pegasus-24c886.netlify.app/applepay/apay-token-gen.html \
  --output token.json --pretty
```

### All CLI flags

| Flag | Default | Description |
|------|---------|-------------|
| `--cert <path>` | required | Apple merchant certificate PEM |
| `--key <path>` | required | Apple merchant private key |
| `--url <url>` | required | Hosted Apple Pay page (HTTPS) |
| `--config <path>` | — | Standalone JSON config file |
| `--creds <path>` | — | Path to creds.json |
| `--connector <name>` | — | Connector name in creds.json |
| `--merchant-id <id>` | — | Apple merchant ID (required with --creds) |
| `--output <path>` | — | Write token JSON to file |
| `--validation-port <n>` | `7777` | Local merchant validation server port |
| `--timeout <ms>` | `300000` | Total wait time for user auth (5 min) |
| `--pretty` | false | Pretty-print JSON output |
| `--screenshots <dir>` | `applepay/screenshots/` | Directory for debug screenshots |

---

## Config File Format

`applepay/configs/stripe.json` (edit `merchantId`, `certPath`, `keyPath`):

```json
{
  "merchantId": "merchant.com.yourcompany.test",
  "merchantName": "stripe",
  "amount": "1.00",
  "currency": "USD",
  "countryCode": "US",
  "supportedNetworks": ["visa", "masterCard", "amex", "discover"],
  "merchantCapabilities": ["supports3DS"],
  "initiativeContext": "hyperswitch-demo-store.netlify.app",
  "certPath": "/path/to/merchant.pem",
  "keyPath": "/path/to/merchant.key"
}
```

`creds.json` shape expected under each connector:

```json
{
  "<connector>": {
    "metadata": {
      "apple_pay_combined": {
        "simplified": {
          "session_token_data": {
            "initiative_context": "your-domain.netlify.app"
          },
          "payment_request_data": {
            "label": "My Store",
            "supported_networks": ["visa", "masterCard", "amex"],
            "merchant_capabilities": ["supports3DS"]
          }
        }
      }
    }
  }
}
```

---

## Token Output Format

The script outputs a JSON object in connector-service format:

```json
{
  "connector": "stripe",
  "merchantId": "merchant.com.yourcompany.test",
  "payment_method": {
    "apple_pay": {
      "payment_data": {
        "encrypted_data": "<base64 of PKPaymentData>"
      },
      "payment_method": {
        "display_name": "Visa 1111",
        "network": "Visa",
        "type": "debit"
      },
      "transaction_identifier": "<txn_id>"
    }
  },
  "raw_token": { "...": "full PKPaymentToken for reference" }
}
```

---

## Resumption Checklist

When resuming work on this, go through in order:

- [ ] Obtain Apple Developer Program membership (if not already)
- [ ] Create Merchant ID in Apple Developer portal (e.g. `merchant.com.yourcompany.test`)
- [ ] Generate CSR, upload to Apple, download `merchant.cer`, convert to `merchant.pem`
- [ ] Register domain `shimmering-pegasus-24c886.netlify.app` with the Merchant ID in Apple portal
- [ ] Download the domain association file from Apple portal and place at:
      `applepay/.well-known/apple-developer-merchantid-domain-association`
- [ ] Redeploy Netlify: `cd browser-automation-engine && npx netlify-cli deploy --prod`
- [ ] Run one-time SafariDriver setup: `sudo safaridriver --enable` + Safari Develop menu
- [ ] Update `applepay/configs/stripe.json` and `cybersource.json` with real `merchantId`
- [ ] Run the script with `--creds` or `--config` and approve Touch ID prompt
- [ ] Verify token output contains `payment_method.apple_pay.payment_data.encrypted_data`

---

## Key Files

```
browser-automation-engine/
├── src/applepay-token-gen.ts          # Main automation script (selenium-webdriver + SafariDriver)
├── applepay/
│   ├── apay-token-gen.html            # Hosted page (ApplePaySession flow)
│   ├── configs/
│   │   ├── stripe.json                # Example config
│   │   └── cybersource.json           # Example config
│   ├── .well-known/                   # EMPTY — needs apple-developer-merchantid-domain-association
│   └── screenshots/                   # Debug screenshots written here at runtime
├── netlify.toml                       # Serves root; CSP allows Apple endpoints
└── package.json                       # "apay": "tsx src/applepay-token-gen.ts"
```
