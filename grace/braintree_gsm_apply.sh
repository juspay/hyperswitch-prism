#!/usr/bin/env bash
# Apply the Braintree GSM row set to a Hyperswitch instance.
#
# GSM rows are RUNTIME DATA in the `gateway_status_map` table, not code — no
# migration seeds them and no connector's rows are checked into the HS repo.
# This script POSTs them through the admin API.
#
#   HS_BASE_URL=https://sandbox.hyperswitch.io HS_ADMIN_API_KEY=... ./braintree_gsm_apply.sh
#
# Re-running is safe: a row that already exists is reported and skipped.
set -euo pipefail
BASE="${HS_BASE_URL:?set HS_BASE_URL}"
KEY="${HS_ADMIN_API_KEY:?set HS_ADMIN_API_KEY}"
ROWS="$(dirname "$0")/braintree_gsm_rows.json"
total=$(jq length "$ROWS"); ok=0; skip=0; fail=0
for i in $(seq 0 $((total - 1))); do
  row=$(jq -c ".[$i]" "$ROWS")
  code=$(jq -r .code <<<"$row"); sub=$(jq -r .sub_flow <<<"$row")
  resp=$(curl -sS -o /dev/null -w '%{http_code}' -X POST "$BASE/gsm" \
    -H "api-key: $KEY" -H 'Content-Type: application/json' -d "$row" || echo 000)
  case "$resp" in
    2*) ok=$((ok+1));;
    4*) skip=$((skip+1)); echo "skip  $sub/$code (HTTP $resp — already present or rejected)";;
    *)  fail=$((fail+1)); echo "FAIL  $sub/$code (HTTP $resp)";;
  esac
done
echo "created=$ok skipped=$skip failed=$fail of $total"
