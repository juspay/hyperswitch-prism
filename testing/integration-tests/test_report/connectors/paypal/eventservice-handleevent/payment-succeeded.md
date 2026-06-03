# Connector `paypal` / Suite `EventService/HandleEvent` / Scenario `Handle Event | Payment Succeeded`

- Service: `EventService/HandleEvent`
- Scenario Key: `payment_succeeded`
- PM / PMT: `-` / `-`
- Result: `PASS`

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
    "body": "eyJpZCI6IldILTdZWDQ5ODIzUzIyOTA4MzBLLTBKRTEzMjk2VzY4NTUyMzUiLCJldmVudF90eXBlIjoiUEFZTUVOVC5DQVBUVVJFLkNPTVBMRVRFRCIsInJlc291cmNlX3R5cGUiOiJjYXB0dXJlIiwic3VtbWFyeSI6IlBheW1lbnQgY29tcGxldGVkIGZvciAkIDIwLjAwIFVTRCIsInJlc291cmNlIjp7InN1cHBsZW1lbnRhcnlfZGF0YSI6eyJyZWxhdGVkX2lkcyI6eyJvcmRlcl9pZCI6IjVPMTkwMTI3VE4zNjQ3MTVUIn19LCJhbW91bnQiOnsiY3VycmVuY3lfY29kZSI6IlVTRCIsInZhbHVlIjoiMjAuMDAifSwiaW52b2ljZV9pZCI6InRlc3RfaW52b2ljZV8wMDEifSwiY3JlYXRlX3RpbWUiOiIyMDIzLTA0LTA1VDEyOjAwOjAwLjAwMFoifQ=="
  },
  "merchant_event_id": "paypal_webhook_capture_001"
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
      "connectorTransactionId": "5O190127TN364715T",
      "status": "CHARGED",
      "error": {
        "connectorDetails": {}
      },
      "statusCode": 200
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
