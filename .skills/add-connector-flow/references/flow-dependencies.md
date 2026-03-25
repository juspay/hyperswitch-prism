# Flow Dependencies Reference

## Dependency Graph

```
                 [Independent Flows]
                 CreateAccessToken, CreateOrder, CreateConnectorCustomer,
                 PaymentMethodToken, CreateSessionToken,
                 AcceptDispute, SubmitEvidence, DefendDispute

                 [Main Payment Flow Chain]

                      Authorize
                     /    |    \         \
                  PSync Capture Void   SetupMandate
                    |      |                 |
             IncomingWebhook Refund    RepeatPayment
                              |
                            RSync
```

## Dependency Map

| Flow | Prerequisites |
|------|---------------|
| Authorize | (none) |
| PSync | Authorize |
| Capture | Authorize |
| Void | Authorize |
| VoidPC | Authorize, Capture |
| Refund | Authorize, Capture |
| RSync | Refund |
| SetupMandate | Authorize |
| RepeatPayment | SetupMandate |
| IncomingWebhook | PSync |
| CreateAccessToken | (none) |
| CreateOrder | (none) |
| CreateConnectorCustomer | (none) |
| PaymentMethodToken | (none) |
| CreateSessionToken | (none) |
| AcceptDispute | (none) |
| SubmitEvidence | (none) |
| DefendDispute | (none) |

## Resolution Algorithm

Given `requested_flows` and `existing_flows`, determine implementation order:

```
1. VALIDATE: For each requested flow, check every prerequisite is either
   in existing_flows or in requested_flows. If not, report error and stop.

2. SORT (topological): Repeat until remaining is empty:
   a. Find all flows in remaining whose prerequisites are all satisfied
      (in existing_flows or already in the ordered output list).
   b. If none found, report circular dependency error.
   c. Move those flows from remaining to the ordered output list.

3. Return the ordered list.
```

## Example Resolutions

**Adding [RSync, Refund] when existing = [Authorize, PSync, Capture]:**
- Refund: needs Authorize (existing) + Capture (existing) -> OK
- RSync: needs Refund (in requested) -> OK
- Order: [Refund, RSync]

**Adding [Refund, Capture] when existing = [Authorize]:**
- Capture: needs Authorize (existing) -> OK
- Refund: needs Authorize (existing) + Capture (in requested) -> OK
- Order: [Capture, Refund]

**Adding [Refund] when existing = [Authorize, PSync] (ERROR):**
- Refund: needs Capture -> NOT in existing, NOT in requested -> ERROR
- Message: "Cannot add Refund: prerequisite Capture is not implemented."

## Detecting Existing Flows

Inspect the connector `.rs` file for three indicators. All three must be present
for a flow to be considered fully implemented:

1. **`create_all_prerequisites!` api array** -- look for `(flow: FlowName, ...)` entries.
2. **`macro_connector_implementation!` blocks** -- look for `flow_name: FlowName`.
3. **Trait implementations** -- look for `connector_types::PaymentCapture for Conn<T> {}`.

If a flow appears in the prerequisites macro but lacks a `macro_connector_implementation!`
block, treat it as not implemented.

## Handling Missing Prerequisites

1. **Inform the user** which prerequisite is missing and which flow requires it.
2. **Suggest** adding the prerequisite to the requested set, or implementing it first.
3. **Do not proceed** with the dependent flow until prerequisites are resolved.
4. **Independent flows can proceed.** If requesting [Void, Refund] and Capture is
   missing, Void can still be implemented (needs only Authorize). Only Refund is blocked.
