# Connector `adyen` / Suite `EventService/HandleEvent` / Scenario `Handle Event | Payment Succeeded`

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
    "body": "eyJsaXZlIjoiZmFsc2UiLCJub3RpZmljYXRpb25JdGVtcyI6W3siTm90aWZpY2F0aW9uUmVxdWVzdEl0ZW0iOnsib3JpZ2luYWxSZWZlcmVuY2UiOiIiLCJwc3BSZWZlcmVuY2UiOiI4NTM1Mjk2NjUwMTUzMzE3IiwiYW1vdW50Ijp7InZhbHVlIjoyMDAwLCJjdXJyZW5jeSI6IlVTRCJ9LCJldmVudENvZGUiOiJBVVRIT1JJU0FUSU9OIiwibWVyY2hhbnRBY2NvdW50Q29kZSI6IlRlc3RNZXJjaGFudCIsIm1lcmNoYW50UmVmZXJlbmNlIjoidGVzdF9wYXltZW50XzAwMSIsInN1Y2Nlc3MiOiJ0cnVlIiwicmVhc29uIjoiIiwiYWRkaXRpb25hbERhdGEiOnsiaG1hY1NpZ25hdHVyZSI6InZsd21BcnVtSk9xaE1Lb3dnZkNScWxGczZ5bE9IdVVNRTgrdytucDFPMEU9In19fV19"
  },
  "merchant_event_id": "adyen_webhook_auth_001"
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
date: Tue, 14 Apr 2026 08:45:03 GMT
x-request-id: EventService/HandleEvent_payment_succeeded_req

Response contents:
{
  "eventType": "PAYMENT_INTENT_AUTHORIZATION_SUCCESS",
  "eventContent": {
    "paymentsResponse": {
      "connectorTransactionId": "8535296650153317",
      "status": "AUTHORIZED",
      "error": {
        "connectorDetails": {}
      },
      "statusCode": 200,
      "merchantTransactionId": "8535296650153317"
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
