# Services and Methods

Prism organizes payment operations into services that reflect how payments actually work in the real world. Some operations are independent. Others are follow-on actions that only make sense after a payment exists.

## What is a Service?

A **service** is a logical grouping of related payment operations. Services model distinct phases or aspects of the payment lifecycle:

- **PaymentService** — Core payment operations (authorize, capture, refund)
- **RefundService** — Refund-specific operations
- **DisputeService** — Chargeback and dispute handling
- **RecurringPaymentService** — Stored payment method operations

Think of a service as a namespace that keeps related operations together. Services can depend on other services—a refund requires a payment to exist first.

## What is a Method?

A **method** is a single operation within a service. Methods are actions you invoke:

- `authorize()` — Hold funds on a payment method
- `capture()` — Transfer held funds
- `refund()` — Return funds to the customer

Each method has:
- A **request type** — Input parameters specific to that operation
- A **response type** — Output data returned after execution
- **Error handling** — Structured errors if the operation fails

Methods follow verb-noun naming (authorize, capture, void) to clearly indicate the action performed.

## Flow

A sample flow of the most frequently used methods will look like below.

```
                         ┌─────────────────────────────────────────────────────┐
                         │                  ONE-TIME PAYMENT                   │
                         │                                                     │
                         │   authorize() ──────► capture() ──────► refund()    │
                         │        │                   │                 │      │
                         │        ▼                   ▼                 ▼      │
                         │     void()                get()            get()    │
                         └─────────────────────────────────────────────────────┘
                                                  │
                                                  │ triggers (optional)
                                                  ▼
                         ┌─────────────────────────────────────────────────────┐
                         │                     DISPUTE                         │
                         │                                                     │
                         │          accept()  /  defend() ──► submit_evidence()│
                         └─────────────────────────────────────────────────────┘

                         ┌─────────────────────────────────────────────────────┐
                         │                RECURRING PAYMENT                    │
                         │                                                     │
                         │   setup_recurring() ──► charge() ──► revoke()       │
                         │           │               │                         │
                         │           ▼               ▼                         │
                         │          get()          get()                       │
                         └─────────────────────────────────────────────────────┘
```
