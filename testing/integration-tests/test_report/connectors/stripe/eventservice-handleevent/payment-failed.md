# Connector `stripe` / Suite `EventService/HandleEvent` / Scenario `Handle Event | Payment Failed`

- Service: `EventService/HandleEvent`
- Scenario Key: `payment_failed`
- PM / PMT: `-` / `-`
- Result: `FAIL`

**Error**

```text
Resolved method descriptor:
// Process webhook notifications from connectors. Translates connector events
// into standardized responses for asynchronous payment state updates.
rpc HandleEvent ( .types.EventServiceHandleRequest ) returns ( .types.EventServiceHandleResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: EventService/HandleEvent_payment_failed_ref
x-merchant-id: test_merchant
x-request-id: EventService/HandleEvent_payment_failed_req
x-tenant-id: default

Response headers received:
(empty)

Response trailers received:
content-type: application/grpc
date: Tue, 14 Apr 2026 08:44:46 GMT
x-request-id: EventService/HandleEvent_payment_failed_req
Sent 1 request and received 0 responses

ERROR:
  Code: Unimplemented
  Message: Webhooks not implemented for this connector (get_event_type)
```

**Pre Requisites Executed**

- None
<details>
<summary>Show Request (masked)</summary>

```bash
grpcurl -plaintext \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: EventService/HandleEvent_payment_failed_req" \
  -H "x-connector-request-reference-id: EventService/HandleEvent_payment_failed_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:8000 types.EventService/HandleEvent <<'JSON'
{
  "request_details": {
    "method": "HTTP_METHOD_POST",
    "headers": {},
    "body": "eyJpZCI6ImV2dF8xTXFMbkpIZlhiUTh3V1ZVNkpadk4zQ2UiLCJvYmplY3QiOiJldmVudCIsImFwaV92ZXJzaW9uIjoiMjAyMi0xMS0xNSIsImNyZWF0ZWQiOjE2ODA2MTQ0MDAsImRhdGEiOnsib2JqZWN0Ijp7ImlkIjoicGlfM01xTG5KSGZYYlE4d1dWVTBLOG01VjZxIiwib2JqZWN0IjoicGF5bWVudF9pbnRlbnQiLCJhbW91bnQiOjIwMDAsImFtb3VudF9jYXB0dXJhYmxlIjowLCJhbW91bnRfcmVjZWl2ZWQiOjAsImN1cnJlbmN5IjoidXNkIiwic3RhdHVzIjoiZmFpbGVkIiwibGFzdF9wYXltZW50X2Vycm9yIjp7ImNvZGUiOiJjYXJkX2RlY2xpbmVkIiwibWVzc2FnZSI6IllvdXIgY2FyZCB3YXMgZGVjbGluZWQuIn19fSwidHlwZSI6InBheW1lbnRfaW50ZW50LnBheW1lbnRfZmFpbGVkIn0="
  },
  "merchant_event_id": "stripe_webhook_payment_failed"
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
x-connector-request-reference-id: EventService/HandleEvent_payment_failed_ref
x-merchant-id: test_merchant
x-request-id: EventService/HandleEvent_payment_failed_req
x-tenant-id: default

Response headers received:
(empty)

Response trailers received:
content-type: application/grpc
date: Tue, 14 Apr 2026 08:44:46 GMT
x-request-id: EventService/HandleEvent_payment_failed_req
Sent 1 request and received 0 responses

ERROR:
  Code: Unimplemented
  Message: Webhooks not implemented for this connector (get_event_type)
```

</details>


[Back to Connector Suite](../eventservice-handleevent.md) | [Back to Overview](../../../test_overview.md)
