#!/usr/bin/env bash
# End-to-end sampler verification: boots UCS in record mode against the local
# rig per policy case and asserts whether tape events land on the Kafka topic.
set -u
ROOT=~/new-grpc/prism-rebase
HERE=$ROOT/scripts/deja-local
BIN=$ROOT/target/debug/grpc-server
SP=$ROOT/config/superposition.toml
TOPIC=ucs-deja-recording-events
LOG=/tmp/deja-e2e-server.log
PASS=0; FAIL=0

restore() {
  [[ -f "$SP.hidden" ]] && mv "$SP.hidden" "$SP"
  cp "$SP.e2e-backup" "$SP" 2>/dev/null; rm -f "$SP.e2e-backup"
  pkill -9 -f "target/debug/grpc-server" 2>/dev/null
  pkill -f deja-local/mock_connector.py 2>/dev/null
}
trap restore EXIT
cp "$SP" "$SP.e2e-backup"

count_topic() {
  docker exec deja-kafka /opt/kafka/bin/kafka-console-consumer.sh \
    --bootstrap-server localhost:9092 --topic $TOPIC \
    --from-beginning --timeout-ms 4000 2>/dev/null | wc -l | tr -d ' '
}

boot() { # $1=environment  extra env via E2E_EXTRA
  pkill -9 -f "target/debug/grpc-server" 2>/dev/null; sleep 1
  ( cd "$ROOT" && RUST_MIN_STACK=33554432 \
    CS__DEJA__MODE=record \
    CS__DEJA__RECORDING__KAFKA__BROKERS=localhost:9092 \
    CS__COMMON__ENVIRONMENT="$1" \
    CS__TEST__ENABLED=true \
    CS__TEST__MOCK_SERVER_URL=http://localhost:3000/mock \
    nohup env ${E2E_EXTRA:-} "$BIN" > $LOG 2>&1 < /dev/null & )
  for _ in $(seq 1 30); do
    grep -q "deja runtime hook installed" $LOG 2>/dev/null && return 0
    sleep 1
  done
  echo "BOOT FAILED"; tail -5 $LOG; return 1
}

fire() { # $1=rpc  $2=request-id  $3=payload-file-or-inline
  if [[ -f "$3" ]]; then
    grpcurl -max-time 20 -plaintext \
      -H 'x-connector: stripe' -H 'x-auth: header-key' -H 'x-api-key: sk_test_dummy_demo' \
      -H 'x-merchant-id: merchant_demo' -H 'x-tenant-id: default' \
      -H "x-request-id: $2" -H 'x-connector-request-reference-id: e2e_ref' \
      -d @ localhost:8000 "$1" < "$3" >/dev/null 2>&1
  else
    grpcurl -max-time 20 -plaintext \
      -H 'x-connector: stripe' -H 'x-auth: header-key' -H 'x-api-key: sk_test_dummy_demo' \
      -H 'x-merchant-id: merchant_demo' -H 'x-tenant-id: default' \
      -H "x-request-id: $2" -H 'x-connector-request-reference-id: e2e_ref' \
      -d "$3" localhost:8000 "$1" >/dev/null 2>&1
  fi
  sleep 2  # let the writer batch flush (flush_interval)
}

check() { # $1=case-name  $2=before  $3=expected: grow|same
  local after; after=$(count_topic)
  local verdict="FAIL"
  if [[ "$3" == grow && "$after" -gt "$2" ]]; then verdict="PASS"; fi
  if [[ "$3" == same && "$after" -eq "$2" ]]; then verdict="PASS"; fi
  [[ $verdict == PASS ]] && PASS=$((PASS+1)) || FAIL=$((FAIL+1))
  echo "[$verdict] $1 (topic: $2 -> $after, expected $3)"
}

AUTH=$HERE/authorize.json

echo "== infra =="
docker info >/dev/null 2>&1 || { echo "docker daemon is not running - start OrbStack/Docker first"; exit 2; }
docker compose -f "$HERE/docker-compose.yml" up -d >/dev/null 2>&1
until docker exec deja-kafka /opt/kafka/bin/kafka-topics.sh --bootstrap-server localhost:9092 --list >/dev/null 2>&1; do sleep 1; done
pkill -f deja-local/mock_connector.py 2>/dev/null; sleep 1
nohup python3 "$HERE/mock_connector.py" > /tmp/deja-mock.log 2>&1 < /dev/null &
sleep 1
echo "kafka + mock ready"

# ---- Case 1: development wholesale (shipped file) ----
cp "$SP.e2e-backup" "$SP"
boot development || exit 1
B=$(count_topic); fire types.PaymentService/Authorize e2e-dev-1 "$AUTH"
check "1 development records wholesale" "$B" grow

# ---- Case 1b: health/reflection never recorded ----
B=$(count_topic)
grpcurl -plaintext localhost:8000 grpc.health.v1.Health/Check >/dev/null 2>&1; sleep 2
check "1b health check not recorded" "$B" same

# ---- Case 2: production dark by default (shipped file) ----
boot production || exit 1
B=$(count_topic); fire types.PaymentService/Authorize e2e-prod-dark-1 "$AUTH"
check "2 production dark by default" "$B" same

# ---- Case 3: production + targeted rpc_method override ----
cp "$SP.e2e-backup" "$SP"
cat >> "$SP" <<'EOF'

[[overrides]]
_context_ = { environment = "production", rpc_method = "/types.PaymentService/Authorize" }
deja_record = true
EOF
boot production || exit 1
B=$(count_topic); fire types.PaymentService/Authorize e2e-prod-rpc-1 "$AUTH"
check "3a targeted rpc records" "$B" grow
B=$(count_topic); fire types.PaymentService/Capture e2e-prod-rpc-2 '{}'
check "3b untargeted rpc stays dark" "$B" same

# ---- Case 4: production + rpc_service class override ----
cp "$SP.e2e-backup" "$SP"
cat >> "$SP" <<'EOF'

[[overrides]]
_context_ = { environment = "production", rpc_service = "payment" }
deja_record = true
EOF
boot production || exit 1
B=$(count_topic); fire types.PaymentService/Authorize e2e-cls-1 "$AUTH"
check "4a payment-class rpc records (Authorize)" "$B" grow
B=$(count_topic); fire types.PaymentService/Capture e2e-cls-2 '{}'
check "4b payment-class rpc records (Capture)" "$B" grow
B=$(count_topic); fire types.RefundService/Refund e2e-cls-3 '{}'
check "4c other class stays dark (Refund)" "$B" same

# ---- Case 5: percentage 50, deterministic ids ----
# FNV-1a buckets (computed offline): pick one id below and one at/above 50.
LOW_ID=$(python3 - <<'PY'
def bucket(s):
    h = 0xcbf29ce484222325
    for b in s.encode():
        h ^= b; h = (h * 0x100000001b3) % (1 << 64)
    return h % 100
low = next(f"e2e-pct-{i}" for i in range(999) if bucket(f"e2e-pct-{i}") < 50)
print(low)
PY
)
HIGH_ID=$(python3 - <<'PY'
def bucket(s):
    h = 0xcbf29ce484222325
    for b in s.encode():
        h ^= b; h = (h * 0x100000001b3) % (1 << 64)
    return h % 100
high = next(f"e2e-pct-{i}" for i in range(999) if bucket(f"e2e-pct-{i}") >= 50)
print(high)
PY
)
cp "$SP.e2e-backup" "$SP"
cat >> "$SP" <<'EOF'

[[overrides]]
_context_ = { environment = "production", rpc_service = "payment" }
deja_record_percent = 50
EOF
boot production || exit 1
B=$(count_topic); fire types.PaymentService/Authorize "$LOW_ID" "$AUTH"
check "5a percent 50: low-bucket id ($LOW_ID) records" "$B" grow
B=$(count_topic); fire types.PaymentService/Authorize "$HIGH_ID" "$AUTH"
check "5b percent 50: high-bucket id ($HIGH_ID) skips" "$B" same

# ---- Case 6: record mode, superposition file missing => fail-closed dark ----
mv "$SP" "$SP.hidden"
boot production || exit 1
NOSOURCE=$(grep -c "no sampling source" $LOG || true)
B=$(count_topic); fire types.PaymentService/Authorize e2e-nosrc-1 "$AUTH"
check "6a no-source fail-closed stays dark" "$B" same
[[ "$NOSOURCE" -ge 1 ]] && { echo "[PASS] 6b no-source error logged"; PASS=$((PASS+1)); } \
  || { echo "[FAIL] 6b no-source error NOT logged"; FAIL=$((FAIL+1)); }

# ---- Case 6c: same, fail_closed=false => records ----
E2E_EXTRA="CS__DEJA__SAMPLER__FAIL_CLOSED=false" boot production || exit 1
B=$(count_topic); fire types.PaymentService/Authorize e2e-nosrc-2 "$AUTH"
check "6c no-source fail-open records" "$B" grow
mv "$SP.hidden" "$SP"

echo
echo "== RESULT: $PASS passed, $FAIL failed =="
exit $FAIL
