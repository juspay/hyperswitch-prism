# Connector `stripe` / Suite `EventService/HandleEvent` / Scenario `Handle Event | Payment Succeeded`

- Service: `EventService/HandleEvent`
- Scenario Key: `payment_succeeded`
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
x-connector-request-reference-id: EventService/HandleEvent_payment_succeeded_ref
x-merchant-id: test_merchant
x-request-id: EventService/HandleEvent_payment_succeeded_req
x-tenant-id: default

Response headers received:
(empty)

Response trailers received:
content-type: application/grpc
date: Tue, 14 Apr 2026 08:44:46 GMT
x-request-id: EventService/HandleEvent_payment_succeeded_req
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
  -H "x-request-id: EventService/HandleEvent_payment_succeeded_req" \
  -H "x-connector-request-reference-id: EventService/HandleEvent_payment_succeeded_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:8000 types.EventService/HandleEvent <<'JSON'
{
  "request_details": {
    "method": "HTTP_METHOD_POST",
    "headers": {},
    "body": "eyJpZCI6ImV2dF8xTXFMbkpIZlhiUTh3V1ZVNkpadk4zQ2QiLCJvYmplY3QiOiJldmVudCIsImFwaV92ZXJzaW9uIjoiMjAyMi0xMS0xNSIsImNyZWF0ZWQiOjE2ODA2MTQzOTksImRhdGEiOnsib2JqZWN0Ijp7ImlkIjoicGlfM01xTG5KSGZYYlE4d1dWVTBLOG01VjZwIiwib2JqZWN0IjoicGF5bWVudF9pbnRlbnQiLCJhbW91bnQiOjIwMDAsImFtb3VudF9jYXB0dXJhYmxlIjowLCJhbW91bnRfcmVjZWl2ZWQiOjIwMDAsImN1cnJlbmN5IjoidXNkIiwic3RhdHVzIjoic3VjY2VlZGVkIn19LCJ0eXBlIjoicGF5bWVudF9pbnRlbnQuc3VjY2VlZGVkIn0="
  },
  "merchant_event_id": "stripe_webhook_payment_succeeded"
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
(empty)

Response trailers received:
content-type: application/grpc
date: Tue, 14 Apr 2026 08:44:46 GMT
x-request-id: EventService/HandleEvent_payment_succeeded_req
Sent 1 request and received 0 responses

ERROR:
  Code: Unimplemented
  Message: Webhooks not implemented for this connector (get_event_type)
```

</details>


[Back to Connector Suite](../eventservice-handleevent.md) | [Back to Overview](../../../test_overview.md)
