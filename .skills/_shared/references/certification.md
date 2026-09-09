# Certification Reference

**Certification is merge-blocking.** It stopped being advisory on 2026-08-31
(`75079740f`, "ci: make connector certification a merge-blocking gate with regression
attribution"). A branch that compiles, passes clippy and answers grpcurl by hand is **not**
done. This file is the definition of done for a connector PR.

Three separate CI mechanisms are involved. They fail for different reasons, at different
times, and only one of them has an escape hatch. Do not conflate them.

| Mechanism | Where | Scope | Escape hatch |
|---|---|---|---|
| `check_connector_specs` | `crates/internal/integration-tests/src/bin/check_connector_specs.rs`, run in the **Compilation Check** job | **Every** connector in the repo, every run | none |
| `verify-new-connectors.sh` | `.github/scripts/verify-new-connectors.sh`, run in the **Run Tests** job | Connectors **added by this PR** | `alpha_connectors.json` + a `reason` |
| `certify-connectors.sh` | `.github/scripts/certify-connectors.sh`, run in the **Run Tests** job | Connectors this PR **touched**, plus a sweep when shared code changed | merge-base arbitration |

---

## 1. `check_connector_specs` — the structural check every connector faces

CI step (`.github/workflows/ci.yml`, job `check`, name "Compilation Check"):

```yaml
- name: Run Connector Specs Coverage Check
  run: cargo run --all-features --bin check_connector_specs
```

The `check` job's `if:` gates on `any-rust || proto || ci`, so for any connector PR it
runs — and once it runs it checks **every** connector in the repo, not just yours. It needs
no credentials and no network. **Run it locally before pushing**; it is the cheapest way to
find out that the branch cannot merge:

```bash
cargo run --bin check_connector_specs   # expect: "All checks passed. OK."
```

Its `main()` exits 1 if **any** of these hold (see the `reasons` vector at the end of
`main`):

| Phase | Fails when | Fix |
|---|---|---|
| **1 — connector list parity** | A `.rs` file under `connectors/` (excluding `macros.rs`) has no matching `connector_specs/<name>/` directory, **or** a spec directory has no connector file | Create `connector_specs/<name>/specs.json`. This is why scaffolding without a specs.json produces a branch that cannot pass CI. |
| **2 — flow → suite coverage** | A flow the connector implements maps to a suite that is absent from that connector's `supported_suites` | Add the suite, or remove the flow |
| **2 — unknown flow** | A flow is in neither `flow_to_suites` nor `OUT_OF_SCOPE_FLOWS` | Triage the flow in `check_connector_specs.rs` first — see §5 |
| **2 — unreadable** | The connector's `.rs` file could not be read | — |
| **2b — webhook coverage** | `supported_suites` contains `"EventService/HandleEvent"` but there is no `connector_specs/<name>/webhook_payload.json` | Add a **real captured** payload fixture, or drop the suite |
| **3 — testable suite report** | never (informational only) | — |

Phase 2 reads the flows a connector implements from **both** declaration forms —
`create_all_prerequisites!(api: [ flow: X, ... ])` **and** hand-written
`impl ... ConnectorIntegrationV2<X, ...>` blocks. A hand-rolled `impl` does not hide a
flow from this check.

`IGNORE_SERVICES = ["PayoutService", "DisputeService"]` scopes Phase 3's *report* only. It
exempts nothing from Phases 1 and 2. There is no `dispute_connectors/` directory — dispute
flows are declared on ordinary `connectors/*.rs` files (`adyen.rs` among them), so those
connectors are fully inside Phases 1 and 2, and their dispute flows clear Phase 2 through
`OUT_OF_SCOPE_FLOWS` (§5), not through `IGNORE_SERVICES`.

What is genuinely out of reach is the sibling directories: Phase 1 walks `connectors/`, not
`payout_connectors/`, `authenticator_connectors/` or `surcharge_connectors/`, so those
siblings need **no** `connector_specs` entry at all.

---

## 2. `verify-new-connectors.sh` — the gate on a brand-new connector

### What counts as "new"

`.github/workflows/ci.yml`, step **"Detect newly added connectors"**: a connector is new
when its spec directory does not exist at the PR's base commit.

```bash
spec_dir="crates/internal/integration-tests/src/connector_specs/$c"
if ! git cat-file -e "${base_ref}:${spec_dir}" 2>/dev/null; then
  new_list="${new_list}${new_list:+,}${c}"
fi
```

`base_ref` is `pull_request.base.sha` / `merge_group.base_sha` / `event.before` — the
merge base, not the immediate parent, so a multi-commit PR is classified correctly. **A
connector cannot be made "not new" by splitting the PR**; the spec dir either existed on
the target branch or it did not.

Check your own branch the same way CI does:

```bash
git cat-file -e "$(git merge-base origin/main HEAD):crates/internal/integration-tests/src/connector_specs/<name>" \
  && echo "existing connector" || echo "NEW — verify-new-connectors.sh applies"
```

### The hard-fail paths

All of them are `exit 1` / `failures++` in `.github/scripts/verify-new-connectors.sh`:

1. **`CONNECTOR_AUTH_FILE_PATH` unset or the file is empty** —
   `"New connectors were added but no credentials file is available — cannot verify their
   declared scenarios."` This is an immediate `exit 1` before any per-connector loop.
   Note the workflow deliberately does **not** fail when S3/GPG decryption hiccups; it
   emits a warning and lets *this* step be the one that fails, and only when a new
   connector was actually added.
2. **No `specs.json`** at `connector_specs/<name>/specs.json`.
3. **Empty `supported_suites`** — `jq '.supported_suites // [] | length'` is `0`.
   *"List every suite the connector supports; nothing is certified without it."*
4. **Listed in `alpha_connectors.json` with no `reason`** — see §3. A bare `{}` entry
   fails for a new connector.
5. **Not listed in alpha AND absent from the CI credentials file** —
   `jq -e --arg c "<name>" 'has($c)' "$CONNECTOR_AUTH_FILE_PATH"`. Not being on the alpha
   list is a *claim of certification*, and a certified connector with no credentials would
   be silently skipped by the sweep — a connector missing from the report reads as "no
   regression".
6. **Any declared scenario fails.** For a connector that clears all of the above:

   ```bash
   ./scripts/run-tests --skip-setup --no-build --connector <name> --interface grpc --report
   ```

   Every scenario of every suite in `supported_suites` runs against the live sandbox.
   There is **no merge-base arbitration here** — a new connector has no merge-base
   behaviour to compare against, so one failing scenario blocks the merge outright.

### Fork PRs do not get this gate

The **Run Tests** job sets

```
RUN_TESTS: push || (pull_request && head.repo.full_name == base.repo.full_name) || merge_group
```

so on a fork PR both certification steps are skipped. It merges through the merge queue,
where `merge_group` makes `RUN_TESTS` true again. Do not read a green fork PR as
certified.

---

## 3. `alpha_connectors.json` — the escape hatch, and its price

`crates/internal/integration-tests/src/connector_specs/alpha_connectors.json`:

```json
{
  "connectors": {
    "<name>": { "reason": "why it cannot be proven yet" }
  }
}
```

**What listing does:** `certify-connectors.sh`'s `is_alpha()` skips the connector (no
credentials in CI, nothing to certify), and `verify-new-connectors.sh` records it as
*unproven* instead of running its scenarios.

**What it costs:**

- The `reason` is **mandatory for a new connector** and must be non-empty. An entry of
  `{}` is `exit 1` with
  *"State why it cannot be proven yet, e.g. \"sandbox access requested, pending
  <team/ticket>\"."*
  (Most of the ~100 pre-existing entries are bare `{}`; they are grandfathered because the
  new-connector gate only inspects the connectors this PR added. Do not copy their shape.)
- A `::warning::` in the log: *"'<name>' is merging WITHOUT live sandbox proof."*
- A section in the job summary: *"⚠️ New connectors merging without live sandbox proof"*,
  each line ending *"Confirm the stated reason is real before approving."*
- An automatically posted (and updated) **PR comment** carrying the marker
  `<!-- unproven-connectors -->`: *"No real sandbox call backs them … they do not certify
  until the entry is removed."*

Reasons currently in the file are the model to imitate — specific, and stating what would
remove the entry:

> `paynearme`: "No PayNearMe sandbox credentials exist; the integration was verified
> against a local spec-shaped mock server (outbound request + response parsing) rather than
> the live sandbox."

**Removal is a promotion, not a cleanup.** `ci.yml`'s "Detect connectors with a modified
specs.json or override.json" step diffs the alpha list against the base commit and adds
every *removed* name to `SPEC_MODIFIED_CONNECTORS` — which certification treats as having
**no arbitration escape hatch**, because the newly exposed scenarios never ran at the merge
base. Deleting a name pulls the connector into the full sweep in that same PR.

For the same reason, **`add_connector.sh` deliberately does not touch this file.** Add the
entry by hand.

`live_connectors.json` is the mirror image: connectors that carry real production traffic
(`adyen`, `cybersource`, `stripe`, `tsys`). Adding a name there also lands in
`SPEC_MODIFIED_CONNECTORS`.

---

## 4. `specs.json` is a claim, not a scaffold artifact

`add_connector.sh` seeds `connector_specs/<name>/specs.json` with the six core flows
(`Authorize,PSync,Capture,Void,Refund,RSync`) and merges — never overwrites — an existing
file. That seed is a *starting point*, and the script says so:

> `specs.json` is a CERTIFICATION CLAIM, not boilerplate. Before pushing:
> - trim it to the suites the connector ACTUALLY implements; `check_connector_specs` reads
>   the flows straight out of `<name>.rs`, and `.github/scripts/verify-new-connectors.sh`
>   runs every scenario of every declared suite against the live sandbox.
> - run `cargo run --bin check_connector_specs` and expect "All checks passed".

The two directions fail differently, and both are blocking:

- **Declared but not implemented** → the scenario runs against the sandbox and fails →
  `verify-new-connectors.sh` exits 1.
- **Implemented but not declared** → `check_connector_specs` Phase 2 reports
  `MISSING flow=… suite=…` and exits 1.

`--flows` takes **`check_connector_specs` flow names**, not the trait names
`--list-flows` prints. The accepted vocabulary is the `flow_to_suite` table in the script:
`Authorize, PSync, Capture, Void, Refund, RSync, SetupMandate, RepeatPayment,
MandateRevoke, CreateConnectorCustomer, GetConnectorCustomer, PaymentMethodToken,
PaymentMethodEligibility, ServerAuthenticationToken, ClientAuthenticationToken,
ServerSessionAuthenticationToken, PreAuthenticate, Authenticate, PostAuthenticate,
CreateOrder, IncrementalAuthorization`. An unrecognised name aborts the run with
*"Refusing to write a specs.json that CI would reject"*.

`EventService/HandleEvent` maps from no flow, so it can only reach the file by hand — and
the moment it is there, Phase 2b demands a real `webhook_payload.json` fixture next to it.
The scaffold script does not fabricate one.

---

## 5. "Out of scope" does **not** mean "do not build it"

`add_connector.sh`'s `is_out_of_scope_flow()` reads as a list of flows *excluded from
certification*, which is easy to misread as a list of flows that should never be
implemented. **That reading is wrong.** The Rust source it mirrors says the opposite, in
`OUT_OF_SCOPE_FLOWS` in `check_connector_specs.rs`:

```rust
// Implemented today with no suite to map to. Each is a coverage gap, not a
// decision that it should never be covered: add a suite, then move the flow
// into flow_to_suites above.
"VoidPC",              // 14 connectors
"VerifyWebhookSource", // 2 connectors
"VoidPostRefund",
"Recharge",
"CreatePaymentMethod",
"GetPaymentMethod",
"RefreshPaymentMethod",
"PreRiskCheck",
"PostRiskCheck",
"FrmPaymentOutcome",
"FrmRefundProcessed",
"FrmChargebackReceived",
```

The list exists so that a *new* flow cannot slip through uncovered because nobody updated
the mapping — the comment on the constant names `VoidPC` as the flow that "came to be
implemented by 14 connectors and certified by none". Implement the flow when the tech spec
calls for it; it simply contributes no suite to `supported_suites`.

Only the first two groups are genuine decisions: dispute flows (`Accept`, `DefendDispute`,
`SubmitEvidence`) have no suites yet, and payout flows are out of scope for the payment
suites by design.

---

## 6. Definition of done for a connector PR

`cargo build` is step 1 of 8, not the finish line.

1. **Builds**: `cargo build --package connector-integration` — zero errors, zero warnings.
2. **Structural check passes**: `cargo run --bin check_connector_specs` prints
   `All checks passed. OK.`
3. **`specs.json` trimmed** to the suites actually implemented (§4), and
   `webhook_payload.json` present if `EventService/HandleEvent` is declared.
4. **Certification decided, in writing** — either
   - credentials for the connector exist in the CI creds file and every declared scenario
     passes `./scripts/run-tests --connector <name> --interface grpc --report`, **or**
   - an `alpha_connectors.json` entry with a specific, non-empty `reason` that states what
     would remove it (§3), understanding that the PR will carry a public "merging without
     live sandbox proof" comment.
5. **Proto ordinal re-checked against the branch head** before opening the PR — someone
   may have merged an enum value first:

   ```bash
   git fetch origin main
   git show origin/main:crates/types-traits/grpc-api-types/proto/payment.proto \
     | sed -n '/^enum Connector {/,/^}/p' \
     | grep -oE '= [0-9]+;' | grep -oE '[0-9]+' | sort -n | tail -1
   ```

   If that is `>=` the ordinal your branch claimed, renumber.
6. **Superposition + URL patching landed**: the connector's sandbox and production
   `connector_base_url` overrides are in `config/superposition.toml` and the arm exists in
   `Connectors::patch_connector_urls` (`crates/types-traits/domain_types/src/types.rs`).
   `cargo test -p grpc-server --all-features --test test_superposition_config` is the CI
   check.
7. **Quality gate clean**: `.skills/_shared/references/quality-checklist.md` §15 (Pre-Flight Gate),
   including the PR-body disclosure of any novel algorithmic logic.
8. **Evidence regenerated against the branch head** after the last fix commit. Request and
   response captures from an earlier revision are not evidence for the code being merged.

### Not applicable to payout / authenticator / surcharge / FRM connectors

`payout_connectors/`, `authenticator_connectors/` and `surcharge_connectors/` are
**siblings** of `connectors/`, not subdirectories, and `check_connector_specs` Phase 1 only
walks `connectors/`. A payout connector therefore needs **no `connector_specs` entry and no
`superposition.toml` entry**, and none of §1–§4 applies to it. Steps 1, 5 and 7 above still
do.

---

## 7. Reading a red certification job

- **`::error::New connector '<x>' has no specs.json`** → §1 Phase 1 / §2 fail path 2.
- **`::error::New connector '<x>' declares no "supported_suites"`** → §2 fail path 3.
- **`::error::… is listed in alpha_connectors.json with no "reason"`** → §3.
- **`::error::… has no entry in the CI credentials file`** → §2 fail path 5: add
  credentials, or list it in alpha with a reason. Nothing else clears it.
- **`::error::Verification failed for new connector <x>`** → a declared scenario failed
  against the sandbox. Either the implementation is wrong or the claim in `specs.json` is.
  Reports are uploaded as the `certification-reports` artifact
  (`${runner.temp}/certify-*.json`, 2-day retention) and contain the recorded request and
  response for every scenario — read those before guessing.
- **`ERROR: N connector(s) implement a flow with no suite mapping`** → §5: triage the flow
  into `flow_to_suites` or `OUT_OF_SCOPE_FLOWS` in `check_connector_specs.rs`.
- **A failure in `certify-connectors.sh` on a connector you did not touch** → that script
  re-runs the failing scenario at the merge base and only blocks when the scenario passed
  there and fails on HEAD. If it blocked, this PR is implicated; a pre-existing breakage is
  reported but does not block.

---

## Source index

| Fact | Verify with |
|---|---|
| Gate landed 2026-08-31 | `git log -1 --format='%H %ad %s' --date=short 75079740f` |
| New-connector fail paths | `.github/scripts/verify-new-connectors.sh` |
| "New" = spec dir absent at base | `.github/workflows/ci.yml`, step "Detect newly added connectors" |
| Alpha promotion → no arbitration | `.github/workflows/ci.yml`, step "Detect connectors with a modified specs.json or override.json" |
| Merge-base arbitration | `.github/scripts/certify-connectors.sh` header comment |
| Phases and exit conditions | `crates/internal/integration-tests/src/bin/check_connector_specs.rs` (module doc + `main`) |
| Coverage gap ≠ decision | `OUT_OF_SCOPE_FLOWS` in the same file |
| specs.json seeding and merge semantics | `generate_connector_specs()` in `grace/rulesbook/codegen/add_connector.sh` |
| Fork PRs skip certification | `RUN_TESTS` expression on the `test` job in `.github/workflows/ci.yml` |
