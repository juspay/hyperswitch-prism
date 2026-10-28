# Extending the E2E suite

Start with `scenarios/payments/create-confirm/card/one-time/no3ds/create-confirm.spec.ts`. It is the reference for new API scenarios. Use ordinary TypeScript variables and the exported Playwright fixtures; there is no step language or hidden payment sequence.

## One supported contribution pattern

1. Add one shared spec under resource/operation/payment-method and the relevant variation, following README.md. Place it by the operation being verified and include its prerequisites in the spec. Give it a stable, unique scenario ID, a readable title, a journey `description`, a `verifies` description of the expected outcome, and `paymentMethodsEnabled` describing the methods its merchant connector account needs.
2. Import `test` from `framework/fixtures.ts` and the connector registry from `connectors/index.ts`. Use the same `for`/`describe`/`test.use` registration pattern as the reference spec. The loop includes every registered connector.
3. Write the calls in execution order using the `payments` fixture. Group each call and its checks in `report.step(...)`; it attaches audit evidence afterward, including on failure. Keep request fields, use of returned values, and expected outcomes visible in the spec.
4. Import native `expect` from `@playwright/test`. Use `report.check(name, { actual, expected }, ({ actual, expected }) => expect(actual).toMatchObject(expected))` for reported assertions, choosing the native matcher appropriate to the check. Select only report-safe values; never pass the complete payment response or card/authentication data. The helper attaches the same comparison on pass or failure and does not choose a matcher.
5. For an asynchronous scenario, use native Playwright `expect.poll` with an explicit timeout and read-only retrieval inside the spec. `retrievePayment` makes one GET. Do not retry mutations. Keep the final selected values in the reported comparison; the current immediate-success scenario does not poll.
6. Run `npm run check`, `npm run list`, and the new spec against the sandbox. An empty placeholder does not count as coverage.

Use a new variable for each response and pass fields directly to the next operation. Responses are not automatically stored in shared state. Each test receives an isolated merchant and connector account. Keep request amounts, addresses, and expected outcomes fixed; generated resource identifiers are created only for isolation.

## What changes for each contribution

| Contribution | Expected change |
| --- | --- |
| Another journey using existing operations | One shared spec. No fixture or operation changes. |
| Another connector | One data definition, one registry entry, and external credentials. Restart the stack to apply the registry's UCS-only routing. Existing specs inherit the connector. |
| A processor-required request difference | Data in that connector's override for the shared scenario ID. |
| Unsupported behavior | An exclusion on the connector with a reason; the shared test remains registered and reports skipped. |
| A genuinely new HS endpoint | Add one small method to its resource's `*-operations.ts` module (`payment-operations.ts` for `/payments`). It forwards API fields, validates the response shape, and returns the full response. It does not call other mutating operations. |
| A new resource family | Add its resource module and fixture, reusing the HTTP client and assertions: for example, `/refunds` uses `refund-operations.ts` and the `refunds` fixture. Keep its journey in the spec. |

New endpoints necessarily need an operation implementation; adding an ordinary scenario must not require editing a dispatcher, global scenario enumeration, or connector execution code. Do not add an unimplemented operation in anticipation of future coverage.

## Connector differences

Use the factories in `test-data.ts` for reusable address, contact, customer, browser and card values. Call them inside each test to obtain fresh objects and customer IDs. Keep the payment request assembly, amount, currency, capture mode, order items, metadata and expected results visible in the spec. `createCard(connector.card)` selects the shared sandbox card plus optional connector-specific field overrides. Do not create a factory that silently builds an entire payment request.

Connector definitions contain data only. Request overrides are applied by the corresponding operation, so specs do not implement their own merging. They replace complete top-level fields; no implicit recursive merge occurs. Reports annotate when an override is configured, without exposing the request body.

Shared assertions are the scenario's contract. Connector configuration cannot replace expected outcomes or relax routing checks. If behavior differs materially, model a distinct shared scenario or record an unsupported scenario with a reason. Do not accept a failure status merely to obtain a green test.

A scenario ID identifies the behavior independently of its display title. Use that exact ID in connector configuration. Review ID/override references when moving or renaming a scenario; scenario IDs are not a second registration list.

## Boundaries

- Specs own call order and expected business outcomes. They use public fixtures and shared operations, not raw HTTP clients, API keys, SQL, or custom reporting.
- Fixtures own setup and cleanup only. The public fixture type excludes raw HTTP, server keys, override internals, and generic polling callbacks.
- Operations own individual API calls. HTTP retries and redirects are disabled. The HTTP client validates that responses are JSON objects; endpoint helpers validate fields needed for their returned types.
- Native Playwright assertions in specs own comparisons. `report.check` only groups the assertion and attaches selected expected/actual values. Database helpers return facts; the spec asserts what those facts must be.
- The audit collector owns masked event correlation and attachments. It receives HTTP response identifiers through the client's response hook and buffers evidence until the report step finishes. The report helper only controls grouping and attachment order; it has no payment journey. Scenarios do not read Docker logs or implement masking. Audit collection is diagnostic; gateway checks remain required, and missing events cannot prove flow execution.
- Request bodies follow HS's API. The suite accepts additional fields instead of mirroring or inventing a complete request schema.

Contributors can deliberately bypass a TypeScript API by importing unrelated modules. Types, the reference spec, and review define the supported approach; source-scanning scripts are not used to police arbitrary code.

## Review before and after pushing

Read the final diff as a contributor seeing the framework for the first time:

- Can the complete API sequence, request data, response dependencies, and expectations be understood from the spec?
- Could the same behavior be written through a second public helper or hidden fixture sequence? Remove redundant routes; document genuinely different operations such as one GET versus bounded polling.
- Do all connectors inherit the scenario? Are unsupported cases visible and explained?
- Does each file stay within the boundaries above? Are ordinary scenario additions independent of framework internals?
- Can a mutation be retried, a failed result be treated as success, or a native HS connector path pass as UCS?
- Are response types supported by runtime validation, rather than unchecked casts? Are missing fields visible in failure reports?
- Are fixtures and identifiers isolated per test? Are request data and expected results stable?
- Does test discovery point to the actual spec, and does the report identify the failed operation and comparison?
- Does the final public diff contain only generic documentation and sandbox data, with no credentials or production values?

After a push, inspect the published diff and confirm it matches the validated commit. If it reveals another gap, fix it and repeat the relevant verification. Report remaining dependencies, including sandbox/network availability and HS database schema compatibility, without claiming universal determinism.
