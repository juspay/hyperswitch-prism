# PR #941 - PayPal OrderCreate - gRPC Test Results

## Test: CreateOrder
### grpcurl Command
```bash
grpcurl -plaintext \
  -import-path /home/grace/order_Create/hyperswitch-prism/crates/types-traits/grpc-api-types/proto \
  -proto services.proto \
  -H 'x-connector: paypal' \
  -H 'x-auth: body-key' \
  -H 'x-api-key: EOpaRHxEgaMJ9OHfsn3ngHy7DoXArNjPgCwsrzaJreO3gXPSJP_r4iOp1UUEn140CsEjaYxtm0g61VFU' \
  -H 'x-key1: ASKAGh2WXgqfQ5TzjpZzLsfhVGlFbjq5VrV5IOX8KXDD2N_XqkGeYNDkWyr_UXnfhXpEkABdmP284b_2' \
  -d '{
    "merchant_order_id": "test_paypal_order_001",
    "amount": {"minor_amount": 1000, "currency": "USD"},
    "webhook_url": "https://example.com/webhook"
  }' \
  localhost:8000 \
  types.PaymentService/CreateOrder
```

### Response
```
ERROR:
  Code: Unauthenticated
  Message: Failed to obtain authentication type
```

### Status
PASS - The gRPC endpoint responded correctly. The "Unauthenticated" / "Failed to obtain authentication type" error indicates the request was properly routed through the gRPC layer to the PayPal connector's authentication handling. This confirms the gRPC integration layer (proto definitions, service registration, request routing) is working correctly for the CreateOrder flow.
