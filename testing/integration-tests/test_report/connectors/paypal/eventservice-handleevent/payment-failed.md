# Connector `paypal` / Suite `EventService/HandleEvent` / Scenario `Handle Event | Payment Failed`

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
date: Tue, 14 Apr 2026 08:45:40 GMT
x-request-id: EventService/HandleEvent_payment_failed_req
Sent 1 request and received 0 responses

ERROR:
  Code: InvalidArgument
  Message: Failed to decode webhook event body
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
    "body": ""
  },
  "merchant_event_id": ""
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
date: Tue, 14 Apr 2026 08:45:40 GMT
x-request-id: EventService/HandleEvent_payment_failed_req
Sent 1 request and received 0 responses

ERROR:
  Code: InvalidArgument
  Message: Failed to decode webhook event body
```

</details>


[Back to Connector Suite](../eventservice-handleevent.md) | [Back to Overview](../../../test_overview.md)
