#!/usr/bin/env bash
# Grace v2 — Phase 0 bootstrap.
#
# Reads connector.json (a JSON array of slugs) at the repo root, hits the
# status API for each slug, filters items in state "not_implemented" (Decision
# 5: unknown is skipped silently), and writes per-connector artifacts under
# grace/workflow/v2/work/:
#   {slug}_unimplemented.md   human-readable snapshot
#   {slug}_manifest.json      machine-readable input for the per-connector session
# Plus an aggregate work/_manifest.json the orchestrator iterates over.
#
# No LLM. Pure shell + curl + jq.

set -euo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WORK="$REPO/grace/workflow/v2/work"
API="https://connectors-status-production.up.railway.app/api/connectors"

command -v jq   >/dev/null 2>&1 || { echo "jq is required (brew install jq)" >&2; exit 1; }
command -v curl >/dev/null 2>&1 || { echo "curl is required" >&2; exit 1; }

INPUT="${1:-$REPO/connector.json}"
[[ -f "$INPUT" ]] || { echo "connector list not found: $INPUT" >&2; exit 1; }

mkdir -p "$WORK"
# Wipe stale per-connector files so a re-run reflects the current connector.json
# (preserves the .gitkeep + .gitignore in work/).
find "$WORK" -maxdepth 1 -type f \( -name "*_unimplemented.md" -o -name "*_manifest.json" \) -delete 2>/dev/null || true
rm -f "$WORK/_manifest.json"

declare -i processed=0 skipped_empty=0 skipped_error=0

while read -r slug; do
  [[ -z "$slug" ]] && continue
  echo "[grace-v2] $slug"

  pm_url="$API/$slug/payment_method"
  ap_url="$API/$slug/api"

  if ! pm_json=$(curl -fsSL "$pm_url"); then
    echo "  ! payment_method fetch failed; skipping connector" >&2
    skipped_error+=1; sleep 0.2; continue
  fi
  sleep 0.2
  if ! ap_json=$(curl -fsSL "$ap_url"); then
    echo "  ! api fetch failed; skipping connector" >&2
    skipped_error+=1; sleep 0.2; continue
  fi
  sleep 0.2

  not_impl_pm=$(echo "$pm_json" | jq '[.payment_methods[]? | select(.state == "not_implemented")]')
  not_impl_ap=$(echo "$ap_json" | jq '[.apis[]?            | select(.state == "not_implemented")]')

  pm_count=$(echo "$not_impl_pm" | jq 'length')
  ap_count=$(echo "$not_impl_ap" | jq 'length')

  if [[ "$pm_count" -eq 0 && "$ap_count" -eq 0 ]]; then
    echo "  · nothing unimplemented; skipping"
    skipped_empty+=1
    continue
  fi

  snapshot="$WORK/${slug}_unimplemented.md"
  {
    echo "# $slug — Unimplemented Items"
    echo
    echo "_Snapshot: $(date -u +%Y-%m-%dT%H:%M:%SZ)_"
    echo
    echo "Source: $API/$slug/{payment_method,api}"
    echo
    echo "## API Flows ($ap_count not_implemented)"
    echo
    if [[ "$ap_count" -gt 0 ]]; then
      echo "$not_impl_ap" | jq -r '.[] | "- `\(.id)` | **\(.name)** | section: \(.section // "—") | \(.note // "")"'
    else
      echo "_(none)_"
    fi
    echo
    echo "## Payment Methods ($pm_count not_implemented, grouped by source_flow)"
    echo
    if [[ "$pm_count" -gt 0 ]]; then
      echo "$not_impl_pm" | jq -r '
        group_by(.source_flow // "unknown")[]
        | "### " + (.[0].source_flow // "unknown") + "\n"
          + (map("- `\(.id)` | **\(.name)** | section: \(.section // "—") | \(.note // "")") | join("\n"))
          + "\n"
      '
    else
      echo "_(none)_"
    fi
  } > "$snapshot"

  manifest="$WORK/${slug}_manifest.json"
  jq -n \
    --arg slug "$slug" \
    --arg snapshot "$snapshot" \
    --argjson pm_count "$pm_count" \
    --argjson ap_count "$ap_count" \
    --argjson items_pm "$not_impl_pm" \
    --argjson items_ap "$not_impl_ap" \
    '{
      connector: $slug,
      snapshot: $snapshot,
      counts: {payment_methods: $pm_count, api_flows: $ap_count},
      items_payment_methods: $items_pm,
      items_api_flows:       $items_ap
    }' > "$manifest"

  echo "  ✓ wrote $(basename "$snapshot") + $(basename "$manifest")  (pm=$pm_count, api=$ap_count)"
  processed+=1
done < <(jq -r '.[]' "$INPUT")

# Aggregate per-connector manifests into one orchestrator-input file.
shopt -s nullglob
manifests=( "$WORK"/*_manifest.json )
shopt -u nullglob
if [[ ${#manifests[@]} -gt 0 ]]; then
  jq -s '.' "${manifests[@]}" > "$WORK/_manifest.json"
else
  echo '[]' > "$WORK/_manifest.json"
fi

echo
echo "[grace-v2] done. processed=$processed empty=$skipped_empty errors=$skipped_error"
echo "          aggregate manifest: $WORK/_manifest.json"
