# Browser Automation Engine v1

Rule-based browser automation runtime built on top of Playwright.

## What it does

- Exposes `POST /run`
- Accepts `url` and a JSON DSL `rules` array
- Supports both API and CLI execution modes
- Launches Playwright Chromium in headless mode by default
- Uses a fresh isolated browser context per request
- Executes rules sequentially
- Returns structured pass/fail with per-step details and extracted data

This is a JSON-DSL layer over Playwright, not a Playwright replacement.

## Tech stack

- Node.js (>=18)
- TypeScript
- Fastify
- Playwright (`headless: true` by default)

## Project structure

```text
browser-automation-engine/
  package.json
  tsconfig.json
  README.md
  src/
    server.ts
    api/
      routes.ts
    engine/
      automationEngine.ts
      interpreter.ts
    drivers/
      browserDriver.ts
      playwrightDriver.ts
    types/
      dsl.ts
      api.ts
    utils/
      validation.ts
```

## Setup

```bash
cd browser-automation-engine
npm install
npm run install:browsers
```

## Run

Development:

```bash
npm run dev
```

Production:

```bash
npm run build
npm start
```

The server starts at `http://localhost:3000` by default.

## CLI

Run automation directly from local JSON input without starting the server:

```bash
npm run cli -- --input examples/sample-request.json --pretty
```

Run in headed mode so you can watch browser actions:

```bash
npm run cli -- --input examples/paypal-3ds-accept.json --headed --slow-mo 250 --pretty
```

Note: update `cart_id` in `examples/paypal-3ds-accept.json` with a fresh value from your latest 3DS authorize response.

Optional CLI flags:

- `--input <path>` required input JSON file
- `--headed` opens visible browser (`headless=false`)
- `--slow-mo <ms>` adds Playwright slow motion delay
- `--output <path>` writes final JSON response to file
- `--pretty` prints formatted JSON

## API

### POST `/run`

#### Request

```json
{
  "url": "https://example.com/login",
  "rules": [
    { "action": "fill", "selector": "#email", "value": "user@test.com" },
    { "action": "fill", "selector": "#password", "value": "secret" },
    { "action": "click", "selector": "#login" },
    { "action": "waitFor", "selector": ".dashboard" },
    { "action": "extract", "selector": ".username", "as": "username" }
  ],
  "options": {
    "headless": true,
    "slowMoMs": 0,
    "defaultTimeoutMs": 10000,
    "navigationTimeoutMs": 20000
  }
}
```

#### Success response

```json
{
  "success": true,
  "data": {
    "username": "Amit"
  },
  "steps": [
    { "index": 0, "action": "fill", "status": "ok", "durationMs": 230 },
    { "index": 1, "action": "fill", "status": "ok", "durationMs": 210 },
    { "index": 2, "action": "click", "status": "ok", "durationMs": 180 },
    { "index": 3, "action": "waitFor", "status": "ok", "durationMs": 1100 },
    { "index": 4, "action": "extract", "status": "ok", "durationMs": 90 }
  ],
  "durationMs": 4200
}
```

#### Failure response

```json
{
  "success": false,
  "failedStep": 2,
  "error": "Element is not visible: #login",
  "data": {},
  "steps": [
    { "index": 0, "action": "fill", "status": "ok", "durationMs": 220 },
    { "index": 1, "action": "fill", "status": "ok", "durationMs": 205 },
    {
      "index": 2,
      "action": "click",
      "status": "failed",
      "error": "Element is not visible: #login",
      "durationMs": 10012
    }
  ],
  "durationMs": 10640
}
```

## Supported DSL actions (v1)

- `goto`
- `click`
- `fill`
- `press`
- `waitFor`
- `assertText`
- `assertVisible`
- `extract`
- `extractAll`
- `screenshot`

## Notes

- The engine always navigates to `url` before processing rules.
- `goto` can be used later in rules to navigate again.
- Screenshots are saved only when a `screenshot` rule is present.
- Browser context is isolated for every request.
- API and CLI both use the same execution engine and rule interpreter.
