#!/usr/bin/env bash
# Seed a Superposition workspace from config/superposition.toml — hyperswitch's
# scripts/seed_superposition.sh, ported. The baked file IS the seed: it is already
# SuperTOML ([default-configs], [dimensions], [[overrides]]), so the remote workspace
# and the file fallback start byte-equivalent.
#
#   SUPERPOSITION_URL=http://localhost:8080 ORG_ID=hyperswitch WORKSPACE_ID=prism \
#     scripts/seed_superposition.sh
#
# Env: SUPERPOSITION_URL (default http://localhost:8080), SEED_FILE
# (config/superposition.toml), ORG_ID (hyperswitch), WORKSPACE_ID (prism),
# WORKSPACE_ADMIN_EMAIL (required only when the workspace does not exist yet),
# SUPERPOSITION_TOKEN (optional bearer, for deployments fronted by an auth proxy),
# MAX_RETRIES (60) × RETRY_INTERVAL (2s) to wait for /health.
#
# The workspace is created when it does not exist yet (GET 404 -> POST), so a new
# environment needs no dashboard step before seeding. Then, in the order the server
# requires: dimensions by position (positions must stay dense; variantIds is
# position 0 and comes with the workspace), default-configs, contexts. Every create
# carries a description and a change_reason (the server requires them). 2xx or 409
# (already exists) is success — idempotent by 409, exactly like the reference
# script. Ends with a resolve self-check so a badly seeded workspace fails here, not
# silently at runtime.
set -euo pipefail

SUPERPOSITION_URL="${SUPERPOSITION_URL:-http://localhost:8080}"
SEED_FILE="${SEED_FILE:-config/superposition.toml}"
ORG_ID="${ORG_ID:-hyperswitch}"
WORKSPACE_ID="${WORKSPACE_ID:-prism}"
WORKSPACE_ADMIN_EMAIL="${WORKSPACE_ADMIN_EMAIL:-}"
SUPERPOSITION_TOKEN="${SUPERPOSITION_TOKEN:-}"
MAX_RETRIES="${MAX_RETRIES:-60}"
RETRY_INTERVAL="${RETRY_INTERVAL:-2}"
DESCRIPTION="seeded from config/superposition.toml"
CHANGE_REASON="prism superposition seed"

for dependency in curl jq; do
  command -v "$dependency" >/dev/null 2>&1 || { echo "error: '$dependency' not found in PATH" >&2; exit 127; }
done
[[ -f "$SEED_FILE" ]] || { echo "error: seed file not found: $SEED_FILE" >&2; exit 1; }

toml_to_json() {
  if command -v yq >/dev/null 2>&1; then
    yq -p toml -o json '.' "$SEED_FILE"
  elif python3 -c 'import tomllib' >/dev/null 2>&1; then
    python3 -c 'import json, sys, tomllib; json.dump(tomllib.load(open(sys.argv[1], "rb")), sys.stdout)' "$SEED_FILE"
  else
    echo "error: TOML parsing needs yq or Python 3.11+" >&2; exit 127
  fi
}

auth_header=()
[[ -n "$SUPERPOSITION_TOKEN" ]] && auth_header=(-H "Authorization: Bearer $SUPERPOSITION_TOKEN")

# $1=method $2=path $3=json body $4=label — 2xx or 409 succeed, anything else fails loudly.
call() {
  local method="$1" path="$2" body="$3" label="$4" status response
  response=$(curl -sS -o /tmp/seed_body.$$ -w '%{http_code}' -X "$method" "$SUPERPOSITION_URL$path" \
    -H 'Content-Type: application/json' -H "x-org-id: $ORG_ID" -H "x-workspace: $WORKSPACE_ID" \
    "${auth_header[@]}" -d "$body") || { echo "error: $label: curl failed" >&2; exit 1; }
  status="$response"
  if [[ "$status" =~ ^2 ]]; then echo "  ok   $label"
  elif [[ "$status" == "409" ]]; then echo "  skip $label (already exists)"
  else echo "error: $label -> HTTP $status: $(cat /tmp/seed_body.$$)" >&2; rm -f /tmp/seed_body.$$; exit 1; fi
  rm -f /tmp/seed_body.$$
}

echo "==> waiting for $SUPERPOSITION_URL/health"
for _ in $(seq 1 "$MAX_RETRIES"); do
  curl -sS -o /dev/null "$SUPERPOSITION_URL/health" && break
  sleep "$RETRY_INTERVAL"
done
curl -sS -o /dev/null "$SUPERPOSITION_URL/health" || { echo "error: Superposition not healthy" >&2; exit 1; }

SEED_JSON=$(toml_to_json)

echo "==> workspace $ORG_ID/$WORKSPACE_ID"
workspace_status=$(curl -sS -o /dev/null -w '%{http_code}' "$SUPERPOSITION_URL/workspaces/$WORKSPACE_ID" \
  -H "x-org-id: $ORG_ID" "${auth_header[@]}") || { echo "error: workspace lookup: curl failed" >&2; exit 1; }
case "$workspace_status" in
  200) echo "  skip workspace $WORKSPACE_ID (already exists)" ;;
  404)
    [[ -n "$WORKSPACE_ADMIN_EMAIL" ]] || { echo "error: workspace $WORKSPACE_ID does not exist; set WORKSPACE_ADMIN_EMAIL to create it" >&2; exit 1; }
    # strict_mode and workspace_admin_email are required by the server; the rest are defaults.
    body=$(jq -cn --arg name "$WORKSPACE_ID" --arg admin "$WORKSPACE_ADMIN_EMAIL" \
      '{workspace_name: $name, workspace_admin_email: $admin, workspace_status: "ENABLED", strict_mode: false}')
    call POST /workspaces "$body" "workspace $WORKSPACE_ID"
    ;;
  *) echo "error: workspace lookup -> HTTP $workspace_status" >&2; exit 1 ;;
esac

echo "==> dimensions (by position; variantIds is the workspace's own, position 0)"
echo "$SEED_JSON" | jq -c '.dimensions | to_entries | sort_by(.value.position) | .[] | select(.key != "variantIds")' | while read -r entry; do
  name=$(echo "$entry" | jq -r '.key')
  body=$(echo "$entry" | jq -c --arg d "$DESCRIPTION" --arg r "$CHANGE_REASON" \
    '{dimension: .key, position: .value.position, schema: .value.schema, dimension_type: {REGULAR: {}}, description: $d, change_reason: $r}')
  call POST /dimension "$body" "dimension $name"
done

echo "==> default-configs"
echo "$SEED_JSON" | jq -c '."default-configs" | to_entries | .[]' | while read -r entry; do
  key=$(echo "$entry" | jq -r '.key')
  body=$(echo "$entry" | jq -c --arg d "$DESCRIPTION" --arg r "$CHANGE_REASON" \
    '{key: .key, value: .value.value, schema: .value.schema, description: (.value.description // $d), change_reason: (.value.change_reason // $r)}')
  call POST /default-config "$body" "default-config $key"
done

echo "==> overrides (contexts)"
echo "$SEED_JSON" | jq -c '.overrides[]' | while read -r entry; do
  label=$(echo "$entry" | jq -c '._context_')
  body=$(echo "$entry" | jq -c --arg d "$DESCRIPTION" --arg r "$CHANGE_REASON" \
    '{context: ._context_, override: (del(._context_)), description: $d, change_reason: $r}')
  call PUT /context "$body" "context $label"
done

echo "==> self-check: the seeded workspace resolves what the file resolves"
resolve() { # $1=context json
  curl -sS -X POST "$SUPERPOSITION_URL/config/resolve" -H 'Content-Type: application/json' \
    -H "x-org-id: $ORG_ID" -H "x-workspace: $WORKSPACE_ID" "${auth_header[@]}" -d "{\"context\": $1}"
}
sampler=$(resolve '{"environment":"sandbox","rpc_method":"/types.PaymentService/Authorize"}')
echo "$sampler" | jq -e 'has("deja_record")' >/dev/null \
  || { echo "error: deja_record did not resolve for the sampler context: $sampler" >&2; exit 1; }
urls=$(resolve '{"connector":"stripe","environment":"sandbox"}')
echo "$urls" | jq -e '.connector_base_url | strings | length > 0' >/dev/null \
  || { echo "error: connector_base_url did not resolve for stripe/sandbox: $urls" >&2; exit 1; }
echo "  ok   deja_record=$(echo "$sampler" | jq -c '.deja_record') · stripe/sandbox base_url=$(echo "$urls" | jq -r '.connector_base_url')"
echo "done: org=$ORG_ID workspace=$WORKSPACE_ID seeded from $SEED_FILE"
