#!/usr/bin/env bash
# End-to-end local déjà recording demo:
#   infra (Kafka → Vector → MinIO) → UCS in record mode → one real Authorize
#   → show the tape from the Kafka topic AND the MinIO tape object.
#
# Usage:
#   scripts/deja-local/record-demo.sh            # assumes the deja build exists
#   scripts/deja-local/record-demo.sh --build    # cargo build --features deja first
#
# Requires: docker (compose), grpcurl, python3, cargo.
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
BIN="$ROOT/target/debug/grpc-server"
CORR="deja-demo-$(date +%s)"

# Run an mc command against the rig's MinIO (the mc image has no awk/sed — keep it mc-native).
mc_run() {
  docker run --rm --network deja-local_default --entrypoint /bin/sh minio/mc:latest -c \
    "mc alias set local http://minio:9000 minioadmin minioadmin >/dev/null && $1"
}

if [[ "${1:-}" == "--build" ]]; then
  (cd "$ROOT" && cargo build -p grpc-server --features deja)
fi
[[ -x "$BIN" ]] || { echo "missing $BIN — run with --build"; exit 1; }

echo "==> [1/6] infra up (kafka, minio, vector)"
# NB: no `--wait` — the one-shot minio-setup service exits by design and --wait would
# wait on it forever. Kafka readiness is probed explicitly below.
docker compose -f "$HERE/docker-compose.yml" up -d
until docker exec deja-kafka /opt/kafka/bin/kafka-topics.sh --bootstrap-server localhost:9092 --list >/dev/null 2>&1; do sleep 1; done
echo "    kafka ready"

echo "==> [2/6] mock connector on :3000"
pkill -f deja-local/mock_connector.py 2>/dev/null || true
nohup python3 "$HERE/mock_connector.py" > /tmp/deja-mock.log 2>&1 < /dev/null &
disown
sleep 1

echo "==> [3/6] UCS in record mode on :8000"
pkill -9 -f "target/debug/grpc-server" 2>/dev/null || true
sleep 1
(
  cd "$ROOT"
  CS__DEJA__MODE=record \
  CS__DEJA__RECORDING__KAFKA__BROKERS=localhost:9092 \
  CS__TEST__ENABLED=true \
  CS__TEST__MOCK_SERVER_URL=http://localhost:3000/mock \
  nohup "$BIN" > /tmp/deja-server.log 2>&1 < /dev/null &
  disown
)
# Poll (not a fixed sleep) — the tracing pipeline can flush the boot line late.
BOOT_OK=""
for _ in $(seq 1 30); do
  grep -q "deja runtime hook installed" /tmp/deja-server.log 2>/dev/null && { BOOT_OK=1; break; }
  sleep 1
done
[[ -n "$BOOT_OK" ]] && echo "    hook installed (mode=record)" \
  || { echo "    BOOT FAILED — see /tmp/deja-server.log"; exit 1; }

# Baseline BEFORE firing: step 6 polls for the object count to grow past this, so the
# script works for any Vector batch.timeout_secs (a flush during steps 4-5 still counts).
MINIO_BASELINE=$(mc_run "mc ls -r local/deja-tapes/" 2>/dev/null | wc -l | tr -d ' ')

echo "==> [4/6] firing types.PaymentService/Authorize (correlation: $CORR)"
grpcurl -max-time 20 -plaintext \
  -H 'x-connector: stripe' -H 'x-auth: header-key' -H 'x-api-key: sk_test_dummy_demo' \
  -H 'x-merchant-id: merchant_demo' -H 'x-tenant-id: default' \
  -H "x-request-id: $CORR" -H 'x-connector-request-reference-id: deja_demo_ref' \
  -d @ localhost:8000 types.PaymentService/Authorize < "$HERE/authorize.json" \
  >/dev/null 2>&1 || true   # business error is expected (mock response isn't real Stripe)
echo "    sent (handler error expected — the recording is the point)"

echo "==> [5/6] tape via Kafka topic"
docker exec deja-kafka /opt/kafka/bin/kafka-console-consumer.sh \
  --bootstrap-server localhost:9092 --topic ucs-deja-recording-events \
  --from-beginning --timeout-ms 6000 2>/dev/null \
  | python3 -c "
import json, sys
for line in sys.stdin:
    line = line.strip()
    if not line: continue
    e = json.loads(line)
    ev = e.get('event') or {}
    print(f\"  {e.get('artifact_type','?'):22} boundary={ev.get('boundary','-'):16} corr={ev.get('correlation_id') or '-'}\")"

echo "==> [6/6] tape objects in MinIO (polling until Vector's next flush lands)"
# Works for any batch.timeout_secs: wait until the object count grows past the pre-fire
# baseline, bounded by DEJA_MINIO_WAIT_SECS (default 90 — cover a 60s flush with margin).
WAIT="${DEJA_MINIO_WAIT_SECS:-90}"
DEADLINE=$((SECONDS + WAIT))
while :; do
  COUNT=$(mc_run "mc ls -r local/deja-tapes/" 2>/dev/null | wc -l | tr -d ' ')
  if [ "${COUNT:-0}" -gt "${MINIO_BASELINE:-0}" ]; then
    echo "    new tape object landed (${MINIO_BASELINE} -> ${COUNT} objects)"
    break
  fi
  if [ "$SECONDS" -ge "$DEADLINE" ]; then
    echo "    no new object after ${WAIT}s — Vector's batch window may be longer; the"
    echo "    events are safely in Kafka (step 5) and will ship on the next flush."
    break
  fi
  sleep 5
done
mc_run "mc ls -r local/deja-tapes/ && echo '--- first lines of newest object ---' && mc cat \$(mc find local/deja-tapes/ --name '*.jsonl' | tail -1) | head -2 | cut -c1-200"

echo
echo "done. server log: /tmp/deja-server.log · minio console: http://localhost:9001 (minioadmin/minioadmin)"
echo "stop everything:  pkill -f grpc-server; pkill -f mock_connector; docker compose -f $HERE/docker-compose.yml down"
