#!/usr/bin/env bash
# Replay a recorded UCS tape with the MODIFIED deja working tree (~/deja):
# role-based ingress (no relabel shim), the kernel's gRPC drive path
# (h2 + descriptor-set diff), the role-aware renderer/scorer.
#
#   scripts/deja-local/replay-dev.sh              # newest recorded correlation
#   scripts/deja-local/replay-dev.sh <corr-id>    # a specific one
#
# Works on BOTH tape shapes and auto-detects which:
#   grpc_incoming  -> boots prism in gRPC mode, kernel drives over HTTP/2
#   http_incoming  -> boots prism in HTTP mode, kernel drives HTTP/1.1
#
# Prereqs: a recording on the Kafka topic (record-demo.sh for gRPC,
# http-demo.sh for HTTP), target/debug/grpc-server built with --features deja,
# the deja checkout at ~/deja (override: DEJA_DEV_REPO).
#
# Legacy-tape shim, gRPC only: tapes recorded SINCE the deja pin bump carry
# `role: "ingress"` and `response.grpc_status` natively, making the staging
# injection below a no-op. For tapes recorded at the OLD pin it fills them in:
# role="ingress", and grpc_status only when absent (default 13 — every
# mock-connector authorize in this rig fails Internal; for a passing flow set
# DEJA_DEV_ASSUME_GRPC_STATUS=0).
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
BIN="$ROOT/target/debug/grpc-server"
DEJA_DEV_REPO="${DEJA_DEV_REPO:-$HOME/deja}"
BRIDGE_DIR=/tmp/deja-bridge-dev
BRIDGE="$BRIDGE_DIR/target/release/deja-bridge-dev"
KERNEL="$DEJA_DEV_REPO/target/release/deja-kernel"
# The dashboard's state root (HARNESS_STATE_DIR of the local orchestrator) —
# writing run artifacts here makes every run's detail page work in the UI.
STATE=/tmp/deja-ucs-state
RUN_ID="ucs-dev-$(date +%s)"
ASSUME_GRPC_STATUS="${DEJA_DEV_ASSUME_GRPC_STATUS:-13}"

[[ -x "$BIN" ]] || { echo "missing $BIN — build with: cargo build -p grpc-server --features deja"; exit 1; }
[[ -d "$DEJA_DEV_REPO/.git" ]] || { echo "no deja checkout at $DEJA_DEV_REPO (set DEJA_DEV_REPO)"; exit 1; }

echo "==> [1/6] build the MODIFIED deja-kernel + dev bridge (against $DEJA_DEV_REPO)"
(cd "$DEJA_DEV_REPO" && cargo build --release -p deja-kernel 2>&1 | tail -1 | sed 's/^/    /')
if [[ ! -x "$BRIDGE" || "$BRIDGE_DIR/src/main.rs" -nt "$BRIDGE" || "$0" -nt "$BRIDGE" ]]; then
  mkdir -p "$BRIDGE_DIR/src"
  cat > "$BRIDGE_DIR/Cargo.toml" <<EOF
[package]
name = "deja-bridge-dev"
version = "0.1.0"
edition = "2021"
[dependencies]
deja-orchestrator = { path = "$DEJA_DEV_REPO/crates/deja-orchestrator" }
serde_json = "1"
[workspace]
EOF
  cat > "$BRIDGE_DIR/src/main.rs" <<'EOF'
//! Dev bridge: the MODIFIED deja's real renderer + scorer over a UCS tape.
use deja_orchestrator::{
    lookup::render_lookup_table,
    scope::{RunScope, ScopedRecording, TapeSlot},
    write_json, CandidateSpec, HarnessRoot, Run, RunMode, RunSpec, RunStatus,
};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("prepare") => {
            let (rec_file, rec_id, root, run_id) = (&args[2], &args[3], &args[4], &args[5]);
            let root = HarnessRoot::new(root).expect("state root");
            let dst = TapeSlot::for_write(&root, rec_id);
            std::fs::create_dir_all(dst.parent().expect("parent")).expect("mkdir");
            std::fs::copy(rec_file, &dst).expect("copy recording");
            let recording = ScopedRecording::open(&root, rec_id, RunScope::entire_session())
                .expect("open scoped recording");
            let table = render_lookup_table(&recording, rec_id, 1).expect("render lookup table");
            eprintln!("rendered {} lookup entries", table.entries.len());
            write_json(&root.lookup_table_path(run_id), &table).expect("write table");
            let run = Run {
                run_id: run_id.clone(),
                spec: RunSpec {
                    mode: RunMode::Replay,
                    system_under_test: Some("prism".to_owned()),
                    candidate_spec: CandidateSpec::LocalPath {
                        binary_or_source: "ucs-grpc-server".into(),
                    },
                    candidate_repo: None,
                    recording_id: Some(rec_id.clone()),
                    s3_source: None,
                    correlation_filter: None,
                    workload: serde_json::Value::Null,
                    // Prism's instrumentation contract: every recorded ucs::*/
                    // connector::* span must replay, with equal field values.
                    scored_span_namespaces: vec!["ucs::".to_owned(), "connector::".to_owned()],
                },
                status: RunStatus::Completed,
                recording_id: Some(rec_id.clone()),
                candidate_image: None,
                failure_reason: None,
                stage: None,
                step: 0,
                steps_total: 0,
                stage_updated_ms: 0,
            };
            write_json(&root.run_path(run_id), &run).expect("write run record");
            // Lay down record_graph.jsonl the way the in-pod runner does —
            // without it every dev run scores flat (missing_forest) and the
            // graph tier + span-shape check never engage.
            match deja_orchestrator::lifecycle::extract_record_graph(&root, &run, rec_id) {
                Ok(Some(n)) => eprintln!("record graph: {n} node(s)"),
                Ok(None) => eprintln!("record graph: unavailable (reason left beside the run)"),
                Err(e) => eprintln!("record graph: FAILED — {e}"),
            }
            println!("{}", root.lookup_table_path(run_id).display());
        }
        Some("score") => {
            let root = HarnessRoot::new(&args[2]).expect("state root");
            let card = deja_orchestrator::divergence::detect_and_score(&root, &args[3])
                .expect("detect_and_score");
            println!("{}", serde_json::to_string_pretty(&card).expect("json"));
        }
        _ => {
            eprintln!("usage: deja-bridge-dev prepare <rec.jsonl> <rec-id> <root> <run-id> | score <root> <run-id>");
            std::process::exit(2);
        }
    }
}
EOF
  (cd "$BRIDGE_DIR" && cargo build --release 2>&1 | tail -1 | sed 's/^/    /')
fi

echo "==> [2/6] pull the tape, pick the correlation"
docker exec deja-kafka /opt/kafka/bin/kafka-console-consumer.sh \
  --bootstrap-server localhost:9092 --topic ucs-deja-recording-events \
  --from-beginning --timeout-ms 6000 2>/dev/null > /tmp/tape-dev.jsonl \
  || { echo "    kafka not reachable — is the rig up? (docker compose -f $HERE/docker-compose.yml up -d)"; exit 1; }
if [ $# -ge 1 ]; then CORR="$1"; else
  CORR=$(python3 -c "
import json
last = ''
for line in open('/tmp/tape-dev.jsonl'):
    line = line.strip()
    if not line: continue
    e = json.loads(line)
    if e.get('artifact_type') == 'deja_artifact_record' and e.get('correlation_id'):
        last = e['correlation_id']
print(last)")
fi
[ -n "$CORR" ] || { echo "    no recorded correlations — run record-demo.sh or http-demo.sh first"; exit 1; }
REC_ID="rec-dev-$CORR"
echo "    correlation: $CORR"

echo "==> [3/6] stage (verbatim for HTTP; role+grpc_status shim for gRPC — see header)"
MODE=$(CORR="$CORR" ASSUME="$ASSUME_GRPC_STATUS" python3 -c "
import json, os, sys
corr, assume = os.environ['CORR'], int(os.environ['ASSUME'])
out = open('/tmp/ucs-dev-rec.jsonl', 'w')
n, kinds = {}, set()
for line in open('/tmp/tape-dev.jsonl'):
    line = line.strip()
    if not line: continue
    env = json.loads(line)
    # Graph nodes ride the tape too: without them the record side has no
    # execution graph, every run scores flat (missing_forest), and the
    # span-shape check skips itself.
    if env.get('artifact_type') == 'deja_graph_node':
        node = env.get('node') or {}
        if node.get('correlation_id') != corr: continue
        out.write(json.dumps({'record_kind': 'graph_node', **node}) + '\n')
        n['graph_node'] = n.get('graph_node', 0) + 1
        continue
    if env.get('artifact_type') != 'deja_artifact_record': continue
    if env.get('correlation_id') != corr: continue
    e = dict(env['event'])
    if e.get('boundary') == 'grpc_incoming':
        kinds.add('grpc')
        e['role'] = 'ingress'
        resp = e.get('response') if isinstance(e.get('response'), dict) else {}
        if 'grpc_status' not in resp:
            resp['grpc_status'] = assume
            e['response'] = resp
    if e.get('boundary') == 'http_incoming':
        kinds.add('http')
    n[e.get('boundary')] = n.get(e.get('boundary'), 0) + 1
    out.write(json.dumps({'record_kind': 'boundary_event', **e}) + '\n')
out.close()
print('    staged: ' + ', '.join(f'{b}×{c}' for b, c in sorted(n.items())), file=sys.stderr)
if not kinds:
    print(f'    ERROR: no ingress event for {corr}', file=sys.stderr); sys.exit(1)
print(kinds.pop())")
echo "    ingress kind: $MODE"

echo "==> [4/6] MODIFIED renderer builds the table (no relabel shim)"
TABLE=$("$BRIDGE" prepare /tmp/ucs-dev-rec.jsonl "$REC_ID" "$STATE" "$RUN_ID")
echo "    table: $TABLE"

echo "==> [5/6] boot prism replay ($MODE mode, connector DEAD) + drive with MODIFIED kernel"
pkill -9 -f "target/debug/grpc-server" 2>/dev/null || true
pkill -f deja-local/mock_connector.py 2>/dev/null || true
sleep 1
SERVER_TYPE=""; [ "$MODE" = "http" ] && SERVER_TYPE="http" || SERVER_TYPE="grpc"
(
  cd "$ROOT"
  CS__COMMON__ENVIRONMENT=development \
  CS__SERVER__TYPE="$SERVER_TYPE" \
  RUST_MIN_STACK=33554432 \
  CS__DEJA__MODE=replay \
  CS__DEJA__REPLAY__SOURCE="$TABLE" \
  CS__DEJA__REPLAY__OBSERVED_SINK="$STATE/observed/$RUN_ID.jsonl" \
  CS__TEST__ENABLED=true \
  CS__TEST__MOCK_SERVER_URL=http://localhost:3000/mock \
  nohup "$BIN" > /tmp/deja-dev-replay.log 2>&1 < /dev/null &
  disown
) || true
BOOT_OK=""
for _ in $(seq 1 30); do
  grep -q "deja runtime hook installed" /tmp/deja-dev-replay.log 2>/dev/null && { BOOT_OK=1; break; }
  sleep 1
done
[[ -n "$BOOT_OK" ]] || { echo "    BOOT FAILED — see /tmp/deja-dev-replay.log"; exit 1; }
mkdir -p "$STATE/http-diffs" "$STATE/observed"
: > "$STATE/http-diffs/$RUN_ID.jsonl"
# Descriptor set: prism's own build artifact — lets the kernel re-encode
# decoded gRPC requests and diff responses field-by-field.
FDS=$(ls -t "$ROOT"/target/debug/build/grpc-api-types-*/out/connector_service_descriptor.bin 2>/dev/null | head -1 || true)
[ -n "$FDS" ] && echo "    descriptor set: $FDS" || echo "    (no descriptor set found — gRPC diffs will be byte-exact)"
KERNEL_ENV=(
  KERNEL_RECORDING_PATH=/tmp/ucs-dev-rec.jsonl
  KERNEL_TARGET_HOST=127.0.0.1
  KERNEL_TARGET_PORT=8000
  KERNEL_HTTP_DIFF_SINK="$STATE/http-diffs/$RUN_ID.jsonl"
)
[ -n "$FDS" ] && KERNEL_ENV+=(KERNEL_DESCRIPTOR_SET="$FDS")
env "${KERNEL_ENV[@]}" "$KERNEL" 2>&1 | sed 's/^/    /'
pkill -f "target/debug/grpc-server"; sleep 2

echo "==> [6/6] MODIFIED scorer"
"$BRIDGE" score "$STATE" "$RUN_ID" | python3 -c "
import json, sys
card = json.load(sys.stdin)
v, s = card.get('verdict', {}), card.get('summary', {})
print('    verdict.pass:', v.get('pass'))
print('    reason:      ', v.get('reason'))
print('    requests reproduced:', f\"{s.get('matched_correlations')}/{s.get('total_correlations')}\")
print('    resolved_by_rank:', s.get('resolved_by_rank'))"

# Register in the dashboard (list is pg-backed; the detail page reads the file
# artifacts this script just wrote into the dashboard's state root).
if docker ps --format '{{.Names}}' 2>/dev/null | grep -q '^deja-orchestrator-pg-1$'; then
  CORR="$CORR" REC_ID="$REC_ID" RUN_ID="$RUN_ID" STATE="$STATE" python3 -c "
import json, os
corr, rec, run_id, state = os.environ['CORR'], os.environ['REC_ID'], os.environ['RUN_ID'], os.environ['STATE']
card = json.load(open(f'{state}/runs/{run_id}.scorecard.json'))
run = json.load(open(f'{state}/runs/{run_id}.json'))
def q(v): return \"'\" + json.dumps(v).replace(\"'\", \"''\") + \"'\"
verdict = \"'pass'\" if card.get('verdict', {}).get('pass') else \"'fail'\"
params = {'mode': 'replay', 'candidate_spec': run['spec']['candidate_spec'],
          'system_under_test': 'prism', 'recording_id': rec,
          'correlation_filter': [corr], 'workload': None}
print(f'''INSERT INTO replay_runs (run_id, mode, recording_id, candidate, params, expectation, created_by, state, started_at, verdict, scorecard)
VALUES ('{run_id}', 'replay', '{rec}', {q(run['spec']['candidate_spec'])}, {q(params)}, NULL, 'replay-dev', 'completed', now(), {verdict}, {q(card)})
ON CONFLICT (run_id) DO UPDATE SET verdict=EXCLUDED.verdict, scorecard=EXCLUDED.scorecard;
INSERT INTO recordings (recording_id, source_path, event_count, correlation_count, byte_size, manifest, created_by)
VALUES ('{rec}', 'kafka topic ucs-deja-recording-events (replay-dev)', 0, 1, 0, NULL, 'replay-dev')
ON CONFLICT (recording_id) DO NOTHING;''')" \
  | docker exec -i deja-orchestrator-pg-1 psql -q -U deja -d deja >/dev/null 2>&1 \
    && echo "    dashboard: http://127.0.0.1:8070/r/$RUN_ID" \
    || echo "    (dashboard registration failed — run still fully scored on disk)"
else
  echo "    (dashboard pg not running — skipping registration)"
fi

echo
echo "artifacts:"
echo "  kernel diffs:  cat $STATE/http-diffs/$RUN_ID.jsonl | jq ."
echo "  observed:      cat $STATE/observed/$RUN_ID.jsonl | jq . | head"
echo "  scorecard:     cat $STATE/runs/$RUN_ID.scorecard.json | jq .verdict"
echo "  replay log:    /tmp/deja-dev-replay.log"
