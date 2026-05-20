#!/usr/bin/env bash
# Grace v2 — outer-loop runner. Replaces grace/workflow/v2/1_orchestrator.md
# (which is a markdown agent) with a deterministic shell loop. Each iteration
# dispatches one connector through `openswarm exec --local --pipeline` against
# `grace/workflow/v2/2_connector.md`.
#
# Why a shell loop instead of the LLM orchestrator: the orchestrator MD's
# pre-flight does `git checkout main && git pull`, which would lose the v2
# workflow files when they live on a feature branch. The shell loop honors
# whatever branch you're already on.
#
# Resumable: if {slug}_results.json already exists with a non-FAILED status,
# the connector is skipped on re-run. Delete the file to force a re-run.

set -uo pipefail   # NOTE: not -e — we want partial failures to continue the loop

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WORK="$REPO/grace/workflow/v2/work"
DATE_TAG="$(date -u +%Y%m%d)"

BRANCH="${BRANCH:-feat/grace-v2-run-$DATE_TAG}"
LINK_THRESHOLD="${LINK_THRESHOLD:-0.70}"
TIMEOUT_SEC="${TIMEOUT_SEC:-10800}"   # 3h per connector
CONNECTORS_FILE="${CONNECTORS_FILE:-$REPO/connector.json}"

# Preflight ----------------------------------------------------------------
for tool in jq curl grpcurl make cargo openswarm gh; do
  command -v "$tool" >/dev/null || { echo "missing tool: $tool" >&2; exit 1; }
done
test -f "$REPO/creds.json"      || { echo "creds.json not found at repo root" >&2; exit 1; }
test -f "$CONNECTORS_FILE"      || { echo "no connector list at $CONNECTORS_FILE" >&2; exit 1; }

cd "$REPO"

# Bootstrap if manifest is stale relative to connector.json.
if [[ ! -f "$WORK/_manifest.json" ]] \
   || [[ "$CONNECTORS_FILE" -nt "$WORK/_manifest.json" ]]; then
  echo "[grace-v2-run] manifest stale — re-running bootstrap"
  bash "$REPO/scripts/grace_v2_bootstrap.sh"
fi

# Branch setup. Create-or-stay; never checkout main.
CURRENT="$(git branch --show-current)"
if [[ "$CURRENT" != "$BRANCH" ]]; then
  if git rev-parse --verify --quiet "refs/heads/$BRANCH" >/dev/null; then
    git checkout "$BRANCH"
  else
    git checkout -b "$BRANCH"
  fi
fi
echo "[grace-v2-run] branch: $BRANCH"
echo "[grace-v2-run] threshold: $LINK_THRESHOLD"
echo "[grace-v2-run] timeout/connector: ${TIMEOUT_SEC}s"

# Loop ---------------------------------------------------------------------
trap 'echo; echo "[grace-v2-run] interrupted at: $slug. Already-done connectors are preserved."; exit 130' INT

declare -i idx=0 total ran=0 skipped=0 failed=0
total=$(jq -r '.[]' "$CONNECTORS_FILE" | wc -l | tr -d ' ')

while read -r slug; do
  [[ -z "$slug" ]] && continue
  idx+=1
  echo
  echo "================================================================"
  echo "[$idx/$total] $slug    $(date -Iseconds)"
  echo "================================================================"

  manifest="$WORK/${slug}_manifest.json"
  if [[ ! -f "$manifest" ]]; then
    echo "  ! no manifest for $slug (bootstrap may have skipped it: empty unimplemented). Skipping."
    skipped+=1
    continue
  fi

  results="$WORK/${slug}_results.json"
  if [[ -f "$results" ]]; then
    prior_status=$(jq -r '.status // "UNKNOWN"' "$results")
    if [[ "$prior_status" =~ ^(SUCCESS|PARTIAL|SKIPPED)$ ]]; then
      echo "  · resumable: prior run is $prior_status — skipping (delete $results to re-run)"
      skipped+=1
      continue
    fi
  fi

  prompt="Read and follow the workflow defined in grace/workflow/v2/2_connector.md.

Variables:
  CONNECTOR: $slug
  MANIFEST: grace/workflow/v2/work/${slug}_manifest.json
  BRANCH: $BRANCH
  LINK_THRESHOLD: $LINK_THRESHOLD"

  if openswarm exec --local --pipeline \
        --path "$REPO" \
        --timeout "$TIMEOUT_SEC" \
        "$prompt"; then
    ran+=1
  else
    rc=$?
    echo "  ! openswarm exec returned $rc"
    failed+=1
  fi

  # Brief per-connector summary if we got a results file
  if [[ -f "$results" ]]; then
    jq -r '"  → status=\(.status // "?")  pr=\(.pr_url // "—")  items: success=\([.items[]?|select(.status=="SUCCESS")]|length) failed=\([.items[]?|select(.status=="FAILED")]|length) skipped=\([.items[]?|select(.status=="SKIPPED")]|length)"' "$results"
  fi
done < <(jq -r '.[]' "$CONNECTORS_FILE")

echo
echo "================================================================"
echo "[grace-v2-run] DONE"
echo "  ran=$ran  skipped=$skipped  failed=$failed  total=$total"
echo "  branch: $BRANCH"
echo "  results dir: $WORK"
echo "================================================================"
