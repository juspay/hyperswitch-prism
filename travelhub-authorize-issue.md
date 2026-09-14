# Travelhub Authorize - INVALID Response

## Summary

Authorize request sent to Travelhub (Worldline) preprod environment returns `result: "INVALID"` with `transactionId: null`. The connector rejects the request without creating a transaction.

## Request

**Endpoint:** `POST https://preprod.travel.worldline-solutions.com/travelhub/tpa/api/v1/authorize`

**Headers:**
```
Content-Type: application/json
Authorization: Basic <base64-encoded-credentials>
via: HyperSwitch
```

**Body:**
```json
{
  "merchantId": "15974111",
  "orderId": "azharamin-WL_1787298217-1",
  "amount": 1000,
  "currency": "USD",
  "capture": true,
  "payment": {
    "paymentMethod": {
      "code": "108"
    },
    "paymentCard": {
      "cardName": "Juspay",
      "cardNumber": "4330264936344675",
      "expiryDate": "1126",
      "cvc": "123",
      "request3DS": {
        "cavv": "MTIzNDU2Nzg5MDA5ODc2NTQzMjE=",
        "cavvAlgorithm": "1",
        "eci": "02"
      },
      "authentication": false
    }
  }
}
```

## Response

**HTTP Status:** `200 OK`

**Headers:**
```
Content-Type: application/json
```

**Body:**
```json
{
  "merchantId": "15974111",
  "orderId": "azharamin-WL_1787298217-1",
  "transactionId": null,
  "amount": 1000,
  "currency": "USD",
  "result": "INVALID",
  "paymentMethodCode": "108"
}
```

## Observations

1. The `orderId` is echoed back correctly.
2. `transactionId` is `null` — no transaction was created.
3. `result` is `"INVALID"` — no error message or error code is provided to explain why.
4. The same response is returned for different card numbers (tested with `4111111111111111` and `4330264936344675`).
5. The 3DS data (`cavv`, `eci`) is being passed in `request3DS` with `authentication: false` (indicating the payment is already authenticated).

## Questions for Travelhub/Worldline

1. What causes the `INVALID` result? Is it related to the card data, 3DS authentication data, merchant account configuration, or currency?
2. Is there a way to get a more detailed error message or error code when the result is `INVALID`?
3. Does the merchant account (`merchantId: 15974111`) support USD currency on the preprod environment?
4. Are the test card numbers `4111111111111111` / `4330264936344675` valid for this merchant account on preprod?
5. Is the `cavv` value format correct? The value `MTIzNDU2Nzg5MDA5ODc2NTQzMjE=` is base64-encoded. Should it be in a different format?
6. Should `authentication: false` be set when `request3DS` data is present (already authenticated scenario)?
