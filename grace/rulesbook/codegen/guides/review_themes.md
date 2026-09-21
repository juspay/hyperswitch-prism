# Recurring Reviewer Themes

This file is distilled from human reviewer comments on merged pull requests, not from a style guide. The
current edition covers the **20 merged PRs #2124–#2327** of `juspay/hyperswitch-prism`, collected
**2026-09-19**: 209 comments, of which 111 were substantive reviewer comments (bot output and author
replies excluded). Sixteen themes recurred across two or more PRs; each is a `TH-NN` below.

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
git diff {BASE} -- <files> | grep -nE '^\+\s*(pub\s+)?[a-z_]*amount[a-z_]*\s*:\s*(Option<)?(String|f64)\b'
git diff {BASE} -- <files> | grep -nE '^\+.*(parse::<f64>|as f64).*amount'
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
git diff {BASE} -- <files> | grep -nE '^\+.*attempt_status'          # one value repeated across flows?
git diff {BASE} -- <files> | grep -nE '^\+.*(unwrap_or_default\(\)|"Unknown error")'
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
refund sync must not read the same envelope condition two different ways.

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
git diff {BASE} -- <files> | grep -nE '^\+.*IntegrationErrorContext::default\(\)|^\+.*context: Default::default\(\)'
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
git diff {BASE} -- <files> | grep -nE '^\+.*\.unwrap_or(_default\(\)|\(""\)|_else\(String::new\))'
git diff {BASE} -- <files> | grep -nE '^\+.*\.or_else\(|^\+.*\.unwrap_or\('
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
git diff --name-only {BASE} | grep -vE '^(crates/integrations/connector-integration/src/connectors/<c>|crates/internal/integration-tests/src/connector_specs/<c>)'
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
