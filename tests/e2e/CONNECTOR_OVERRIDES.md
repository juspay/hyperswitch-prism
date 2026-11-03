# Connector override contract

For a compile-checked, non-runnable example containing every currently
supported connector-definition field, see [`connectors/example.ts`](connectors/example.ts).
It is deliberately absent from `connectors/index.ts`. Copy only justified
differences into a real connector rather than registering or duplicating the
whole example.

This document is the implementation contract for connector-specific differences.
The shared scenario remains the source of truth for the journey, API-call order,
base requests, and business assertions. A connector definition contains data
only; it must not contain callbacks, HTTP calls, polling, or assertions.

The framework implements this contract for every operation it currently exposes.
Add an operation key only when its shared operation method is implemented; do
not add speculative configuration for endpoints the suite does not call.

Every entry in `scenarios/catalog.ts` runs for every connector registered in
`connectors/index.ts`. A connector is non-runnable for a catalog scenario only
when it declares an exclusion with a non-empty reason. Do not repeat runnable
scenario IDs as empty overrides or maintain a connector-specific scenario list.

## Identify calls, not only operations

An operation name is not a unique request. A recurring journey can call
`createPayment` once to set up a mandate and again to make the recurring charge.
A scenario can likewise contain multiple captures or refunds. Applying one
`createPayment` override to every call would modify the wrong requests.

Give every overrideable call a stable ID that describes its role in the journey:

```ts
const setup = await payments.createPayment("mandate-setup", setupRequest);
const recurring = await payments.createPayment(
  "recurring-payment",
  recurringRequest,
);
```

Group call IDs under their operation in connector data:

```ts
overrides: {
  "connector-mandate-recurring": {
    requests: {
      createPayment: {
        "mandate-setup": {
          reason: "Example sandbox requires connector program metadata",
          patch: {
            connector_metadata: {
              program: "synthetic-sandbox-program",
            },
          },
        },
        "recurring-payment": {
          reason: "Example sandbox omits the shared shipping phone",
          patch: {
            metadata: {
              test_case: "synthetic-recurring-payment",
            },
          },
          omit: [["shipping", "phone"]],
        },
      },
    },
    expectations: {
      "recurring-payment-result": {
        reason: "Example sandbox returns a documented exact value",
        patch: {
          status: "succeeded",
        },
      },
    },
  },
},
```

The outer key is the stable scenario ID. Request IDs are stable within their
operation and scenario. Display titles and report labels are not override keys.
The catalog-derived `ScenarioId` type rejects misspelled or unimplemented
scenario IDs at compile time. Registry validation provides the same protection
when definitions are assembled dynamically.

## Request transformation rules

For the selected scenario, operation, and call ID, construct the final request in
this exact order:

1. Start with the shared scenario request.
2. Apply every `omit` path.
3. Recursively apply `patch`.

Merge semantics are deterministic:

- Objects merge recursively.
- Arrays replace the shared array completely.
- Strings, numbers, booleans, and `null` replace the shared value.
- `null` remains an explicit JSON value; it never means omission.
- `omit` removes a field completely.

Applying omission before the patch also permits an exact object replacement:

```ts
createPayment: {
  "initial-payment": {
    omit: [["connector_metadata"]],
    patch: {
      connector_metadata: {
        replacement_field: "synthetic-value",
      },
    },
  },
},
```

An `omit` path is an array of field names rather than a dotted string, so field
boundaries are unambiguous. The framework should fail when a configured omit path
does not exist; silently ignoring a typo can certify the wrong request.

## Expectation overrides

The shared spec assigns stable IDs to report-safe checks:

```ts
const expectedResult = expected("recurring-payment-result", {
  status: "succeeded",
  amount_received: 6000,
});
```

Connector configuration may recursively patch exact expected values for a check
that the shared scenario already declares. It must not:

- remove an assertion;
- select a different matcher;
- add executable assertion code;
- accept failure merely to make the test green;
- relax the UCS routing or correlated-event requirements; or
- report complete responses or sensitive fields.

If a field is not universally meaningful, express a shared invariant such as
presence, model a separate shared scenario, or exclude unsupported behavior with
a reason. Do not hide a materially different lifecycle in an expectation patch.

## Validation requirements

The framework must reject:

- an unknown scenario ID in an override or exclusion;
- a scenario that the same connector both overrides and excludes;
- a configured request override that no call consumes;
- a configured expectation override that no check consumes;
- the same mutation call ID being consumed more than once;
- an empty scenario exclusion reason; and
- executable values in connector definitions.

These checks belong in the typed runtime API, not in source-scanning or regular
expression scripts.

## Boundaries

- Scenarios own sequences, dynamic IDs, dependencies between responses, and base
  expectations.
- Operations own one HS API call and apply the override identified by the call ID.
- Connector definitions own only minimal sandbox request and exact-value
  expectation differences.
- Credentials and connector-account metadata stay in the external approved
  sandbox credentials file, not in overrides.
- Use synthetic values only. Never copy production identifiers, payload values,
  credentials, logs, or internal links into connector definitions or reports.

If a connector needs most of the request replaced, first verify whether it is
actually the same scenario. A large override is usually evidence that a distinct
shared scenario or payment-method journey is required.
