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
# Unique per invocation: every replay is its own run (and dashboard row); artifacts
# accumulate in $STATE so older runs stay browsable.
RUN_ID="ucs-replay-$(date +%s)"

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
grpc-api-types = { path = "$ROOT/crates/types-traits/grpc-api-types" }
prost-reflect = { version = "0.16.5", features = ["serde"] }
base64 = "0.21"
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
        // decode-response </types.Service/Method> <base64-of-grpc-frames>
        // Decodes a recorded response (gRPC-framed protobuf) to proto3-JSON using the
        // server's own FILE_DESCRIPTOR_SET — the descriptor-aware step the D2 driver owns.
        Some("decode-response") => {
            use base64::Engine as _;
            let rpc = &args[2];
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(&args[3])
                .expect("base64");
            let pool = prost_reflect::DescriptorPool::decode(grpc_api_types::FILE_DESCRIPTOR_SET)
                .expect("descriptor pool");
            let (svc, method_name) = rpc
                .trim_start_matches('/')
                .split_once('/')
                .expect("rpc path");
            let service = pool
                .services()
                .find(|s| s.full_name() == svc)
                .expect("service");
            let method = service
                .methods()
                .find(|m| m.name() == method_name)
                .expect("method");
            let payload: &[u8] = if bytes.len() >= 5 && bytes[0] == 0 {
                let len = u32::from_be_bytes([bytes[1], bytes[2], bytes[3], bytes[4]]) as usize;
                &bytes[5..5 + len]
            } else {
                &bytes[..]
            };
            let message = prost_reflect::DynamicMessage::decode(method.output(), payload)
                .expect("decode response message");
            println!("{}", serde_json::to_string(&message).expect("json"));
        }
        _ => {
            eprintln!("usage: deja-bridge prepare <rec.jsonl> <rec-id> <root> <run-id> | score <root> <run-id> | decode-response <rpc> <b64>");
            std::process::exit(2);
        }
    }
}
EOF
  (cd "$BRIDGE_DIR" && cargo build --release)
fi
BRIDGE="$BRIDGE_DIR/target/release/deja-bridge"

echo "==> [2/7] pull tape from Kafka, select correlations"
docker exec deja-kafka /opt/kafka/bin/kafka-console-consumer.sh \
  --bootstrap-server localhost:9092 --topic ucs-deja-recording-events \
  --from-beginning --timeout-ms 6000 2>/dev/null > /tmp/full_tape.jsonl
# Selection: --all = every correlation on the tape · <id> [<id>…] = those · none = newest.
if [[ "${1:-}" == "--all" ]]; then
  python3 -c "
import json
seen = []
for line in open('/tmp/full_tape.jsonl'):
    e = json.loads(line)
    if e.get('artifact_type') == 'deja_artifact_record' and e.get('correlation_id'):
        c = e['correlation_id']
        if c not in seen: seen.append(c)
print('\n'.join(seen))" > /tmp/ucs-corrs.txt
elif [ $# -ge 1 ]; then
  printf '%s\n' "$@" > /tmp/ucs-corrs.txt
else
  python3 -c "
import json
last = ''
for line in open('/tmp/full_tape.jsonl'):
    e = json.loads(line)
    if e.get('artifact_type') == 'deja_artifact_record' and e.get('correlation_id'):
        last = e['correlation_id']
print(last)" > /tmp/ucs-corrs.txt
fi
[ -s /tmp/ucs-corrs.txt ] || { echo "no recorded correlations on the topic — run record-demo.sh first"; exit 1; }
CORR_COUNT=$(wc -l < /tmp/ucs-corrs.txt | tr -d ' ')
echo "    $CORR_COUNT correlation(s): $(paste -sd' ' - < /tmp/ucs-corrs.txt | cut -c1-120)"

if [ "$CORR_COUNT" -gt 1 ]; then REC_ID="rec-${RUN_ID}"; else REC_ID="rec-$(head -1 /tmp/ucs-corrs.txt)"; fi
echo "==> [3/7] stage events (D3 shim: grpc_incoming -> http_incoming) + extract re-drive specs"
python3 -c "
import base64 as b64mod, json, sys
corrs = [c.strip() for c in open('/tmp/ucs-corrs.txt') if c.strip()]
corr_set = set(corrs)
out = open('/tmp/ucs-rec.jsonl', 'w')
specs = []
kept, skipped = 0, 0
for line in open('/tmp/full_tape.jsonl'):
    env = json.loads(line)
    if env.get('artifact_type') != 'deja_artifact_record': continue
    if env.get('correlation_id') not in corr_set: continue
    e = dict(env['event'])
    if e.get('boundary') == 'grpc_incoming':
        # One spec per recorded ATTEMPT: the re-drive replays every recorded request in
        # recorded order, so a correlation with k attempts is driven k times and all k
        # recorded egress calls are consumed (occurrence counters advance in lockstep).
        args = e.get('args') or e.get('request') or {}
        if isinstance(args, str): args = json.loads(args)
        decoded = ((args.get('request') or {}).get('decoded'))
        if decoded is None:
            print(f'    WARN: undecoded ingress for {env[\"correlation_id\"]} — skipping that attempt', file=sys.stderr)
            skipped += 1
        else:
            headers = [(n, v) for n, v in (args.get('metadata') or []) if n.startswith('x-')]
            res = e.get('result'); res = json.loads(res) if isinstance(res, str) else (res or {})
            raw = ((res.get('response_body') or {}).get('raw_bytes')) or []
            specs.append({'correlation_id': env['correlation_id'],
                          'global_sequence': int(e.get('global_sequence') or 0),
                          'rpc': (args.get('rpc') or '').lstrip('/'), 'headers': headers,
                          'body': decoded, 'is_error': bool(e.get('is_error')),
                          'request_sequence': int(e.get('request_sequence') or 0),
                          'recorded_response_b64': b64mod.b64encode(bytes(raw)).decode() if raw else None})
        e['boundary'] = 'http_incoming'
    out.write(json.dumps({'record_kind': 'boundary_event', **e}) + '\n'); kept += 1
out.close()
if not specs:
    print('    ERROR: no re-drivable grpc_incoming events in the selection', file=sys.stderr); sys.exit(1)
specs.sort(key=lambda s: s['global_sequence'])   # recorded order across correlations
with open('/tmp/ucs-redrive-specs.jsonl', 'w') as f:
    for s in specs: f.write(json.dumps(s) + '\n')
note = f' ({skipped} undecoded attempt(s) skipped)' if skipped else ''
print(f'    staged {kept} events; {len(specs)} recorded request(s) to re-drive{note}')"
TABLE=$("$BRIDGE" prepare /tmp/ucs-rec.jsonl "$REC_ID" "$STATE" "$RUN_ID")

echo "==> [4/7] boot UCS in replay mode (LookupTableHook) — connector NOT started"
pkill -9 -f "target/debug/grpc-server" 2>/dev/null || true
pkill -f deja-local/mock_connector.py 2>/dev/null || true
sleep 1
# Config parity with the RECORDING is load-bearing (the same-effective-config invariant):
# TestConfig rewrites the connector URL BEFORE the boundary, so replaying a tape recorded
# without test mode under test mode (or vice versa) diverges the egress args and
# fail-stops. Default matches record-demo.sh; for tapes recorded against real connectors
# (e.g. hyperswitch-driven sessions) run with DEJA_TEST_MODE=false.
DEJA_TEST_MODE="${DEJA_TEST_MODE:-true}"
(
  cd "$ROOT"
  CS__COMMON__ENVIRONMENT=development \
  CS__DEJA__MODE=replay \
  CS__DEJA__REPLAY__SOURCE="$TABLE" \
  CS__DEJA__REPLAY__OBSERVED_SINK="$STATE/observed/$RUN_ID.jsonl" \
  CS__TEST__ENABLED="$DEJA_TEST_MODE" \
  CS__TEST__MOCK_SERVER_URL=http://localhost:3000/mock \
  nohup "$BIN" > /tmp/deja-replay-server.log 2>&1 < /dev/null &
  disown
)
sleep 4
grep -q "deja runtime hook installed" /tmp/deja-replay-server.log \
  || { echo "    BOOT FAILED — see /tmp/deja-replay-server.log"; exit 1; }
echo "    replay hook installed from rendered table"

echo "==> [5/7] re-drive ALL selected recorded requests (rpc/metadata/payload from the tape, recorded order)"
: > "$STATE/http-diffs/$RUN_ID.jsonl"
DRIVEN=0
while IFS= read -r SPEC_LINE; do
  [ -n "$SPEC_LINE" ] || continue
  printf '%s' "$SPEC_LINE" > /tmp/ucs-spec-cur.json
  python3 -c "
import json
spec = json.load(open('/tmp/ucs-spec-cur.json'))
json.dump(spec['body'], open('/tmp/ucs-redrive-body.json', 'w'))
open('/tmp/ucs-redrive-rpc.txt', 'w').write(spec['rpc'])
open('/tmp/ucs-redrive-headers.txt', 'w').write('\n'.join(f'{n}: {v}' for n, v in spec['headers']))"
  REDRIVE_RPC=$(cat /tmp/ucs-redrive-rpc.txt)
  REDRIVE_HDRS=()
  while IFS= read -r line; do [ -n "$line" ] && REDRIVE_HDRS+=(-H "$line"); done < /tmp/ucs-redrive-headers.txt
  REDRIVE_RC=0
  grpcurl -max-time 20 -plaintext "${REDRIVE_HDRS[@]}" \
    -d @ localhost:8000 "$REDRIVE_RPC" < /tmp/ucs-redrive-body.json \
    > /tmp/ucs-redrive-reply.json 2>/tmp/ucs-redrive-err.txt || REDRIVE_RC=$?
  DRIVEN=$((DRIVEN + 1))

  # HTTP-diff row per driven request: recorded response decoded from its gRPC frames via
  # the server's descriptors and diffed FIELD-BY-FIELD against the replayed response.
  python3 -c "
import json, subprocess
spec = json.load(open('/tmp/ucs-spec-cur.json'))
try: candidate_body = json.load(open('/tmp/ucs-redrive-reply.json'))
except Exception: candidate_body = None
try: err_text = open('/tmp/ucs-redrive-err.txt').read()
except Exception: err_text = ''
baseline_body = None
if spec.get('recorded_response_b64'):
    try:
        out = subprocess.run(['$BRIDGE', 'decode-response', '/' + spec['rpc'],
                              spec['recorded_response_b64']],
                             capture_output=True, text=True, timeout=30)
        if out.returncode == 0: baseline_body = json.loads(out.stdout)
    except Exception: pass
# Symmetric outcome semantics: 'the rpc responded' on both sides.
baseline_status = 500 if spec['is_error'] else 200
candidate_status = 200 if ($REDRIVE_RC == 0 or 'Code:' in err_text) else 500
def walk(b, c, path, out):
    if b == c: return
    if isinstance(b, dict) and isinstance(c, dict):
        for k in sorted(set(b) | set(c)): walk(b.get(k), c.get(k), f'{path}.{k}', out)
    elif isinstance(b, list) and isinstance(c, list) and len(b) == len(c):
        for i, (x, y) in enumerate(zip(b, c)): walk(x, y, f'{path}[{i}]', out)
    else:
        out.append({'json_path': path or '\$', 'baseline': b, 'candidate': c})
body_diff = []
if baseline_body is not None and candidate_body is not None:
    walk(baseline_body, candidate_body, '\$', body_diff)
row = {
    'correlation_id': spec['correlation_id'],
    'request_sequence': spec['request_sequence'],
    'request_path': '/' + spec['rpc'],
    'status_baseline': baseline_status,
    'status_candidate': candidate_status,
    'status_match': baseline_status == candidate_status,
    'body_diff': body_diff,
    'baseline_body': baseline_body,
    'candidate_body': candidate_body,
}
open('$STATE/http-diffs/$RUN_ID.jsonl', 'a').write(json.dumps(row) + '\n')
compared = 'field-by-field' if baseline_body is not None and candidate_body is not None else 'status-only'
print(f\"    {spec['correlation_id']} seq {spec['request_sequence']}: recorded={baseline_status} replayed={candidate_status} match={row['status_match']} · body {compared}, {len(body_diff)} field(s) differ\")"
done < /tmp/ucs-redrive-specs.jsonl
echo "    drove $DRIVEN recorded request(s)"
pkill -f "target/debug/grpc-server"; sleep 2

echo "==> [6/7] score with deja-orchestrator's divergence scorer"
"$BRIDGE" score "$STATE" "$RUN_ID" | python3 -c "
import json, sys
card = json.load(sys.stdin)
v = card.get('verdict', {})
s = card.get('summary', {})
print('    verdict.pass:', v.get('pass'))
print('    reason:      ', v.get('reason'))
print('    requests reproduced:', f\"{s.get('matched_correlations')}/{s.get('total_correlations')}\")
print('    resolved_by_rank:', s.get('resolved_by_rank'))"

# Register the run (and its recording) in the deja dashboard's store, if it is up —
# the runs LIST is pg-backed; the detail/scorecard pages read the file artifacts.
if docker ps --format '{{.Names}}' 2>/dev/null | grep -q '^deja-orchestrator-pg-1$'; then
  python3 -c "
import json
card = json.load(open('$STATE/runs/$RUN_ID.scorecard.json'))
run = json.load(open('$STATE/runs/$RUN_ID.json'))
def q(v): return \"'\" + json.dumps(v).replace(\"'\", \"''\") + \"'\"
verdict = \"'pass'\" if card.get('verdict', {}).get('pass') else \"'fail'\"
print(f'''INSERT INTO replay_runs (run_id, mode, recording_id, candidate, params, expectation, created_by, state, started_at, verdict, scorecard)
VALUES ('$RUN_ID', 'replay', '$REC_ID', {q(run['spec']['candidate_spec'])}, {q({'correlations': [c.strip() for c in open('/tmp/ucs-corrs.txt') if c.strip()]})}, NULL, 'replay-demo', 'completed', now(), {verdict}, {q(card)})
ON CONFLICT (run_id) DO UPDATE SET verdict=EXCLUDED.verdict, scorecard=EXCLUDED.scorecard;
INSERT INTO recordings (recording_id, source_path, event_count, correlation_count, byte_size, manifest, created_by)
VALUES ('$REC_ID', 'kafka topic ucs-deja-recording-events ($CORR_COUNT correlation(s))', 0, $CORR_COUNT, 0, NULL, 'replay-demo')
ON CONFLICT (recording_id) DO NOTHING;''')" \
  | docker exec -i deja-orchestrator-pg-1 psql -q -U deja -d deja >/dev/null 2>&1 \
    && echo "    registered in dashboard: http://127.0.0.1:8070/runs/$RUN_ID" \
    || echo "    (dashboard registration failed — run still fully scored on disk)"
else
  echo "    (dashboard pg not running — skipping registration)"
fi

echo "==> [7/7] interactive HTML timeline"
python3 "$DEJA_REPO/demo/visualize-replay.py" "$STATE" >/dev/null 2>&1 || true
echo
echo "see it:"
echo "  timeline:   open $STATE/replay-visualization.html"
echo "  scorecard:  cat $STATE/runs/$RUN_ID.scorecard.json | jq .verdict"
echo "  observed:   cat $STATE/observed/$RUN_ID.jsonl | jq ."
echo "  table:      jq . $TABLE | head"
