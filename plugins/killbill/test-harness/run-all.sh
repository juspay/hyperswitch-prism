#!/usr/bin/env bash
#
# Run the full KillBill flow for EVERY parity connector that has a config, and print a PASS/FAIL summary.
#
# Setup: create configs/<connector>.properties for each connector you want to test — the file's contents are
# uploaded verbatim as the plugin config. The keys are listed in ../src/main/resources/hyperswitch.properties.
# Then:
#   ./run-all.sh
#
# Each connector runs in its own tenant (hs-<connector>) so their configs don't collide. Per-connector output
# is captured in logs/<connector>.log. Requires a running KillBill with the plugin installed (see README).
#
set -uo pipefail   # NOT -e: one connector's failure must not abort the sweep

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"
mkdir -p logs

# Card connectors + a default sandbox test card each. PayPal is a redirect/wallet connector — the raw-card
# server-to-server flow doesn't apply to it (it needs the 3DS/redirect flow, deferred in v1).
CONNECTORS=(stripe adyen braintree cybersource forte paypal)
declare -A CARD=(
  [stripe]=4242424242424242
  [adyen]=4111111111111111
  [braintree]=4111111111111111
  [cybersource]=4111111111111111
  [forte]=4111111111111111
  [paypal]=
)
declare -A RESULT

for c in "${CONNECTORS[@]}"; do
  if [ "$c" = "paypal" ]; then
    RESULT[$c]="SKIP — redirect connector; raw-card flow N/A (needs 3DS/redirect, deferred)"
    continue
  fi
  cfg="configs/$c.properties"
  if [ ! -f "$cfg" ]; then
    RESULT[$c]="SKIP — no configs/$c.properties (create it; keys in ../src/main/resources/hyperswitch.properties)"
    continue
  fi

  printf '\n\033[1;35m########## %s ##########\033[0m\n' "$c"
  if CONNECTOR="$c" PLUGIN_CONFIG_FILE="$cfg" CARD_NUMBER="${CARD[$c]}" \
     KB_TENANT_KEY="hs-$c" KB_TENANT_SECRET="hs-$c-secret" \
     bash run.sh 2>&1 | tee "logs/$c.log"; then
    if grep -qE '"status": *"SUCCESS"' "logs/$c.log"; then
      RESULT[$c]="PASS — purchase SUCCESS"
    elif grep -qE '"status": *"PENDING"' "logs/$c.log"; then
      RESULT[$c]="PASS (PENDING — awaiting webhook)"
    else
      RESULT[$c]="RAN — no SUCCESS status; inspect logs/$c.log"
    fi
  else
    RESULT[$c]="FAIL — flow errored; inspect logs/$c.log"
  fi
done

printf '\n\033[1;36m================ CONNECTOR SUMMARY ================\033[0m\n'
for c in "${CONNECTORS[@]}"; do printf '  %-12s %s\n' "$c" "${RESULT[$c]}"; done
printf '\033[1;36m==================================================\033[0m\n'
