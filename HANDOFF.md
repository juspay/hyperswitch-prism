# Connector certification CI — pending work

Context for picking this up cold. PR juspay/hyperswitch-prism#2086, branch
`poc/test-ucs-stripe-authorize-ci`.

## What the PR does

CI never made a live call to any of the 103 connectors, so a PR could break a
connector's real behaviour and merge green. This adds a live-sandbox gate that
blocks merge — but only on failures it can *prove* the PR caused.

A scenario that fails is re-run at HEAD (absorbing flakes), then re-run against
the PR's merge base. It blocks only if it **passes at the base and fails at
HEAD**. Everything else warns and exits 0, so a sandbox outage or pre-existing
breakage never blames an innocent PR.

Key files:

- `.github/workflows/ci.yml` — connector selection, credentials fetch, artifact upload
- `.github/scripts/certify-connectors.sh` — the three passes and the verdicts
- `.github/scripts/verify-new-connectors.sh` — gate for connectors added by the PR
- `crates/internal/integration-tests/src/connector_specs/<name>/specs.json` — what a
  connector supports (`supported_suites`, `supported_payment_methods`,
  `unsupported_scenarios`, `live_in_production`)
- `crates/internal/integration-tests/src/connector_specs/alpha_connectors.json` — the
  101 connectors with no CI credentials, therefore never certified

Self-tests live outside the repo in `~/gopi/`: `ucs-ci-arbitration-selftest.sh` (9),
`ucs-ci-selection-selftest.sh` (27), `ucs-ci-newconnector-gate-selftest.sh` (7). They
drive the real scripts. Run them after any change to either script.

## Current certification state

`live_in_production` is **stripe only**. fiserv and tsys were removed: fiserv's
sandbox fails 44–47% of calls (measured, n=25 and n=30 — 503 GATEWAY_ERROR at ~1.9s
and 504 TIMEOUT at ~23s), and tsys's credentials are rejected. Both are still
certified when their own files change; they just no longer run on every shared
change.

Last full run: stripe 128 passed / 24 failed / 1 skipped.

## Pending — stripe's 24 failures

Three are already fixed in `ba9dab383` (decline scenarios could not pass because a
4xx decline carries no response body; the harness now lifts the error out of the
gRPC status) but are unverified until the next run.

Of the rest:

| n | scenarios | connector said | fix |
|---|---|---|---|
| 4 | all of `EventService/HandleEvent` | `Failed to decode webhook event body` | needs `webhook_secret` in the CI credentials file — infra, not code |
| 4 | `PaymentService/IncrementalAuthorization` | *not eligible … you did not request support using `request_incremental_authorization`* | the Authorize dependency must set that flag |
| 3 | `proxy_auto_capture_card`, `token_auto_capture_credit_card`, `token_manual_capture_credit_card` | `Missing required field: address` | scenario is missing `address` |
| 4 | `RecurringPaymentService/Charge` | *The provided PaymentMethod cannot be attached* | mandate setup, unconfirmed |
| 4 | ach / bacs / giropay / sepa | customer required · GBP unsupported · `setup_future_usage` rejected · US country | stripe `override.json` |
| 2 | `proxy_setup_mandate`, `token_setup_mandate` | error shows the echoed `x-connector-config`, i.e. no real response | needs investigation |

**Fix the request, never the assertion.** Relaxing an assertion to pass is the
failure mode this PR exists to prevent — `fiserv/override.json` previously deleted
both error checks *and* added `CHARGED` to a test named "fail payment", so a
declined card being charged counted as a pass. That override is gone; do not
recreate the pattern. If a connector genuinely cannot support a scenario, declare
it in `unsupported_scenarios` with a reason.

## Pending — blocked on credentials

The CI credentials file is one GPG-encrypted object in S3, fetched in `ci.yml`
(`CONNECTOR_CREDS_S3_BUCKET_URI` secret + `S3_SOURCE_FILE_NAME`).

- **stripe** — confirm the block has a `webhook_secret` key. `webhook_payload.json`
  declares `"webhook_secret_key": "webhook_secret"`, and the harness reads that name
  out of the credentials (`connector_override/mod.rs`, `load_webhook_secret`). It
  returns `Option`, so an absent key means no secret is injected and the signature is
  computed against nothing — which is the likely cause of all four webhook failures.
  **Unproven**: the S3 file cannot be read from a dev machine.
- **tsys** — TSYS returns `F9901 — The value of element 'transactionKey' is not
  valid`. Left alone for now.

## Pending — before merge

**Arbitration's blocking path has never run for real.** Every verdict CI has produced
is `NOT_ATTRIBUTABLE` (warn, exit 0). The `REGRESSION` path — pass at base, fail at
HEAD, exit 1 — exists only in the self-test stubs. Since this gate becomes a required
check, a bug there is discovered either by it never blocking (silent) or by it
blocking every PR after merge (loud and bad).

The test is a deliberate temporary commit: push a scenario that fails at HEAD and
passes at the merge base, confirm the build blocks with
`::error::<connector> (<suite> / <scenario>) passes at the merge base and fails here`,
then push one broken on both sides and confirm it only warns. Revert both.

**Issue #22833** (hyperswitch-cloud) still documents the abandoned
`verified_scenarios` design and is circulating to the euler team. Rewrite it for the
global-suite model once the above lands.

## Things that will bite you

- **`suite_spec.json`, not `suite.json`.** Grepping the wrong name returns zero and
  reads like "the claim was false".
- **`dependency_scope: "scenario"`** re-runs the whole dependency chain per scenario.
  48 scenarios becomes ~200 gRPC calls. It is why sweeps are slow, not the scenario
  count.
- **Every scenario failing on main costs 2 extra runs on every later PR** (one
  re-check, one arbitration). The merge-base rebuild is paid once, not per failure.
- **Base runs use the base's `specs.json` and scenarios**, not the PR's —
  `CHECKOUT_PATHS` replaces all of `crates/`. So while this PR is open, base-vs-HEAD
  errors legitimately differ on any scenario it edits.
- **Timeouts**: `ATTEMPT_TIMEOUT` 300s per connector sweep, `SCENARIO_TIMEOUT` 90s per
  single scenario, and the server's own `CS__PROXY__CONNECTOR_REQUEST_TIMEOUT` 10s per
  call (set in `scripts/grpc-server.sh`).
- **Certification reports are uploaded as the `certification-reports` artifact** with
  2-day retention. They hold the full request and response for every scenario — far
  more than the console shows.
