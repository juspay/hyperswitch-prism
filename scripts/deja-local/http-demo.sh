#!/usr/bin/env bash
# M1 — the unified-deployment proof: record prism in HTTP mode, replay it with the
# STOCK deja toolchain. Unlike replay-demo.sh (gRPC mode), this path needs ZERO shims:
#   - the tape's ingress events are `http_incoming` (deja's native ingress root), so
#     the renderer consumes them unrelabeled, and
#   - the re-drive is deja-kernel itself (the production driver), not a script loop.
# One prism deployment thus replays under the exact harness hyperswitch uses.
#
#   scripts/deja-local/http-demo.sh [--build]
#
# Prereqs: docker, curl, python3, cargo; replay-demo.sh ran once (it builds the shared
# deja-bridge binary and the /tmp/deja-pin worktree this script also uses).
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
BIN="$ROOT/target/debug/grpc-server"
DEJA_REPO="${DEJA_REPO:-$HOME/deja}"
DEJA_PIN="2c3a795ef4d8d2a5eebc47bbe7134984b75be6b3"
PIN_DIR=/tmp/deja-pin
BRIDGE=/tmp/deja-bridge/target/release/deja-bridge
KERNEL="$PIN_DIR/target/release/deja-kernel"
STATE=/tmp/deja-ucs-state
CORR="deja-http-$(date +%s)"
RUN_ID="ucs-http-replay-$(date +%s)"
REC_ID="rec-$CORR"

if [[ "${1:-}" == "--build" ]]; then
  (cd "$ROOT" && cargo build -p grpc-server --features deja)
fi
[[ -x "$BIN" ]] || { echo "missing $BIN — run with --build"; exit 1; }
[[ -x "$BRIDGE" ]] || { echo "missing $BRIDGE — run replay-demo.sh once first (it builds the bridge)"; exit 1; }

echo "==> [1/8] deja worktree @ pin + STOCK deja-kernel build"
[[ -d "$PIN_DIR" ]] || git -C "$DEJA_REPO" worktree add "$PIN_DIR" "$DEJA_PIN"
[[ -x "$KERNEL" ]] || (cd "$PIN_DIR" && cargo build --release -p deja-kernel)
echo "    kernel: $KERNEL (unmodified deja @ ${DEJA_PIN:0:12})"

echo "==> [2/8] infra up (kafka, minio, vector) + mock connector"
docker compose -f "$HERE/docker-compose.yml" up -d
until docker exec deja-kafka /opt/kafka/bin/kafka-topics.sh --bootstrap-server localhost:9092 --list >/dev/null 2>&1; do sleep 1; done
pkill -f deja-local/mock_connector.py 2>/dev/null || true
nohup python3 "$HERE/mock_connector.py" > /tmp/deja-mock.log 2>&1 < /dev/null &
disown
sleep 1

echo "==> [3/8] UCS in HTTP mode + record on :8000"
pkill -9 -f "target/debug/grpc-server" 2>/dev/null || true
sleep 1
(
  cd "$ROOT"
  CS__SERVER__TYPE=http \
  CS__DEJA__MODE=record \
  CS__DEJA__RECORDING__KAFKA__BROKERS=localhost:9092 \
  CS__TEST__ENABLED=true \
  CS__TEST__MOCK_SERVER_URL=http://localhost:3000/mock \
  nohup "$BIN" > /tmp/deja-http-server.log 2>&1 < /dev/null &
  disown
)
# Poll (not a fixed sleep) — the tracing pipeline can flush the boot line late.
BOOT_OK=""
for _ in $(seq 1 30); do
  grep -q "deja runtime hook installed" /tmp/deja-http-server.log 2>/dev/null && { BOOT_OK=1; break; }
  sleep 1
done
[[ -n "$BOOT_OK" ]] && echo "    hook installed (mode=record, server=http)" \
  || { echo "    BOOT FAILED — see /tmp/deja-http-server.log"; exit 1; }

echo "==> [4/8] firing POST /payments/authorize (correlation: $CORR)"
HTTP_STATUS=$(curl -s -o /tmp/deja-http-reply.json -w '%{http_code}' -X POST \
  -H 'content-type: application/json' \
  -H 'x-connector: stripe' -H 'x-auth: header-key' -H 'x-api-key: sk_test_dummy_demo' \
  -H 'x-merchant-id: merchant_demo' -H 'x-tenant-id: default' \
  -H "x-request-id: $CORR" -H 'x-connector-request-reference-id: deja_http_ref' \
  --data-binary @"$HERE/authorize-http.json" \
  http://localhost:8000/payments/authorize || echo 000)
echo "    HTTP $HTTP_STATUS · $(head -c 160 /tmp/deja-http-reply.json)"
[[ "$HTTP_STATUS" != "000" ]] || { echo "    request failed to send"; exit 1; }
sleep 2

echo "==> [5/8] pull tape, stage correlation VERBATIM (no relabel — already http_incoming)"
docker exec deja-kafka /opt/kafka/bin/kafka-console-consumer.sh \
  --bootstrap-server localhost:9092 --topic ucs-deja-recording-events \
  --from-beginning --timeout-ms 6000 2>/dev/null > /tmp/tape-kafka.jsonl || : > /tmp/tape-kafka.jsonl
CORR="$CORR" python3 -c "
import json, os, sys
corr = os.environ['CORR']
out = open('/tmp/ucs-http-rec.jsonl', 'w')
kept, boundaries = 0, {}
for line in open('/tmp/tape-kafka.jsonl'):
    line = line.strip()
    if not line: continue
    env = json.loads(line)
    if env.get('artifact_type') != 'deja_artifact_record': continue
    if env.get('correlation_id') != corr: continue
    e = dict(env['event'])
    boundaries[e.get('boundary')] = boundaries.get(e.get('boundary'), 0) + 1
    out.write(json.dumps({'record_kind': 'boundary_event', **e}) + '\n'); kept += 1
out.close()
if not boundaries.get('http_incoming'):
    print(f'    ERROR: no http_incoming event for {corr} — P1 layer did not record', file=sys.stderr)
    sys.exit(1)
print(f'    staged {kept} events: ' + ', '.join(f'{b}×{n}' for b, n in sorted(boundaries.items())))"

echo "==> [6/8] deja renderer builds the lookup table (stock — http_incoming is native)"
TABLE=$("$BRIDGE" prepare /tmp/ucs-http-rec.jsonl "$REC_ID" "$STATE" "$RUN_ID")
echo "    table: $TABLE"

echo "==> [7/8] boot UCS in HTTP mode + replay (connector DEAD), drive with STOCK deja-kernel"
pkill -9 -f "target/debug/grpc-server" 2>/dev/null || true
pkill -f deja-local/mock_connector.py 2>/dev/null || true
sleep 1
(
  cd "$ROOT"
  CS__COMMON__ENVIRONMENT=development \
  CS__SERVER__TYPE=http \
  CS__DEJA__MODE=replay \
  CS__DEJA__REPLAY__SOURCE="$TABLE" \
  CS__DEJA__REPLAY__OBSERVED_SINK="$STATE/observed/$RUN_ID.jsonl" \
  CS__TEST__ENABLED=true \
  CS__TEST__MOCK_SERVER_URL=http://localhost:3000/mock \
  nohup "$BIN" > /tmp/deja-http-replay.log 2>&1 < /dev/null &
  disown
)
BOOT_OK=""
for _ in $(seq 1 30); do
  grep -q "deja runtime hook installed" /tmp/deja-http-replay.log 2>/dev/null && { BOOT_OK=1; break; }
  sleep 1
done
[[ -n "$BOOT_OK" ]] || { echo "    BOOT FAILED — see /tmp/deja-http-replay.log"; exit 1; }
mkdir -p "$STATE/http-diffs" "$STATE/observed"
: > "$STATE/http-diffs/$RUN_ID.jsonl"
KERNEL_RECORDING_PATH=/tmp/ucs-http-rec.jsonl \
KERNEL_TARGET_HOST=127.0.0.1 \
KERNEL_TARGET_PORT=8000 \
KERNEL_HTTP_DIFF_SINK="$STATE/http-diffs/$RUN_ID.jsonl" \
${KERNEL_BODY_ALLOWLIST:+KERNEL_BODY_ALLOWLIST="$KERNEL_BODY_ALLOWLIST"} \
"$KERNEL" 2>&1 | sed 's/^/    /'
pkill -f "target/debug/grpc-server"; sleep 2

echo "==> [8/8] score with deja-orchestrator's divergence scorer"
"$BRIDGE" score "$STATE" "$RUN_ID" | python3 -c "
import json, sys
card = json.load(sys.stdin)
v, s = card.get('verdict', {}), card.get('summary', {})
print('    verdict.pass:', v.get('pass'))
print('    reason:      ', v.get('reason'))
print('    requests reproduced:', f\"{s.get('matched_correlations')}/{s.get('total_correlations')}\")
print('    resolved_by_rank:', s.get('resolved_by_rank'))"

# Register in the dashboard, if its pg is up (list is pg-backed; details read the files).
if docker ps --format '{{.Names}}' 2>/dev/null | grep -q '^deja-orchestrator-pg-1$'; then
  python3 -c "
import json
card = json.load(open('$STATE/runs/$RUN_ID.scorecard.json'))
run = json.load(open('$STATE/runs/$RUN_ID.json'))
def q(v): return \"'\" + json.dumps(v).replace(\"'\", \"''\") + \"'\"
verdict = \"'pass'\" if card.get('verdict', {}).get('pass') else \"'fail'\"
print(f'''INSERT INTO replay_runs (run_id, mode, recording_id, candidate, params, expectation, created_by, state, started_at, verdict, scorecard)
VALUES ('$RUN_ID', 'replay', '$REC_ID', {q(run['spec']['candidate_spec'])}, {q({'correlations': ['$CORR']})}, NULL, 'http-demo', 'completed', now(), {verdict}, {q(card)})
ON CONFLICT (run_id) DO UPDATE SET verdict=EXCLUDED.verdict, scorecard=EXCLUDED.scorecard;
INSERT INTO recordings (recording_id, source_path, event_count, correlation_count, byte_size, manifest, created_by)
VALUES ('$REC_ID', 'kafka topic ucs-deja-recording-events (http mode)', 0, 1, 0, NULL, 'http-demo')
ON CONFLICT (recording_id) DO NOTHING;''')" \
  | docker exec -i deja-orchestrator-pg-1 psql -q -U deja -d deja >/dev/null 2>&1 \
    && echo "    registered in dashboard: http://127.0.0.1:8070/runs/$RUN_ID" \
    || echo "    (dashboard registration failed — run still fully scored on disk)"
fi

echo
echo "done. record log: /tmp/deja-http-server.log · replay log: /tmp/deja-http-replay.log"
echo "kernel diffs: cat $STATE/http-diffs/$RUN_ID.jsonl | jq ."
