# End-to-end tests

These tests exercise payment flows through Hyperswitch and UCS against processor sandbox environments.

## Run locally

Requirements: Node.js 22+, Docker, and a Hyperswitch-compatible connector credentials JSON file.

```sh
cd tests/e2e
npm ci
CONNECTOR_AUTH_FILE_PATH=/path/to/connector-auth.json npm run stack:up
CONNECTOR_AUTH_FILE_PATH=/path/to/connector-auth.json npm test
```

Open the HTML report with `npx playwright show-report`. Stop the isolated stack with `npm run stack:down`.

`HS_IMAGE`, `UCS_IMAGE`, `HS_BASE_URL`, and `HS_SOURCE_DIR` are optional. The defaults run the latest published Hyperswitch and UCS images locally. The source checkout supplies configuration and database migrations matching the Hyperswitch image and is cached after the first run.

## Extend coverage

- `scenarios/` declares payment behavior. Scenario files contain typed data and register tests through the shared runner.
- `connectors/` declares supported capabilities and connector-required test data. It does not implement payment lifecycles.
- `framework/` owns merchant setup, API sequences, assertions, and reporting.
- `stack/` owns the isolated local services.

All compatible registered connectors automatically receive a shared scenario. Add connector-specific data only when the processor requires it; do not copy the scenario or runner.

The current scenario is a server-side, one-time No3DS card payment with automatic capture:

```text
create payment -> confirm payment -> retrieve payment -> verify success and UCS routing
```

Connector credentials are read only at runtime. Do not commit credentials, reports, runtime files, or copied production data.
