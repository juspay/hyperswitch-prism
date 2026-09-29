# Recurring Reviewer Themes

This file is distilled from human reviewer comments on merged pull requests, not from a style guide. The
prior edition covered the **20 merged PRs #2124–#2327** of `juspay/hyperswitch-prism`, collected
**2026-09-19**: 209 comments, of which 111 were substantive reviewer comments (bot output and author
replies excluded).

The current edition adds the **10 `GRACE-auto` PRs #2278–#2364** and their eight paired Hyperswitch PRs,
collected **2026-09-28**: 102 comments fetched, of which **32 were substantive human reviewer comments** —
the remainder were the pipeline's own overflow comments, bot output, author narrative, empty review records,
and 26 findings from an automated reviewer running under a user account, which are review content but not
human signal. Those 32 human comments are concentrated on three of the ten PRs and come from three reviewers.
**Four of the ten received no review of any kind and three more received only the automated reviewer**, which
is itself the argument for the mechanical checks below: on seven of ten PRs, nothing but this list stood
between the diff and merge. Not one review thread on any of the ten was ever replied to or resolved. Twenty-one themes have recurred across two or
more PRs; each is a `TH-NN` below.

> **Why it exists.** In one connector run, 18 real defects of exactly these shapes survived four passing
> test rounds and were caught only by a post-hoc review — two of them were literal repeats of comments on
> PRs that had already merged. Every one of those late findings cost a plan revision, a codegen pass, an
> environment rebuild and a retest round. Planning against this list costs nothing; rediscovering it costs
> about a third of a run.

Two consumers read it. The **planner** (`grace/workflow/2.3a_plan.md`, Phase 5 / §4 `review_themes[]`)
records, per unit, how each rule is satisfied or why it does not apply. The **reviewer**
(`grace/workflow/2.7_review.md`) checks the diff against the same list. "Checked, does not apply" is an
acceptable answer for any theme; silence is not.

## How to refresh

Re-distil when the sample has drifted — roughly every 20 merged PRs, or after a review cycle raises a
theme this file does not carry.

```bash
gh pr list --repo juspay/hyperswitch-prism --state merged --limit 20 \
  --json number --jq '.[].number' > /tmp/prs.txt
while read -r n; do
  gh api "repos/juspay/hyperswitch-prism/pulls/$n/comments"      --paginate
  gh api "repos/juspay/hyperswitch-prism/issues/$n/comments"     --paginate
done < /tmp/prs.txt > /tmp/comments.json
```

Drop bot accounts (`github-actions[bot]`, `hyperswitch-bot[bot]`) and the PR author's own replies, then
group what remains by the requirement each comment states.

Rules for editing this file:

- Add a theme only when it recurs across **≥2 PRs**. A one-off belongs in `feedback.md`.
- **Never renumber or reuse a `TH-NN`.** A theme that stops applying is struck through with a one-line
  reason and keeps its id; the planner and reviewer cite these ids in stored artifacts.
- Keep every entry connector-agnostic: no connector names, no run ids, no merchant or site identifiers, no
  credentials, and no quoted text naming an individual. Reviewers are referred to by role.
- Update the date and PR range in the header whenever the sample changes.

## Index

| Id | Theme |
|---|---|
| TH-01 | PII, credentials and reusable tokens are `Secret<>` |
| TH-02 | Amounts use a typed unit and one converter |
| TH-03 | Currency is the `Currency` enum |
| TH-04 | Closed-set wire values are enums |
| TH-05 | Common utils over connector-local helpers |
| TH-06 | Errors carry the flow's own `attempt_status` |
| TH-07 | An inconclusive sync is never terminal |
| TH-08 | Errors carry context, never `default()` |
| TH-09 | No hardcoded value that belongs to request or config |
| TH-10 | Required inputs fail closed |
| TH-11 | Test evidence is real, current and self-contained |
| TH-12 | Proto field and enum numbers do not collide |
| TH-13 | PR scope hygiene and honest generated docs |
| TH-14 | Non-obvious logic gets a comment and a pinning test |
| TH-15 | Capture intent is honoured on every path |
| TH-16 | One reference id per payment, across all flows |
| TH-17 | Webhook trust boundary |
| TH-18 | Layer ownership |
| TH-19 | A mapping cites the document it came from |
| TH-20 | Collection selection |
| TH-21 | Registration completeness |

---

### TH-01 — PII, credentials and reusable tokens are `Secret<>`

**Rule.** Every field carrying personal data, a credential or a reusable payment token is typed
`Secret<String>` — `Secret<String, pii::EmailStrategy>` where it carries an email — on **every** struct
that holds it, request and response alike, so `masked_serialize` hides it from connector request logs and
events. A value that is `Secret` on one flow's struct and a plain `String` on another's is the defect: the
masking is only as strong as the weakest struct.

**Seen as.** A reviewer asking to "make PII fields secret"; an email placed into a general-purpose
identifier field as a plain `String`; a stored-credential token declared `Secret` on the authorize request
but plain on the repeat-payment request and on the response that returns it.

**Check.** In the diff, list every new or edited struct field whose name suggests email, name, account or
card number, token, or a reusable `*_id`, and confirm the type is `Secret<...>`. Then grep the whole
connector for that field name across all its request and response structs — inconsistency across flows is
the usual shape.

**PRs.** #2183, #2212, #2223, #2242 — representative:
<https://github.com/juspay/hyperswitch-prism/pull/2212#discussion_r3924196466>

---

### TH-02 — Amounts use a typed unit and one converter

**Rule.** Every amount is a typed unit from `crates/common/common_utils/src/types.rs` — `MinorUnit`,
`StringMinorUnit`, `StringMajorUnit`, `FloatMajorUnit` or `StringTwoDecimalUnit` — converted with the
connector's single amount converter, on request and response paths alike. Never a bare `String`, never an
`f64` parse of a converted string, and never a second converter whose name implies a narrower scope than
its use. Decisions taken *from* an amount (zero-amount detection, transaction-type selection) are taken on
the `MinorUnit` before conversion, not on the formatted string.

**Seen as.** A reviewer asking whether a typed major unit can be used for an amount field; a zero-amount
check implemented by parsing the converted major-unit string as `f64`; response paths hardcoding a
converter constant while request paths route through a differently named one.

**Check.**

```bash
git diff {BASE} -- <files> | grep -nE '^\+\s*(pub\s+)?[a-z_]*amount[a-z_]*\s*:\s*(Option<)?(String|f64)\b'   # gate: blocking
git diff {BASE} -- <files> | grep -nE '^\+.*(parse::<f64>|as f64).*amount'   # gate: blocking
```

Then confirm the connector names exactly one amount converter and both directions use it.

**PRs.** #2187, #2212, #2221 — representative:
<https://github.com/juspay/hyperswitch-prism/pull/2221#discussion_r3924336341>

---

### TH-03 — Currency is the `Currency` enum

**Rule.** Currency fields deserialize as `common_enums::Currency`, not `String` — in webhook and
notification payload structs as much as in flow requests and responses. The one admissible exception is a
raw string kept *solely* as checksum or signature input; where the value is used for anything else, a typed
field sits beside it.

**Seen as.** A reviewer asking, on a plain `String` field, "can we use currency enum here?"; a notification
struct declaring currency as `String` and the connector file parsing it by hand with
`to_uppercase().parse()` at the point of use.

**Check.** Grep the diff for `currency` field declarations typed `String`; for each, find the read site and
ask whether it is a checksum input only.

**PRs.** #2212, #2221, #2223 — representative:
<https://github.com/juspay/hyperswitch-prism/pull/2223#discussion_r3925090200>

---

### TH-04 — Closed-set wire values are enums

**Rule.** Any wire value drawn from a fixed set the API documents — transaction or document types, stage
and step markers, window-size and preference codes, country — is a Rust enum with serde renames carrying
the wire spelling, not a `String` compared against string constants. Country is `common_enums::CountryAlpha2`.

**Seen as.** A reviewer asking "can we make this an enum?"; an `Option<String>` marker field compared
against two module constants at its two read sites; numeric-code strings such as `"01"` / `"05"` fed from
constants into a `String` field; a country built from a typed country value and then stored as `String`.

**Check.** In the diff, for each new `String` or `Option<String>` struct field, find the values ever
assigned to it. A closed set of two or three literals, or a comparison against a `const`, means it should
be an enum.

**PRs.** #2183, #2212, #2221 — representative:
<https://github.com/juspay/hyperswitch-prism/pull/2212#discussion_r3924160597>

---

### TH-05 — Common utils over connector-local helpers

**Rule.** Logic that a second connector would need — signature and checksum helpers, id derivation, date
formatting, field extraction from common domain types — uses an existing common util, or creates one, rather
than a private helper in the connector module.

**Seen as.** A reviewer writing "please create/use a common util for this" on a connector-local function
duplicating behaviour already available.

**Check.** For each new free function or private helper in the connector module, grep the shared crates
(`crates/common/`, `crates/types-traits/domain_types/`) for the same shape by name and by signature before
accepting it.

**PRs.** #2187, #2206, #2212, #2221 — representative:
<https://github.com/juspay/hyperswitch-prism/pull/2206#discussion_r3957184314>

---

### TH-06 — Errors carry the flow's own `attempt_status`

**Rule.** An error response sets the `attempt_status` belonging to the flow that produced it. Refund flows
report a refund status, void flows a void status, authentication and token flows generally report none. A
single hardcoded payment-failure status applied to every non-2xx on every flow is wrong. Absent error codes
and messages use `consts::NO_ERROR_CODE` / `consts::NO_ERROR_MESSAGE`, never `unwrap_or_default()` and never
a literal such as `"Unknown error"`.

**Seen as.** A reviewer tracing that every non-2xx on every flow received the same payment-failure status,
and that the sync flow read it straight through — so a clock-skew 400, a rate limit and a genuine decline
all became a failed payment.

**Check.**

```bash
git diff {BASE} -- <files> | grep -nE '^\+.*attempt_status'          # one value repeated across flows?   # gate: advisory
git diff {BASE} -- <files> | grep -nE '^\+.*(unwrap_or_default\(\)|"Unknown error")'   # gate: advisory
```

For every `build_error_response`-shaped call site, name the flow it serves and check the status matches.

**PRs.** #2183, #2221, #2223 — representative:
<https://github.com/juspay/hyperswitch-prism/pull/2221#discussion_r3931621710>

---

### TH-07 — An inconclusive sync is never terminal

**Rule.** A sync or lookup call that fails at the request level — bad signature, expired timestamp, rate
limit, any transport or envelope error — or that returns no transaction status, must not stamp the resource
terminally failed. It leaves the existing status in place (or `Pending`) and passes `attempt_status: None`.
Terminal connector states must not map to `Pending` either, and an unrecognised state maps to a
non-terminal or unspecified arm. Consistency across flows is part of the rule: the payment sync and the
refund sync must not read the same envelope condition two different ways. TH-19 is the companion rule: this
theme decides whether an arm may be terminal, TH-19 whether its mapping can be cited.

**Seen as.** A reviewer pointing out that an in-band error *about the payment* terminally failed a
**refund**, while the payment sync mapped the identical envelope condition to `Pending` — and asking which
was right. A refund the connector had accepted then reported as failed on a poll, inviting a second refund.

**Check.** For each sync flow, find the arm that handles a request-level error and the arm that handles an
absent status. Neither may produce a terminal failure. Then diff the payment sync's and refund sync's
handling of the same envelope shape; a divergence needs an explicit reason.

**PRs.** #2183, #2187, #2207, #2221, #2227 — representative:
<https://github.com/juspay/hyperswitch-prism/pull/2221#discussion_r3931800193>

---

### TH-08 — Errors carry context, never `default()`

**Rule.** No bare `IntegrationErrorContext::default()`. Every constructed error sets `suggested_action`,
`doc_url` and/or `additional_context`, so distinct causes stay distinguishable instead of collapsing into
one opaque error class at the gRPC boundary.

**Seen as.** A reviewer showing that three different causes all surfaced as the same generic
invalid-data-format error to the caller, and asking that the reason be passed through the context field.

**Check.**

```bash
git diff {BASE} -- <files> | grep -nE '^\+.*IntegrationErrorContext::default\(\)|^\+.*context: Default::default\(\)'   # gate: blocking
```

A connector that has a local context-builder helper should use it at every error site, not most of them.

**PRs.** #2183, #2187, #2207 — representative:
<https://github.com/juspay/hyperswitch-prism/pull/2187#discussion_r3916625203>

---

### TH-09 — No hardcoded value that belongs to request or config

**Rule.** A value that varies by merchant, environment or request comes from the request or from config.
Constants must not silently change live behaviour — flags that disable a processor-side check, guessed
hosts, freshly generated identifiers where the caller supplied one. Idempotency and client-reference ids
derive from `connector_request_reference_id`.

**Seen as.** A reviewer finding a constant flag that disabled a duplicate-transaction check for *every*
live merchant when it had been intended for certification only, and asking whether it should be request- or
config-driven.

**Check.** For each new literal in a request body, ask what varies it. Grep the diff for freshly generated
uuids or timestamps used as identifiers, and for hostnames outside the `config/*.toml` base URLs.

**PRs.** #2125, #2183, #2187, #2207 — representative:
<https://github.com/juspay/hyperswitch-prism/pull/2207#discussion_r3965395875>

---

### TH-10 — Required inputs fail closed

**Rule.** A field the connector genuinely requires is required in code: read it from its own source first,
and refuse with a named missing-field error when it is absent. No silent fallback that changes what is sent
— substituting a different name for a cardholder name, omitting a redirect URL block when the return URL is
missing, or turning a missing identifier into an empty string with `unwrap_or_default()`.

**Seen as.** A reviewer writing that the cardholder name should be a required field and should not fall
back to the billing name; a redirect URL block silently omitted on payment methods that always redirect; a
successful response whose missing order identifier became an empty string downstream.

**Check.**

```bash
git diff {BASE} -- <files> | grep -nE '^\+.*\.unwrap_or(_default\(\)|\(""\)|_else\(String::new\))'   # gate: advisory
git diff {BASE} -- <files> | grep -nE '^\+.*\.or_else\(|^\+.*\.unwrap_or\('   # gate: advisory
```

For each hit on a field that goes on the wire or identifies a resource, decide: required and refused, or
genuinely optional. A fallback to a *different* source is the shape reviewers reject.

**PRs.** #2124, #2206, #2212, #2221, #2223 — representative:
<https://github.com/juspay/hyperswitch-prism/pull/2223#discussion_r3925106942>

---

### TH-11 — Test evidence is real, current and self-contained

**Rule.** Every justification string in the committed test specs — unsupported-scenario reasons above all —
and every reference in a code comment must resolve inside the committed repository, at the time a reviewer
reads it. No references to run artifacts that will not be committed, no internal plan item ids, no line
numbers pointing at code that has moved or been deleted, no counts of tests that no longer exist. CI renders
these strings verbatim onto the PR as the justification for merging.

**Seen as.** A reviewer noting that a cited test file had been deleted in an earlier commit, so the tests
the justification claimed no longer existed — and that CI was printing the claim onto the PR anyway.

**Check.** For every justification or comment reference in the diff, resolve it: open the path, check the
line, confirm the artifact is tracked by git. Replace a run-local reference with a self-contained reason
(a documented sandbox error code and what it means) and prefer a stable symbol name to a line number.

**PRs.** #2124, #2183, #2187, #2206, #2212 — representative:
<https://github.com/juspay/hyperswitch-prism/pull/2206#discussion_r3913499174>

---

### TH-12 — Proto field and enum numbers do not collide

**Rule.** A new proto field number or enum value must be unique not only against `main` but against every
open pull request that touches the same message or enum. Check the *number*, not just the name.

**Seen as.** A blocking comment reporting that a new enum value took a number already claimed by a
different value in a parallel open PR.

**Check.** For each added number, search open PRs touching the same proto file:

```bash
gh pr list --repo juspay/hyperswitch-prism --state open --search 'payment.proto' --json number --jq '.[].number'
gh pr diff <n> -- crates/types-traits/grpc-api-types/proto | grep -nE '=\s*<number>\s*;'
```

Record the check and its result; "no collision found, checked against PRs …" is itself the evidence.

**PRs.** #2221, #2223 — representative:
<https://github.com/juspay/hyperswitch-prism/pull/2221#issuecomment-5528624402>

---

### TH-13 — PR scope hygiene and honest generated docs

**Rule.** Changes to shared code — protos, the test harness, macros, server or framework crates — either go
in their own commits (protos under the `proto` scope) or have their blast radius stated explicitly in the PR
body, so they are reviewed independently of the connector work. Regenerated output must match actual
behaviour, and must not include files for unrelated connectors.

**Seen as.** A reviewer accepting a fix but noting it dragged along regenerated docs and examples for three
unrelated connectors; separately, a generated support table rendering payment methods as partially supported
when the flow returned "not implemented" for every one of them.

**Check.**

```bash
git diff --name-only {BASE} | grep -vE '^(crates/integrations/connector-integration/src/connectors/<c>|crates/internal/integration-tests/src/connector_specs/<c>)'   # gate: blocking
```

Everything that survives is either deliberately in scope and called out in the PR body, or belongs in
another commit. For regenerated docs, spot-check two rows against the code that decides them.

**PRs.** #2183, #2187, #2207 — representative:
<https://github.com/juspay/hyperswitch-prism/pull/2187#discussion_r3911909979>

---

### TH-14 — Non-obvious logic gets a comment and a pinning test

**Rule.** Novel, order-sensitive logic — a checksum or signature preimage, a field concatenation order, a
derived identifier — carries a comment saying why it is built that way, and a unit test that pins the output
for a fixed input.

**Seen as.** A reviewer observing that the only novel logic in a PR had nothing testing it, and that a small
case pinning the preimage for a fixed body would lock it in.

**Check.** Identify the one or two genuinely novel functions in the diff. Each needs both a comment and a
test. Note that connector runs are barred from authoring Rust test code, so in that context this becomes a
recorded follow-up rather than an in-run fix — record it either way.

**PRs.** #2207, #2221, #2223 — representative:
<https://github.com/juspay/hyperswitch-prism/pull/2221#discussion_r3931608151>

---

### TH-15 — Capture intent is honoured on every path

**Rule.** The requested capture method reaches the connector on *every* path that can charge, including
hosted-page and redirect paths, and partial-capture requests are either supported or guarded. A merchant on
manual capture must never be auto-captured because one code path omitted the flag.

**Seen as.** A reviewer finding that a hosted-page request body carried no capture directive, so a manual-
capture merchant routed through it got an auto-captured sale and a subsequent capture the processor rejected.

**Check.** Enumerate every request-builder that can result in a charge. For each, confirm the capture
method is read and mapped. Then confirm a guard exists for any capture method the connector cannot honour.

**PRs.** #2124, #2183, #2187, #2206, #2221 — representative:
<https://github.com/juspay/hyperswitch-prism/pull/2206#discussion_r3943031753>

---

### TH-16 — One reference id per payment, across all flows

**Rule.** The connector transaction reference returned for a payment is the same identifier on every flow.
Authorize, sync, capture and void must not each surface a different id form for the same payment.

**Seen as.** A reviewer showing that the sync flow returned one identifier while authorize returned another,
so the same payment reported two different reference ids depending on which call the caller made.

**Check.** For each flow's response mapping, note which response field becomes the resource id. They must
agree, or the difference must be documented as the connector's own semantics.

**PRs.** #2206, #2221 — representative:
<https://github.com/juspay/hyperswitch-prism/pull/2221#discussion_r3931608159>

---

### TH-17 — Webhook trust boundary

**Rule.** The event type, the payment or refund status, and the record a webhook resolves to are derived
**only** from fields inside the signed preimage. A field that the signature does not cover is untrusted
input: it may not classify the event, set a status or select the record. Source verification fails closed —
it never returns a success-shaped or "unsupported" default, and never substitutes the outbound request
credential for the webhook secret when no webhook secret is configured. Signature comparison uses the
platform's `crypto::HmacSha256` / `HmacSha1` / `HmacSha512` `verify_signature`, which compares in constant
time, rather than a hand-rolled byte equality.

**Seen as.** A reviewer showing that the status field sat *outside* the signed preimage, so a genuine
declined notification replayed with the status flipped to approved still verified and marked the payment
charged — and asking that the status be derived from the signed field instead. A reviewer on a
source-verification impl that reported "unsupported" as a no-op, noting that defaulting verification to
false means the webhook cannot be consumed at all and the flow silently falls back to sync. Verification
keyed on the outbound API secret because the test harness delivered no webhook secret. Event and
refund-versus-payment classification read from an attacker-controllable body field *before* verification, so
a forged notification could flip a payment success into a refund success. Verification assuming the
configured secret is a key-set document. A hand-rolled non-constant-time HMAC compare.

**Check.**

```bash
# a source-verification impl that returns a default or success-shaped value
git diff {BASE} -- <files> | grep -nE '^\+.*(fn verify_webhook_source|SourceVerified::(Unsupported|default)|Ok\(true\))'   # gate: blocking
# a hand-rolled comparison instead of crypto::*::verify_signature
git diff {BASE} -- <files> | grep -nE '^\+.*(eq_ignore_ascii_case|constant_time|ConstantTimeEq|\.as_bytes\(\) *==)'   # gate: blocking
# the preimage itself, to enumerate the fields the signature actually covers
git diff {BASE} -- <files> | grep -nE '^\+.*(preimage|to_sign|string_to_sign|signing_string)'   # gate: advisory
```

Then cross-check by hand: list every field the event-type mapper, the status mapper and the reference
resolver read, and confirm each one appears in the preimage construction. A field read by a mapper and
absent from the preimage is the defect. Also confirm no classification happens before verification.

**PRs.** #2330, #2340, #2341, #2356, #2364 — representative:
<https://github.com/juspay/hyperswitch-prism/pull/2356#discussion_r4111659599>

---

### TH-18 — Layer ownership

**Rule.** A connector integration adds no connector-specific branch to shared orchestration. On the UCS side
that means the shared crates (`crates/types-traits/domain_types`, `crates/common/`); on the Hyperswitch side
it means `crates/router` core flow code. A per-connector condition in shared code is refactored into a trait
method or a named predicate the connector implements, and a conversion or default added to a shared type
module becomes a reusable function rather than an inline arm. Decide the layer before writing the branch: a
shared-code condition is also a reviewer's cue to ask whether the logic belongs in the other repository
entirely.

**Seen as.** A reviewer asking, on connector transformer logic, whether it should live in the orchestrator
rather than in the connector service; the same reviewer asking that a capture validation not be added at
that layer at all. A silent unspecified-to-`None` enum conversion added inside a shared types module, where
the reviewer asked for a reusable function or trait covering every call site instead. On the Hyperswitch
side, a maintainer's blocking comment on a core authorize flow against a literal `is_<connector> && …`
condition, asking that a named predicate be introduced inside the existing gate function instead — a
CHANGES_REQUESTED that is still blocking that PR.

**Check.**

```bash
# does the diff touch shared crates at all?
git diff --name-only {BASE} | grep -E '^crates/(types-traits/domain_types|common)/'   # gate: blocking
# within anything it touches, a branch keyed on one connector
git diff {BASE} -- <files> | grep -nE '^\+.*(ConnectorEnum::[A-Z]|eq_ignore_ascii_case\("|is_[a-z0-9_]+ *&&)'   # gate: blocking
```

Every hit needs either a trait method / predicate the connector implements, or a stated reason why the
branch cannot be expressed that way. Run the second grep over the paired Hyperswitch diff as well.

**PRs.** #2330, #2340, and Hyperswitch PR juspay/hyperswitch#14316 — representative:
<https://github.com/juspay/hyperswitch-prism/pull/2330#discussion_r4102182541>

---

### TH-19 — A mapping cites the document it came from

**Rule.** Every status arm, every event-type arm and every verification scheme carries the
connector-documentation reference it was derived from. An arm nobody can cite maps to a **non-terminal**
status — never to a terminal success, and never to a terminal state the connector has no way to move off.
This is the provenance half of TH-07: TH-07 governs whether an arm may be terminal at all, TH-19 governs
whether anyone can show where the arm came from. Reviewers detect invented mappings by asking for the doc,
so an arm without a citation is an arm that will be questioned.

**Seen as.** A reviewer asking "can we check the doc for these two event?" on a webhook event mapping, and
separately asking that the doc be checked to confirm how source verification is expected to be performed. A
final catch-all arm mapping any unrecognised or missing status to authentication-successful, where the
reviewer asked that success be mapped explicitly and everything else fail. An unknown webhook status mapped
to a refund pending state the connector offers no way to resolve.

**Check.**

```bash
# every catch-all arm and trailing else in the diff
git diff {BASE} -- <files> | grep -nE '^\+.*(_ =>|\} else \{)'   # gate: advisory
# the doc references the diff does carry
git diff {BASE} -- <files> | grep -nE '^\+ *//.*(https?://|doc:|spec:)'   # gate: advisory
```

For each arm the first grep finds, name the documented state it came from. If none can be named, it must not
resolve terminally. A diff whose second grep returns nothing near its status maps has no provenance at all.

**PRs.** #2340, #2356 — representative:
<https://github.com/juspay/hyperswitch-prism/pull/2340#discussion_r4119093019>

---

### TH-20 — Collection selection

**Rule.** Never take the first element of a connector's array of refunds, settlements, authorizations,
captures or disputes. Select the element by the identifier being reconciled; when no element matches, that is
an error, not a fallback to element zero. Reconciling one record must not overwrite another's identifier
either — a refund notification keeps the original payment's transaction id distinct from the refund id.

**Seen as.** A refund webhook always reading the first entry of the charge's refunds array, so a second
partial refund credits the wrong record. A webhook mapper always taking the first authorization and the first
settlement, so multi-authorization and multi-settlement events return the wrong reference, amount or status.
A refund notification writing the refund id into the payment's transaction-id field, losing the original
payment's identifier.

**Check.**

```bash
git diff {BASE} -- <files> | grep -nE '^\+.*(\[0\]|\.get\(0\)|\.first\(\)|\.into_iter\(\)\.next\(\))'   # gate: blocking
```

Note that the selection and the collection are often on separate lines, so the grep is deliberately keyed on
the selection alone: for every hit, name the collection it reads and the identifier that should have chosen
the element. Then confirm each flow's reference mapping keeps the payment id and the refund id in their own
fields.

**PRs.** #2340, #2341, #2356 — representative:
<https://github.com/juspay/hyperswitch-prism/pull/2356#discussion_r4111659628>

---

### TH-21 — Registration completeness

**Rule.** A flow declared in code is reachable through dispatch, and a config key added to one environment
file is added to every environment file that needs it. A declaration without its registration is a flow that
compiles and cannot be called. Overriding a per-flow predicate is not registration either: while the
connector stays listed in a blanket default-impl macro, the generated empty impls win and the leg silently
no-ops.

**Seen as.** A blocking comment reporting a connector fully implemented but absent from the
supported-connectors arm used by dispatch, so calls fall through to an invalid-connector error "even though
the connector code exists". On the Hyperswitch side, a connector added to a supported-connectors config list
in the integration-test, sandbox and development files but missed in the example and production files; and a
connector whose 3DS leg predicates were overridden while it remained listed in the blanket
authenticate-steps default-impl macro, so both legs resolved to the generated empty impls and 3DS
transactions appeared to succeed while skipping authentication entirely.

**Check.**

```bash
# every flow marker / trait impl the diff declares
git diff {BASE} -- <files> | grep -nE '^\+.*(V2(<[A-Za-z]+>)? for|IncomingWebhook for|connector_flow::)'   # gate: advisory
# the connector's own arm in the dispatch match
grep -nE 'ConnectorEnum::<Connector> *=>' crates/types-traits/domain_types/src/types.rs
# config keys: the environment files the diff touched, against the full set
git diff --name-only {BASE} | grep -E '^config(/deployments)?/.*\.toml$'   # gate: advisory
ls config/*.toml
```

Every marker from the first grep needs a dispatch arm from the second. For a new config key, grep a
neighbouring key in the same table across `config/*.toml` and require the same file set. On the Hyperswitch
side, also grep the blanket default-impl macros for the connector's name whenever the diff overrides one of
their methods — being listed there and overriding are mutually exclusive.

**PRs.** #2341, and Hyperswitch PRs juspay/hyperswitch#14347 and juspay/hyperswitch#14299 —
representative: <https://github.com/juspay/hyperswitch-prism/pull/2341#issuecomment-5771011440>
