#!/usr/bin/env bash
# Commit ONLY the new grace v2 workflow files to a fresh branch.
# Pre-existing uncommitted changes to grace/workflow/{2_connector,2.2_techspec,
# 2.3_codegen}.md and grace/rulesbook/codegen/guides/github_issue_template.md
# are intentionally left unstaged and untouched.

set -euo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO"

BRANCH="${1:-feat/grace-v2-workflow}"
BASE_REF="$(git rev-parse --abbrev-ref HEAD)"

echo "[grace-v2-commit] base branch: $BASE_REF"
echo "[grace-v2-commit] target branch: $BRANCH"

# Sanity: don't blow up if branch already exists
if git rev-parse --verify --quiet "refs/heads/$BRANCH" >/dev/null; then
  echo "[grace-v2-commit] branch '$BRANCH' already exists" >&2
  echo "  delete with: git branch -D $BRANCH"
  exit 1
fi

# Create branch from current HEAD. Pre-existing uncommitted edits travel with
# the working tree (unstaged) — they'll remain unstaged on the new branch.
git checkout -b "$BRANCH"

# Stage ONLY the v2 workflow + bootstrap + scaffold files.
FILES=(
  "connector.json"
  "scripts/grace_v2_bootstrap.sh"
  "scripts/grace_v2_commit.sh"
  "grace/workflow/v2/1_orchestrator.md"
  "grace/workflow/v2/2_connector.md"
  "grace/workflow/v2/2.1_link_scoring.md"
  "grace/workflow/v2/2.2_techspec.md"
  "grace/workflow/v2/2.3_item_codegen.md"
  "grace/workflow/v2/2.4_pr.md"
  "grace/workflow/v2/2.5_grpcurl_runner.md"
  "grace/workflow/v2/work/.gitkeep"
  "grace/workflow/v2/work/.gitignore"
)

for f in "${FILES[@]}"; do
  if [[ -e "$f" ]]; then
    git add "$f"
  else
    echo "  ! missing file (skipped): $f" >&2
  fi
done

# Show what is about to be committed
echo
echo "[grace-v2-commit] staged for commit:"
git diff --cached --stat

# Commit
git commit -m "feat(grace): add v2 status-API-driven workflow

Adds a new orchestrator workflow (grace/workflow/v2/) that:

- Reads connector.json (smoke set: stripe, adyen)
- Calls the status API (connectors-status-production.up.railway.app)
  to discover state==not_implemented payment methods + API flows
- Generates per-connector unimplemented snapshots and an aggregate
  manifest (Phase 0, deterministic — scripts/grace_v2_bootstrap.sh)
- Per connector, dispatches a separate Claude session via openswarm
  exec --local --pipeline (sequential, never parallel)
- Inside each session: link discovery + 5-criterion confidence
  scoring (host/200/title/markers/path), techspec only for
  high-confidence items, per-item codegen via separate subagents,
  grpcurl validation, batched per-item PR

Existing v1 workflow at grace/workflow/{1_orchestrator,2_connector,
2.1_links,...}.md is untouched; v2 lives alongside as a peer."

echo
echo "[grace-v2-commit] done."
git log --oneline -1
echo
echo "Branch: $BRANCH"
echo "  return to base with: git checkout $BASE_REF"
echo "  push with:           git push -u origin $BRANCH"
