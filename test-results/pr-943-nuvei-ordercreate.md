# PR #943 - Nuvei OrderCreate - gRPC Test Results

## Test: CreateOrder
### grpcurl Command
```bash
grpcurl -plaintext \
  -H 'x-connector: nuvei' \
  -H 'x-auth: signature-key' \
  -H 'x-api-key: 8068622888249377739' \
  -H 'x-key1: 1005117' \
  -H 'x-api-secret: xHcjAsY4wAX81Ud30KTd0Q7uZ2I3f1MeI1AdPQd21ymowva6uwstvudDcZEw2YdZ' \
  -d '{
    "merchant_order_id": "test_nuvei_order_001",
    "amount": {"minor_amount": 1000, "currency": "USD"},
    "webhook_url": "https://example.com/webhook"
  }' \
  localhost:8000 \
  types.PaymentService/CreateOrder
```

### Response
```json
{
  "connectorOrderId": "15537595121",
  "status": "PENDING",
  "statusCode": 200,
  "responseHeaders": {
    "access-control-allow-headers": "content-type, X-PINGOTHER",
    "access-control-allow-methods": "GET, POST",
    "access-control-allow-origin": "*",
    "connection": "keep-alive",
    "content-length": "312",
    "content-type": "application/json;charset=UTF-8",
    "date": "Tue, 07 Apr 2026 09:48:19 GMT",
    "p3p": "CP=\"ALL ADM DEV PSAi COM NAV OUR OTR STP IND DEM\"",
    "server": "nginx",
    "set-cookie": "JSESSIONID=757c6cae1715438c28422c02ac92; Path=/ppp; Secure; HttpOnly; SameSite=None"
  },
  "rawConnectorRequest": {
    "value": "{\"url\":\"https://ppp-test.nuvei.com/ppp/api/v1/openOrder.do\",\"method\":\"POST\",\"headers\":{\"Content-Type\":\"application/json\",\"via\":\"HyperSwitch\"},\"body\":{\"merchantId\":\"8068622888249377739\",\"merchantSiteId\":\"1005117\",\"clientUniqueId\":\"test_nuvei_order_001\",\"clientRequestId\":\"test_nuvei_order_001\",\"currency\":\"USD\",\"amount\":\"10.00\",\"timeStamp\":\"20260407094818\",\"checksum\":\"556ae6c67ef9581e4b378bad0c6753aee0b1c4abc676a98e9bff86b886e52ada\",\"transactionType\":\"Auth\"}}"
  },
  "rawConnectorResponse": {
    "value": "{\"internalRequestId\":162727232121,\"status\":\"SUCCESS\",\"errCode\":0,\"reason\":\"\",\"merchantId\":\"8068622888249377739\",\"merchantSiteId\":\"1005117\",\"version\":\"1.0\",\"clientRequestId\":\"test_nuvei_order_001\",\"sessionToken\":\"77fc92001fb04c2ca5e23089f708c7410121\",\"clientUniqueId\":\"test_nuvei_order_001\",\"orderId\":15537595121}"
  }
}
```

### Status
PASS
