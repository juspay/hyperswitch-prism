#!/usr/bin/env bash
# Direct grpcurl commands for end-to-end verification of mock + grpc-server.
#
# Two-process flow: grpcurl hits grpc-server on :8000, which routes
# x-connector: dummy through the Dummy Rust connector, which POSTs HTTP
# (Stripe-shape, form-urlencoded) to mock on :8777.
#
# Prereqs (run from repo root, each in its own terminal):
#   1. cargo run -p mock --release            # mock HTTP backend on :8777
#   2. cargo run -p grpc-server               # UCS grpc-server on :8000
#   3. data/field_probe/dummy.json must exist (regen with
#      `cargo run -p field-probe --release` if missing).
#   4. grpcurl, jq, curl installed.
#
# Run end-to-end: `bash mock/grpc-test-commands.sh`
# Or copy-paste individual sections.

set -euo pipefail

cd "$(git rev-parse --show-toplevel)"

HDRS=(
  -H 'x-connector: dummy'
  -H 'x-auth: header-key'
  -H 'x-api-key: sk_test_dummy'
  -H 'x-merchant-id: m_grpcurl_run'
  -H 'x-tenant-id: default'
)
rid() { echo "rid_${1}_$(date +%s)_$RANDOM"; }

# Field-probe samples wrap SecretString / CardNumberType as bare strings; proto JSON
# needs them as {value: "..."} objects. wrap_card() applies that transform plus an
# optional card_number override.
wrap_card() {
  local override_number="${1:-}"
  if [[ -n "$override_number" ]]; then
    jq --arg pan "$override_number" '.flows.authorize.Card.proto_request
      | .payment_method.card.card_number      = {value: $pan}
      | .payment_method.card.card_exp_month   = {value: .payment_method.card.card_exp_month}
      | .payment_method.card.card_exp_year    = {value: .payment_method.card.card_exp_year}
      | .payment_method.card.card_cvc         = {value: .payment_method.card.card_cvc}
      | .payment_method.card.card_holder_name = {value: .payment_method.card.card_holder_name}' \
      data/field_probe/dummy.json
  else
    jq '.flows.authorize.Card.proto_request
      | .payment_method.card.card_number      = {value: .payment_method.card.card_number}
      | .payment_method.card.card_exp_month   = {value: .payment_method.card.card_exp_month}
      | .payment_method.card.card_exp_year    = {value: .payment_method.card.card_exp_year}
      | .payment_method.card.card_cvc         = {value: .payment_method.card.card_cvc}
      | .payment_method.card.card_holder_name = {value: .payment_method.card.card_holder_name}' \
      data/field_probe/dummy.json
  fi
}

wrap_upi() {
  local vpa="$1"
  jq --arg v "$vpa" '.flows.authorize.UpiCollect.proto_request
    | .payment_method.upi_collect.vpa_id = {value: $v}' data/field_probe/dummy.json
}

# =====================================================================
# A. Authorize sweep — one cell per v1 payment method
# =====================================================================

echo "=== A1: Card success (4242424242424242) — expect CHARGED ==="
grpcurl -plaintext "${HDRS[@]}" -H "x-request-id: $(rid a1)" \
  -d "$(wrap_card 4242424242424242)" \
  localhost:8000 types.PaymentService/Authorize | jq '{status, ctid: .connectorTransactionId}'

echo "=== A2: Card decline (4000000000000002) — expect FAILURE, msg contains 'declined' ==="
grpcurl -plaintext "${HDRS[@]}" -H "x-request-id: $(rid a2)" \
  -d "$(wrap_card 4000000000000002)" \
  localhost:8000 types.PaymentService/Authorize | jq '{status, msg: .error.connectorDetails.message}'

echo "=== A3: Card 3DS (4000003800000446) — expect AUTHENTICATION_PENDING + redirect URL ==="
grpcurl -plaintext "${HDRS[@]}" -H "x-request-id: $(rid a3)" -emit-defaults \
  -d "$(wrap_card 4000003800000446)" \
  localhost:8000 types.PaymentService/Authorize | jq '{status, redirect: .redirectionData.form.endpoint}'

echo "=== A4: UPI success (success@upi) — expect CHARGED ==="
grpcurl -plaintext "${HDRS[@]}" -H "x-request-id: $(rid a4)" \
  -d "$(wrap_upi success@upi)" \
  localhost:8000 types.PaymentService/Authorize | jq '{status}'

echo "=== A5: UPI failure (failure@upi) — expect FAILURE, code upi_declined ==="
grpcurl -plaintext "${HDRS[@]}" -H "x-request-id: $(rid a5)" \
  -d "$(wrap_upi failure@upi)" \
  localhost:8000 types.PaymentService/Authorize | jq '{status, code: .error.connectorDetails.code}'

# A6 Bancontact — has card fields that need wrapping
echo "=== A6: Bancontact — expect AUTHENTICATION_PENDING + redirect URL ==="
BANCONTACT_REQ=$(jq '.flows.authorize.BancontactCard.proto_request
  | .payment_method.bancontact_card.card_number      = {value: .payment_method.bancontact_card.card_number}
  | .payment_method.bancontact_card.card_exp_month   = {value: .payment_method.bancontact_card.card_exp_month}
  | .payment_method.bancontact_card.card_exp_year    = {value: .payment_method.bancontact_card.card_exp_year}
  | .payment_method.bancontact_card.card_holder_name = {value: .payment_method.bancontact_card.card_holder_name}' \
  data/field_probe/dummy.json)
grpcurl -plaintext "${HDRS[@]}" -H "x-request-id: $(rid a6)" -emit-defaults \
  -d "$BANCONTACT_REQ" \
  localhost:8000 types.PaymentService/Authorize | jq '{status, redirect: .redirectionData.form.endpoint}'

# A7-A15: empty-oneof redirect PMs — no field wrapping needed
for entry in \
    'A7:Ideal' \
    'A8:Trustly' \
    'A9:Blik' \
    'A10:MbWay' \
    'A11:Satispay' \
    'A12:Wero' \
    'A13:AliPayRedirect' \
    'A14:WeChatPayQr' \
    'A15:RevolutPay'; do
  id="${entry%%:*}"
  pm="${entry##*:}"
  echo "=== $id: $pm — expect AUTHENTICATION_PENDING + redirect URL ==="
  grpcurl -plaintext "${HDRS[@]}" -H "x-request-id: $(rid "$id")" -emit-defaults \
    -d "$(jq ".flows.authorize.$pm.proto_request" data/field_probe/dummy.json)" \
    localhost:8000 types.PaymentService/Authorize | jq '{status, redirect: .redirectionData.form.endpoint}'
done

# =====================================================================
# B. Manual capture flow — Card 4242, MANUAL → Capture → Refund → RSync
# =====================================================================

echo ""
echo "=== B1: Card 4242 MANUAL — expect AUTHORIZED ==="
B1_REQ=$(wrap_card 4242424242424242 | jq '.capture_method = "MANUAL"')
B1_RESP=$(grpcurl -plaintext "${HDRS[@]}" -H "x-request-id: $(rid b1)" -d "$B1_REQ" \
  localhost:8000 types.PaymentService/Authorize)
echo "$B1_RESP" | jq '{status, ctid: .connectorTransactionId}'
CTID_B=$(echo "$B1_RESP" | jq -r .connectorTransactionId)

echo "=== B2: PSync — expect AUTHORIZED ==="
B2_REQ=$(jq --arg ct "$CTID_B" '.flows.get.default.proto_request
  | .connector_transaction_id = $ct
  | .amount = {minor_amount: 1000, currency: "USD"}' data/field_probe/dummy.json)
grpcurl -plaintext "${HDRS[@]}" -H "x-request-id: $(rid b2)" -d "$B2_REQ" \
  localhost:8000 types.PaymentService/Get | jq '{status}'

echo "=== B3: Capture (full) — expect CHARGED ==="
B3_REQ=$(jq --arg ct "$CTID_B" '.flows.capture.default.proto_request
  | .connector_transaction_id = $ct' data/field_probe/dummy.json)
grpcurl -plaintext "${HDRS[@]}" -H "x-request-id: $(rid b3)" -d "$B3_REQ" \
  localhost:8000 types.PaymentService/Capture | jq '{status}'

echo "=== B4: PSync after capture — expect CHARGED ==="
grpcurl -plaintext "${HDRS[@]}" -H "x-request-id: $(rid b4)" -d "$B2_REQ" \
  localhost:8000 types.PaymentService/Get | jq '{status}'

echo "=== B5: Refund (full) — expect REFUND_SUCCESS ==="
B5_REQ=$(jq --arg ct "$CTID_B" '.flows.refund.default.proto_request
  | .connector_transaction_id = $ct' data/field_probe/dummy.json)
B5_RESP=$(grpcurl -plaintext "${HDRS[@]}" -H "x-request-id: $(rid b5)" -d "$B5_REQ" \
  localhost:8000 types.PaymentService/Refund)
echo "$B5_RESP" | jq '{status, refundId: .connectorRefundId}'
RID_B=$(echo "$B5_RESP" | jq -r .connectorRefundId)

echo "=== B6: RefundSync — expect REFUND_SUCCESS ==="
B6_REQ=$(jq --arg ct "$CTID_B" --arg rid "$RID_B" '.flows.refund_get.default.proto_request
  | .connector_transaction_id = $ct
  | .refund_id = $rid' data/field_probe/dummy.json)
grpcurl -plaintext "${HDRS[@]}" -H "x-request-id: $(rid b6)" -d "$B6_REQ" \
  localhost:8000 types.RefundService/Get | jq '{status}'

# =====================================================================
# C. Void flow — Card 4242 MANUAL → Void → PSync
# =====================================================================

echo ""
echo "=== C1: Card 4242 MANUAL (separate PI) — expect AUTHORIZED ==="
C1_RESP=$(grpcurl -plaintext "${HDRS[@]}" -H "x-request-id: $(rid c1)" -d "$B1_REQ" \
  localhost:8000 types.PaymentService/Authorize)
CTID_C=$(echo "$C1_RESP" | jq -r .connectorTransactionId)
echo "$C1_RESP" | jq '{status, ctid: .connectorTransactionId}'

echo "=== C2: Void — expect VOIDED ==="
C2_REQ=$(jq --arg ct "$CTID_C" '.flows.void.default.proto_request
  | .connector_transaction_id = $ct' data/field_probe/dummy.json)
grpcurl -plaintext "${HDRS[@]}" -H "x-request-id: $(rid c2)" -d "$C2_REQ" \
  localhost:8000 types.PaymentService/Void | jq '{status}'

echo "=== C3: PSync after void — expect VOIDED ==="
C3_REQ=$(jq --arg ct "$CTID_C" '.flows.get.default.proto_request
  | .connector_transaction_id = $ct
  | .amount = {minor_amount: 1000, currency: "USD"}' data/field_probe/dummy.json)
grpcurl -plaintext "${HDRS[@]}" -H "x-request-id: $(rid c3)" -d "$C3_REQ" \
  localhost:8000 types.PaymentService/Get | jq '{status}'

# =====================================================================
# D. Redirect-completion happy path — Bancontact → visit URL → PSync CHARGED
# =====================================================================

echo ""
echo "=== D1: Bancontact authorize ==="
D1_RESP=$(grpcurl -plaintext "${HDRS[@]}" -H "x-request-id: $(rid d1)" -emit-defaults \
  -d "$BANCONTACT_REQ" \
  localhost:8000 types.PaymentService/Authorize)
URL_D=$(echo "$D1_RESP"  | jq -r .redirectionData.form.endpoint)
CTID_D=$(echo "$D1_RESP" | jq -r .connectorTransactionId)
echo "$D1_RESP" | jq '{status, redirect: .redirectionData.form.endpoint, ctid: .connectorTransactionId}'

echo "=== D2: Visit redirect URL (browser-step simulator) ==="
curl -s -o /dev/null -w 'redirect HTTP: %{http_code}\n' "$URL_D"

echo "=== D3: PSync after redirect — expect CHARGED ==="
D3_REQ=$(jq --arg ct "$CTID_D" '.flows.get.default.proto_request
  | .connector_transaction_id = $ct
  | .amount = {minor_amount: 1000, currency: "USD"}' data/field_probe/dummy.json)
grpcurl -plaintext "${HDRS[@]}" -H "x-request-id: $(rid d3)" -d "$D3_REQ" \
  localhost:8000 types.PaymentService/Get | jq '{status}'

# =====================================================================
# E. Redirect-rejection path — iDeal → visit URL with ?reject=1 → PSync FAILURE
# =====================================================================

echo ""
echo "=== E1: iDeal authorize ==="
E1_RESP=$(grpcurl -plaintext "${HDRS[@]}" -H "x-request-id: $(rid e1)" -emit-defaults \
  -d "$(jq '.flows.authorize.Ideal.proto_request' data/field_probe/dummy.json)" \
  localhost:8000 types.PaymentService/Authorize)
URL_E=$(echo "$E1_RESP"  | jq -r .redirectionData.form.endpoint)
CTID_E=$(echo "$E1_RESP" | jq -r .connectorTransactionId)
echo "$E1_RESP" | jq '{status, redirect: .redirectionData.form.endpoint}'

echo "=== E2: Visit redirect URL with ?reject=1 ==="
curl -s -o /dev/null -w 'redirect HTTP: %{http_code}\n' "${URL_E}?reject=1"

echo "=== E3: PSync — expect FAILURE, code redirect_rejected ==="
E3_REQ=$(jq --arg ct "$CTID_E" '.flows.get.default.proto_request
  | .connector_transaction_id = $ct
  | .amount = {minor_amount: 1000, currency: "USD"}' data/field_probe/dummy.json)
grpcurl -plaintext "${HDRS[@]}" -H "x-request-id: $(rid e3)" -d "$E3_REQ" \
  localhost:8000 types.PaymentService/Get | jq '{status, code: .error.connectorDetails.code, msg: .error.connectorDetails.message}'

# =====================================================================
# F. Admin webhook trigger — POST /dummy/admin/trigger-webhook
#    Uses curl (HTTP, not gRPC) because the trigger is admin-only on the mock backend itself.
# =====================================================================

echo ""
echo "=== F1: Webhook trigger — sink receives Stripe-shaped event ==="
echo "(Start a sink first: python3 -c \"import http.server; ...\" listening on 127.0.0.1:9004)"
CTID_F=$(grpcurl -plaintext "${HDRS[@]}" -H "x-request-id: $(rid f1auth)" \
  -d "$(wrap_card 4242424242424242)" \
  localhost:8000 types.PaymentService/Authorize | jq -r .connectorTransactionId)
echo "Webhook PI: $CTID_F"
curl -s -H 'Authorization: Bearer sk_test_dummy' -H 'Content-Type: application/json' \
  http://127.0.0.1:8777/dummy/admin/trigger-webhook \
  -d "{\"target_url\":\"http://127.0.0.1:9004/webhook\",\"payment_intent_id\":\"$CTID_F\",\"event_type\":\"payment_intent.succeeded\"}" | jq

echo "=== F2: Webhook trigger to unreachable target — expect 502 ==="
curl -s -i -H 'Authorization: Bearer sk_test_dummy' -H 'Content-Type: application/json' \
  http://127.0.0.1:8777/dummy/admin/trigger-webhook \
  -d "{\"target_url\":\"http://127.0.0.1:1/never\",\"payment_intent_id\":\"$CTID_F\",\"event_type\":\"payment_intent.succeeded\"}" \
  | head -10
