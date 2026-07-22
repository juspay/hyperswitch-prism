#!/usr/bin/env bash
#
# KillBill API test harness for the Hyperswitch Prism plugin (killbill-hyperswitch).
#
# Drives the full flow against a RUNNING KillBill that has the plugin installed:
#   1. create (or reuse) a tenant
#   2. upload the plugin config for that tenant
#   3. create an account
#   4. add a payment method (card)  -> plugin sets up a Prism mandate
#   5. purchase                      -> Prism authorize+capture on the connector
#   6. get payment (show status)
#   7. refund
#   8. get payment (show refund)
#   9. replay a connector webhook to the plugin's servlet
#
# This is the server-to-server analog of the Medusa demo app (minus the browser).
#
# Requires: bash, curl, jq — and a running KillBill (default :8080) with killbill-hyperswitch deployed.
# Every payment call hits the real connector sandbox, so configure a real sandbox key.
#
# Usage:
#   STRIPE_API_KEY=sk_test_xxx ./run.sh
#   CONNECTOR=adyen PLUGIN_CONFIG_FILE=./adyen.properties ./run.sh
#
set -euo pipefail

# ----------------------------- config (override via env) -----------------------------
KB_URL="${KB_URL:-http://127.0.0.1:8080}"
KB_USER="${KB_USER:-admin}"
KB_PASSWORD="${KB_PASSWORD:-password}"
KB_TENANT_KEY="${KB_TENANT_KEY:-hyperswitch-test}"
KB_TENANT_SECRET="${KB_TENANT_SECRET:-hyperswitch-test-secret}"
CREATED_BY="${CREATED_BY:-test-harness}"
PLUGIN_NAME="killbill-hyperswitch"

CONNECTOR="${CONNECTOR:-stripe}"
STRIPE_API_KEY="${STRIPE_API_KEY:-sk_test_REPLACE_ME}"
PLUGIN_CONFIG_FILE="${PLUGIN_CONFIG_FILE:-}"   # optional: full .properties body (for non-stripe connectors)

AMOUNT="${AMOUNT:-10.00}"
CURRENCY="${CURRENCY:-USD}"
CARD_NUMBER="${CARD_NUMBER:-4242424242424242}"   # Stripe test card; use the connector's own test card otherwise
CARD_EXP_MONTH="${CARD_EXP_MONTH:-12}"
CARD_EXP_YEAR="${CARD_EXP_YEAR:-2030}"
CARD_CVC="${CARD_CVC:-123}"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WEBHOOK_FILE="${WEBHOOK_FILE:-$SCRIPT_DIR/webhooks/stripe-payment_intent-succeeded.json}"

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

# ----------------------------- helpers -----------------------------
step(){ printf '\n\033[1;36m==> %s\033[0m\n' "$*"; }
info(){ printf '    %s\n' "$*"; }
warn(){ printf '\033[1;33m    ! %s\033[0m\n' "$*"; }
die(){  printf '\033[1;31mERROR: %s\033[0m\n' "$*" >&2; exit 1; }

command -v curl >/dev/null || die "curl not found"
command -v jq   >/dev/null || die "jq not found"

show_body(){ if jq -e . "$TMP/body" >/dev/null 2>&1; then jq . "$TMP/body"; else cat "$TMP/body"; echo; fi; }
# Some endpoints (e.g. POST …/payments) return a Location with a trailing slash — strip it before taking the last segment.
location_id(){ grep -i '^Location:' "$TMP/hdr" | tr -d '\r' | sed 's#/*$##; s#.*/##'; }
ok(){ case "$1" in 2*) return 0;; *) return 1;; esac; }

# RBAC + tenant + common headers. Args: METHOD PATH [JSON_BODY]. Prints the HTTP status code.
kb(){
  local method="$1" path="$2" data="${3:-}"
  local args=( -sS -X "$method" -u "$KB_USER:$KB_PASSWORD"
    -H "X-Killbill-ApiKey: $KB_TENANT_KEY" -H "X-Killbill-ApiSecret: $KB_TENANT_SECRET"
    -H "X-Killbill-CreatedBy: $CREATED_BY" -H "Content-Type: application/json" -H "Accept: application/json"
    -D "$TMP/hdr" -o "$TMP/body" -w '%{http_code}' )
  [ -n "$data" ] && args+=( --data "$data" )
  curl "${args[@]}" "$KB_URL$path"
}

# ----------------------------- preflight -----------------------------
step "Preflight — $KB_URL (plugin: $PLUGIN_NAME, connector: $CONNECTOR)"
if [ -z "$PLUGIN_CONFIG_FILE" ] && [ "$CONNECTOR" = "stripe" ] && [ "$STRIPE_API_KEY" = "sk_test_REPLACE_ME" ]; then
  warn "STRIPE_API_KEY is the placeholder — the purchase WILL fail at Stripe. Export a real sandbox key."
fi
code=$(curl -sS -o "$TMP/body" -w '%{http_code}' "$KB_URL/1.0/kb/nodesInfo" -u "$KB_USER:$KB_PASSWORD" || echo 000)
[ "$code" = "000" ] && die "KillBill not reachable at $KB_URL. Start KillBill + install the plugin first."
info "KillBill responded (HTTP $code)."

# ----------------------------- 1. tenant -----------------------------
step "1/9  Create tenant ($KB_TENANT_KEY)"
code=$(curl -sS -X POST -u "$KB_USER:$KB_PASSWORD" \
  -H "X-Killbill-CreatedBy: $CREATED_BY" -H "Content-Type: application/json" \
  -D "$TMP/hdr" -o "$TMP/body" -w '%{http_code}' \
  --data "{\"apiKey\":\"$KB_TENANT_KEY\",\"apiSecret\":\"$KB_TENANT_SECRET\"}" \
  "$KB_URL/1.0/kb/tenants")
case "$code" in
  201) info "tenant created";;
  409) info "tenant already exists — reusing";;
  *)   show_body; die "tenant creation failed ($code)";;
esac

# ----------------------------- 2. plugin config -----------------------------
step "2/9  Upload plugin config for the tenant"
if [ -n "$PLUGIN_CONFIG_FILE" ]; then
  cfg="$(cat "$PLUGIN_CONFIG_FILE")"
  info "using $PLUGIN_CONFIG_FILE"
else
  cfg=$(cat <<EOF
org.killbill.billing.plugin.hyperswitch.connector=$CONNECTOR
org.killbill.billing.plugin.hyperswitch.environment=SANDBOX
org.killbill.billing.plugin.hyperswitch.captureMethod=AUTOMATIC
org.killbill.billing.plugin.hyperswitch.$CONNECTOR.apiKey=$STRIPE_API_KEY
EOF
)
fi
code=$(curl -sS -X POST -u "$KB_USER:$KB_PASSWORD" \
  -H "X-Killbill-ApiKey: $KB_TENANT_KEY" -H "X-Killbill-ApiSecret: $KB_TENANT_SECRET" \
  -H "X-Killbill-CreatedBy: $CREATED_BY" -H "Content-Type: text/plain" \
  -o "$TMP/body" -w '%{http_code}' --data-binary "$cfg" \
  "$KB_URL/1.0/kb/tenants/uploadPluginConfig/$PLUGIN_NAME")
ok "$code" || { show_body; die "config upload failed ($code)"; }
info "config uploaded ($code)"

# ----------------------------- 3. account -----------------------------
step "3/9  Create account"
# Full billing address on the account: connectors like Cybersource require a complete bill-to for mandate setup.
acct="{\"currency\":\"$CURRENCY\",\"name\":\"John Doe\",\"email\":\"harness@example.com\",\"address1\":\"1295 Charleston Rd\",\"city\":\"Mountain View\",\"state\":\"CA\",\"postalCode\":\"94043\",\"country\":\"US\",\"externalKey\":\"harness-$(date +%s)-$RANDOM\"}"
code=$(kb POST /1.0/kb/accounts "$acct")
ok "$code" || { show_body; die "account creation failed ($code)"; }
ACCOUNT_ID=$(location_id)
[ -n "$ACCOUNT_ID" ] || die "could not parse accountId from Location"
info "accountId=$ACCOUNT_ID"

# ----------------------------- 4. payment method -----------------------------
step "4/9  Add payment method (card) — plugin sets up a Prism mandate"
pm=$(cat <<EOF
{"pluginName":"$PLUGIN_NAME","pluginInfo":{"properties":[
  {"key":"ccNumber","value":"$CARD_NUMBER","isUpdatable":false},
  {"key":"ccExpirationMonth","value":"$CARD_EXP_MONTH","isUpdatable":false},
  {"key":"ccExpirationYear","value":"$CARD_EXP_YEAR","isUpdatable":false},
  {"key":"ccVerificationValue","value":"$CARD_CVC","isUpdatable":false},
  {"key":"ccFirstName","value":"John","isUpdatable":false},
  {"key":"ccLastName","value":"Doe","isUpdatable":false}
]}}
EOF
)
code=$(kb POST "/1.0/kb/accounts/$ACCOUNT_ID/paymentMethods?isDefault=true" "$pm")
ok "$code" || { show_body; die "add payment method failed ($code)"; }
PM_ID=$(location_id)
info "paymentMethodId=$PM_ID"

# ----------------------------- 5. purchase -----------------------------
step "5/9  Purchase $AMOUNT $CURRENCY"
# paymentMethodId is a query parameter on this endpoint, not a PaymentTransactionJson field.
pay="{\"transactionType\":\"PURCHASE\",\"amount\":$AMOUNT,\"currency\":\"$CURRENCY\"}"
code=$(kb POST "/1.0/kb/accounts/$ACCOUNT_ID/payments?withPluginInfo=true&paymentMethodId=$PM_ID" "$pay")
ok "$code" || { show_body; warn "purchase returned $code (connector may have declined — inspect below)"; }
PAYMENT_ID=$(jq -r '.paymentId // empty' "$TMP/body" 2>/dev/null || true)
[ -n "$PAYMENT_ID" ] || PAYMENT_ID=$(location_id)
info "paymentId=$PAYMENT_ID"
jq '.transactions[]? | {type:.transactionType, status:.status, amount:.amount, gatewayError:.gatewayErrorMsg, ref:.firstPaymentReferenceId}' "$TMP/body" 2>/dev/null || show_body
CONNECTOR_TXN_ID=$(jq -r '[.transactions[]?.firstPaymentReferenceId] | map(select(. != null)) | last // empty' "$TMP/body" 2>/dev/null || true)
[ -n "$CONNECTOR_TXN_ID" ] && info "connectorTransactionId=$CONNECTOR_TXN_ID"

# ----------------------------- 6. get payment -----------------------------
step "6/9  Get payment (with plugin info)"
code=$(kb GET "/1.0/kb/payments/$PAYMENT_ID?withPluginInfo=true&withAttempts=true")
ok "$code" || warn "get returned $code"
jq '.transactions[]? | {type:.transactionType, status:.status}' "$TMP/body" 2>/dev/null || show_body

# ----------------------------- 7. refund -----------------------------
step "7/9  Refund $AMOUNT $CURRENCY"
code=$(kb POST "/1.0/kb/payments/$PAYMENT_ID/refunds?withPluginInfo=true" "{\"amount\":$AMOUNT,\"currency\":\"$CURRENCY\"}")
ok "$code" || { show_body; warn "refund returned $code"; }
jq '.transactions[]? | {type:.transactionType, status:.status}' "$TMP/body" 2>/dev/null || show_body

# ----------------------------- 8. get payment (after refund) -----------------------------
step "8/9  Get payment (after refund)"
code=$(kb GET "/1.0/kb/payments/$PAYMENT_ID?withPluginInfo=true")
jq '.transactions[]? | {type:.transactionType, status:.status}' "$TMP/body" 2>/dev/null || show_body

# ----------------------------- 9. replay webhook -----------------------------
step "9/9  Replay a connector webhook to the plugin servlet"
info "POST $KB_URL/plugins/$PLUGIN_NAME/webhook   (body: $WEBHOOK_FILE)"
if [ -f "$WEBHOOK_FILE" ]; then
  code=$(curl -sS -X POST \
    -H "X-Killbill-ApiKey: $KB_TENANT_KEY" -H "X-Killbill-ApiSecret: $KB_TENANT_SECRET" \
    -H "Content-Type: application/json" -o "$TMP/body" -w '%{http_code}' \
    --data-binary @"$WEBHOOK_FILE" \
    "$KB_URL/plugins/$PLUGIN_NAME/webhook")
  info "HTTP $code"; show_body
  warn "A replayed webhook won't pass the connector's signature check → expect {\"status\":\"rejected\"}."
  warn "For a real webhook test, use the connector dashboard's 'send test event' with a valid signature,"
  warn "and set the webhookSecret in the plugin config."
else
  warn "webhook body not found: $WEBHOOK_FILE (skipping)"
fi

# ----------------------------- summary -----------------------------
step "Done"
info "tenant=$KB_TENANT_KEY  account=$ACCOUNT_ID  paymentMethod=$PM_ID  payment=$PAYMENT_ID"
info "Inspect in Kaui, or: curl -u $KB_USER:*** -H 'X-Killbill-ApiKey: $KB_TENANT_KEY' -H 'X-Killbill-ApiSecret: ***' '$KB_URL/1.0/kb/payments/$PAYMENT_ID?withPluginInfo=true'"
