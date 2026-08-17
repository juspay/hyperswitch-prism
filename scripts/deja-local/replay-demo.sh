#!/usr/bin/env bash
# Replay a recorded correlation "as it should be": deja-orchestrator's REAL renderer
# builds the lookup table, UCS boots via the production LookupTableHook with an
# observed-call sink, the request is re-driven with the connector DEAD, and
# deja-orchestrator's REAL scorer produces the verdict + an interactive HTML timeline.
#
#   scripts/deja-local/replay-demo.sh [correlation-id]
#
# Prereqs: record-demo.sh ran at least once (tape on the Kafka topic); a deja checkout
# (DEJA_REPO, default ~/deja); target/debug/grpc-server built with --features deja.
#
# Known upstream gaps this script shims (tracked as deja D2/D3 in the RFC):
#   D3 — the renderer only recognizes `http_incoming` as the ingress root, so our
#        `grpc_incoming` events are relabeled before rendering.
#   D2 — deja-kernel re-drives HTTP only, so the re-drive here is grpcurl.
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
BIN="$ROOT/target/debug/grpc-server"
DEJA_REPO="${DEJA_REPO:-$HOME/deja}"
# The exact rev UCS records with (workspace Cargo pin) — renderer/scorer must match
# the event schema, so the bridge builds against a worktree at this rev.
DEJA_PIN="2c3a795ef4d8d2a5eebc47bbe7134984b75be6b3"
PIN_DIR=/tmp/deja-pin
BRIDGE_DIR=/tmp/deja-bridge
STATE=/tmp/deja-ucs-state
RUN_ID=ucs-replay-run-1
REC_ID=ucs-rec-1

[[ -x "$BIN" ]] || { echo "missing $BIN — build with: cargo build -p grpc-server --features deja"; exit 1; }
[[ -d "$DEJA_REPO/.git" ]] || { echo "no deja checkout at $DEJA_REPO (set DEJA_REPO)"; exit 1; }

echo "==> [1/7] deja worktree @ pinned rev + bridge bin"
[[ -d "$PIN_DIR" ]] || git -C "$DEJA_REPO" worktree add "$PIN_DIR" "$DEJA_PIN"
if [[ ! -x "$BRIDGE_DIR/target/release/deja-bridge" ]]; then
  mkdir -p "$BRIDGE_DIR/src"
  cat > "$BRIDGE_DIR/Cargo.toml" <<EOF
[package]
name = "deja-bridge"
version = "0.1.0"
edition = "2021"
[dependencies]
deja-orchestrator = { path = "$PIN_DIR/crates/deja-orchestrator" }
serde_json = "1"
[workspace]
EOF
  cat > "$BRIDGE_DIR/src/main.rs" <<'EOF'
//! Bridge: deja-orchestrator's REAL renderer + scorer over a UCS tape (mirrors
//! lifecycle/mod.rs replay steps: render -> write_json table -> detect_and_score).
use deja_orchestrator::{
    lookup::render_lookup_table, write_json, CandidateSpec, HarnessRoot, Run, RunMode, RunSpec,
    RunStatus,
};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("prepare") => {
            let (rec_file, rec_id, root, run_id) = (&args[2], &args[3], &args[4], &args[5]);
            let root = HarnessRoot::new(root).expect("state root");
            let dst = root.recording_events_path(rec_id);
            std::fs::create_dir_all(dst.parent().expect("parent")).expect("mkdir");
            std::fs::copy(rec_file, &dst).expect("copy recording");
            let table =
                render_lookup_table(dst.as_path(), rec_id, 1).expect("render lookup table");
            eprintln!("rendered {} lookup entries", table.entries.len());
            write_json(&root.lookup_table_path(run_id), &table).expect("write table");
            let run = Run {
                run_id: run_id.clone(),
                spec: RunSpec {
                    mode: RunMode::Replay,
                    candidate_spec: CandidateSpec::LocalPath {
                        binary_or_source: "ucs-grpc-server".into(),
                    },
                    candidate_repo: None,
                    recording_id: Some(rec_id.clone()),
                    s3_source: None,
                    correlation_filter: None,
                    workload: serde_json::Value::Null,
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
            println!("{}", root.lookup_table_path(run_id).display());
        }
        Some("score") => {
            let root = HarnessRoot::new(&args[2]).expect("state root");
            let card = deja_orchestrator::divergence::detect_and_score(&root, &args[3])
                .expect("detect_and_score");
            println!("{}", serde_json::to_string_pretty(&card).expect("json"));
        }
        _ => {
            eprintln!("usage: deja-bridge prepare <rec.jsonl> <rec-id> <root> <run-id> | score <root> <run-id>");
            std::process::exit(2);
        }
    }
}
EOF
  (cd "$BRIDGE_DIR" && cargo build --release)
fi
BRIDGE="$BRIDGE_DIR/target/release/deja-bridge"

echo "==> [2/7] pull tape from Kafka, pick a correlation"
docker exec deja-kafka /opt/kafka/bin/kafka-console-consumer.sh \
  --bootstrap-server localhost:9092 --topic ucs-deja-recording-events \
  --from-beginning --timeout-ms 6000 2>/dev/null > /tmp/full_tape.jsonl
CORR="${1:-$(python3 -c "
import json
last = ''
for line in open('/tmp/full_tape.jsonl'):
    e = json.loads(line)
    if e.get('artifact_type') == 'deja_artifact_record' and e.get('correlation_id'):
        last = e['correlation_id']
print(last)")}"
[[ -n "$CORR" ]] || { echo "no recorded correlations on the topic — run record-demo.sh first"; exit 1; }
echo "    correlation: $CORR"

echo "==> [3/7] stage events (D3 shim: grpc_incoming -> http_incoming) + render lookup table"
rm -rf "$STATE"
python3 -c "
import json
out = open('/tmp/ucs-rec.jsonl', 'w')
kept = 0
for line in open('/tmp/full_tape.jsonl'):
    env = json.loads(line)
    if env.get('artifact_type') != 'deja_artifact_record': continue
    if env.get('correlation_id') != '$CORR': continue
    e = dict(env['event'])
    if e.get('boundary') == 'grpc_incoming':
        e['boundary'] = 'http_incoming'
    out.write(json.dumps({'record_kind': 'boundary_event', **e}) + '\n'); kept += 1
out.close(); print(f'    staged {kept} events')"
TABLE=$("$BRIDGE" prepare /tmp/ucs-rec.jsonl "$REC_ID" "$STATE" "$RUN_ID")

echo "==> [4/7] boot UCS in replay mode (LookupTableHook) — connector NOT started"
pkill -9 -f "target/debug/grpc-server" 2>/dev/null || true
pkill -f deja-local/mock_connector.py 2>/dev/null || true
sleep 1
(
  cd "$ROOT"
  CS__COMMON__ENVIRONMENT=development \
  CS__DEJA__MODE=replay \
  CS__DEJA__REPLAY__SOURCE="$TABLE" \
  CS__DEJA__REPLAY__OBSERVED_SINK="$STATE/observed/$RUN_ID.jsonl" \
  CS__TEST__ENABLED=true \
  CS__TEST__MOCK_SERVER_URL=http://localhost:3000/mock \
  nohup "$BIN" > /tmp/deja-replay-server.log 2>&1 < /dev/null &
  disown
)
sleep 4
grep -q "deja runtime hook installed" /tmp/deja-replay-server.log \
  || { echo "    BOOT FAILED — see /tmp/deja-replay-server.log"; exit 1; }
echo "    replay hook installed from rendered table"

echo "==> [5/7] re-drive the recorded request (egress must come from tape)"
grpcurl -max-time 20 -plaintext \
  -H 'x-connector: stripe' -H 'x-auth: header-key' -H 'x-api-key: sk_test_dummy_demo' \
  -H 'x-merchant-id: merchant_demo' -H 'x-tenant-id: default' \
  -H "x-request-id: $CORR" -H 'x-connector-request-reference-id: deja_demo_ref' \
  -d @ localhost:8000 types.PaymentService/Authorize < "$HERE/authorize.json" \
  >/dev/null 2>&1 || true
pkill -f "target/debug/grpc-server"; sleep 2

echo "==> [6/7] score with deja-orchestrator's divergence scorer"
"$BRIDGE" score "$STATE" "$RUN_ID" | python3 -c "
import json, sys
card = json.load(sys.stdin)
v = card.get('verdict', {})
print('    verdict.pass:', v.get('pass'))
print('    reason:      ', v.get('reason'))
print('    resolved_by_rank:', card.get('summary', {}).get('resolved_by_rank'))"

echo "==> [7/7] interactive HTML timeline"
python3 "$DEJA_REPO/demo/visualize-replay.py" "$STATE" >/dev/null 2>&1 || true
echo
echo "see it:"
echo "  timeline:   open $STATE/replay-visualization.html"
echo "  scorecard:  cat $STATE/runs/$RUN_ID.scorecard.json | jq .verdict"
echo "  observed:   cat $STATE/observed/$RUN_ID.jsonl | jq ."
echo "  table:      jq . $TABLE | head"
