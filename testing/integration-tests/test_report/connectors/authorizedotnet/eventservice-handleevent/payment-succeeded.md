# Connector `authorizedotnet` / Suite `EventService/HandleEvent` / Scenario `Handle Event | Payment Succeeded`

- Service: `EventService/HandleEvent`
- Scenario Key: `payment_succeeded`
- PM / PMT: `-` / `-`
- Result: `FAIL`

**Error**

```text
assertion failed for field 'source_verified': expected true, got missing
```

**Pre Requisites Executed**

- None
<details>
<summary>Show Request (masked)</summary>

```bash
grpcurl -plaintext \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: EventService/HandleEvent_payment_succeeded_req" \
  -H "x-connector-request-reference-id: EventService/HandleEvent_payment_succeeded_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:8000 types.EventService/HandleEvent <<'JSON'
{
  "request_details": {
    "method": "HTTP_METHOD_POST",
    "headers": {},
    "body": "eyJub3RpZmljYXRpb25JZCI6IjU1MGU4NDAwLWUyOWItNDFkNC1hNzE2LTQ0NjY1NTQ0MDAwMCIsImV2ZW50VHlwZSI6Im5ldC5hdXRob3JpemUucGF5bWVudC5hdXRoY2FwdHVyZS5jcmVhdGVkIiwiZXZlbnREYXRlIjoiMjAyMy0wNC0wNVQxMjowMDowMC4wMDAwMDAwWiIsIndlYmhvb2tJZCI6IjcyYTU1Yzc4LTY2ZTYtNGIyZS1hMGQ5LTJhM2YxY2Q0Yjg5MCIsInBheWxvYWQiOnsicmVzcG9uc2VDb2RlIjoxLCJhdXRoQ29kZSI6IkFCQ0RFRiIsImF2c1Jlc3BvbnNlIjoiWSIsImF1dGhBbW91bnQiOjIwLjAsImVudGl0eU5hbWUiOiJ0cmFuc2FjdGlvbiIsImlkIjoiNjAxMjM0NTY3ODkifX0="
  },
  "merchant_event_id": "anet_webhook_authcapture_001"
}
JSON
```

</details>

<details>
<summary>Show Response (masked)</summary>

```text
Resolved method descriptor:
// Process webhook notifications from connectors. Translates connector events
// into standardized responses for asynchronous payment state updates.
rpc HandleEvent ( .types.EventServiceHandleRequest ) returns ( .types.EventServiceHandleResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: EventService/HandleEvent_payment_succeeded_ref
x-merchant-id: test_merchant
x-request-id: EventService/HandleEvent_payment_succeeded_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Tue, 14 Apr 2026 08:45:40 GMT
x-request-id: EventService/HandleEvent_payment_succeeded_req

Response contents:
{
  "eventType": "PAYMENT_INTENT_SUCCESS",
  "eventContent": {
    "paymentsResponse": {
      "connectorTransactionId": "60123456789",
      "status": "CHARGED",
      "error": {
        "connectorDetails": {}
      },
      "statusCode": 200,
      "merchantTransactionId": "60123456789"
    }
  },
  "eventStatus": "EVENT_STATUS_COMPLETE"
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>


[Back to Connector Suite](../eventservice-handleevent.md) | [Back to Overview](../../../test_overview.md)
