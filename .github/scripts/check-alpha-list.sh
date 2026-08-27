#!/usr/bin/env bash
#
# Validates alpha_connectors.json — the record of which connectors CI does not
# certify against a live sandbox.
#
# Two lists with different rules. `grandfathered` predates certification and may
# only shrink. `unproven_new` is a per-connector exception and needs a reason.
# Both are checked here so the file cannot rot into a list nobody trusts.
#
# Inputs (environment):
#   SPECS_ROOT                connector_specs directory (default: in-repo path)
#   BASE_SHA                  merge base; empty skips the growth check
#   CONNECTOR_AUTH_FILE_PATH  decrypted credentials; empty skips creds checks
#
# Exit status: 0 unless the file is inconsistent with the repo or the rules.

set -uo pipefail

SPECS_ROOT="${SPECS_ROOT:-crates/internal/integration-tests/src/connector_specs}"
ALPHA_FILE="${SPECS_ROOT}/alpha_connectors.json"
BASE_SHA="${BASE_SHA:-}"
failures=0

fail() { echo "::error::$*"; failures=$((failures + 1)); }

if [[ ! -f "${ALPHA_FILE}" ]]; then
  fail "${ALPHA_FILE} is missing. Every connector must be either certified or recorded there."
  exit 1
fi

if ! jq -e '.grandfathered and .unproven_new' "${ALPHA_FILE}" >/dev/null 2>&1; then
  fail "${ALPHA_FILE} must contain both \"grandfathered\" and \"unproven_new\" objects."
  exit 1
fi

mapfile -t GRANDFATHERED < <(jq -r '.grandfathered | keys[]' "${ALPHA_FILE}")
mapfile -t UNPROVEN_NEW  < <(jq -r '.unproven_new  | keys[]' "${ALPHA_FILE}")

# A name in both lists means nobody knows which rule applies to it.
for name in ${UNPROVEN_NEW[@]+"${UNPROVEN_NEW[@]}"}; do
  if jq -e --arg n "${name}" '.grandfathered | has($n)' "${ALPHA_FILE}" >/dev/null; then
    fail "'${name}' is in both grandfathered and unproven_new. It belongs to exactly one."
  fi
done

# A name that no longer resolves is how a list quietly stops describing reality.
for name in ${GRANDFATHERED[@]+"${GRANDFATHERED[@]}"} ${UNPROVEN_NEW[@]+"${UNPROVEN_NEW[@]}"}; do
  [[ -f "${SPECS_ROOT}/${name}/specs.json" ]] || \
    fail "'${name}' is listed in ${ALPHA_FILE} but ${SPECS_ROOT}/${name}/specs.json does not exist. Remove the entry or restore the connector."
done

# The grandfathered list is the backlog, not a place to file new work. Without
# this a new integrator reads 101 names and adds the 102nd.
if [[ -n "${BASE_SHA}" ]]; then
  # The job checks out at depth 1, so the merge base is normally absent and this
  # check would quietly pass on every PR.
  if ! git cat-file -e "${BASE_SHA}^{commit}" 2>/dev/null; then
    git fetch --no-tags --depth=1 origin "${BASE_SHA}" >/dev/null 2>&1 || true
  fi
  if ! git cat-file -e "${BASE_SHA}^{commit}" 2>/dev/null; then
    echo "::warning::Merge base ${BASE_SHA} unavailable — cannot check that grandfathered only shrank"
  fi
fi

if [[ -n "${BASE_SHA}" ]] && git cat-file -e "${BASE_SHA}^{commit}" 2>/dev/null; then
  base_json=$(git show "${BASE_SHA}:${ALPHA_FILE}" 2>/dev/null || echo '{}')
  added=$(jq -rn --argjson base "$(jq '.grandfathered // {}' <<< "${base_json}")" \
                 --argjson head "$(jq '.grandfathered' "${ALPHA_FILE}")" \
            '$head | keys - ($base | keys) | .[]' 2>/dev/null || true)
  for name in ${added}; do
    fail "'${name}' was added to grandfathered. That list only shrinks — it is the pre-certification backlog. A connector merging unproven today goes in unproven_new with a reason."
  done
fi

# Every connector must be accounted for: certified, or written down here.
for specs in "${SPECS_ROOT}"/*/specs.json; do
  [[ -f "${specs}" ]] || continue
  name=$(basename "$(dirname "${specs}")")
  if jq -e --arg n "${name}" '(.grandfathered + .unproven_new) | has($n)' "${ALPHA_FILE}" >/dev/null; then
    continue
  fi
  # Not listed, so CI certifies it — which needs credentials to mean anything.
  if [[ -n "${CONNECTOR_AUTH_FILE_PATH:-}" && -s "${CONNECTOR_AUTH_FILE_PATH:-}" ]]; then
    jq -e --arg n "${name}" 'has($n)' "${CONNECTOR_AUTH_FILE_PATH}" >/dev/null || \
      fail "'${name}' is not listed in ${ALPHA_FILE}, so CI certifies it, but it has no entry in the credentials file. Add credentials, or record it in unproven_new with a reason."
  fi
done

# Credentials exist but the connector is still recorded as unproven. This is the
# check that stops an entry outliving its reason: nobody has to remember to
# promote it, because the build asks for it the moment the credentials land.
if [[ -n "${CONNECTOR_AUTH_FILE_PATH:-}" && -s "${CONNECTOR_AUTH_FILE_PATH:-}" ]]; then
  for name in ${GRANDFATHERED[@]+"${GRANDFATHERED[@]}"} ${UNPROVEN_NEW[@]+"${UNPROVEN_NEW[@]}"}; do
    if jq -e --arg n "${name}" 'has($n)' "${CONNECTOR_AUTH_FILE_PATH}" >/dev/null; then
      fail "'${name}' has credentials in CI but is still listed in ${ALPHA_FILE}. Remove the entry so it is certified."
    fi
  done
fi

# Carrying production traffic and having never been proven is a contradiction,
# and it is the shape that quietly empties a shared-code sweep: the connector is
# selected for being live, then skipped for being alpha.
for name in ${GRANDFATHERED[@]+"${GRANDFATHERED[@]}"} ${UNPROVEN_NEW[@]+"${UNPROVEN_NEW[@]}"}; do
  specs="${SPECS_ROOT}/${name}/specs.json"
  [[ -f "${specs}" ]] || continue
  if [[ "$(jq -r '.live_in_production // false' "${specs}")" == "true" ]]; then
    fail "'${name}' declares live_in_production but is listed in ${ALPHA_FILE} as unproven. Certify it, or drop live_in_production."
  fi
done

# Reason is what makes the entry reviewable; a bare name says only that someone
# wanted the build to stop asking.
for name in ${UNPROVEN_NEW[@]+"${UNPROVEN_NEW[@]}"}; do
  reason=$(jq -r --arg n "${name}" '.unproven_new[$n].reason // ""' "${ALPHA_FILE}")
  [[ -n "${reason}" ]] || \
    fail "'${name}' is in unproven_new with no \"reason\". State why it cannot be proven yet, e.g. \"sandbox access requested, <ticket>\"."
done

echo "alpha list: ${#GRANDFATHERED[@]} grandfathered (TODO, shrink to zero), ${#UNPROVEN_NEW[@]} unproven-new"

if [[ "${failures}" -gt 0 ]]; then
  echo "::error::${failures} problem(s) in ${ALPHA_FILE}"
  exit 1
fi
exit 0
