#!/usr/bin/env bash
#
# Verifies the scenarios a newly added connector declares it was tested against.
#
# A connector added in this PR must say what it proved, and prove it: every
# verified_scenarios entry with live credentials is executed against the real
# sandbox here. An entry may opt out with has_live_creds: false, but never
# silently — it must give a reason, and the reason is surfaced on the PR.
#
# Inputs (environment):
#   NEW_CONNECTORS            comma-separated connector names added in this PR
#   CONNECTOR_AUTH_FILE_PATH  decrypted connector credentials
#   GITHUB_STEP_SUMMARY       job summary file (optional)
#   GITHUB_OUTPUT             step output file (optional)
#   UNPROVEN_FILE             where to record unproven declarations (optional)
#
# Exit status: 0 unless a declaration is missing, malformed, or fails to verify.

set -uo pipefail

SPECS_ROOT="crates/internal/integration-tests/src/connector_specs"
SRC_ROOT="crates/integrations/connector-integration/src/connectors"
SUMMARY="${GITHUB_STEP_SUMMARY:-/dev/null}"
UNPROVEN_FILE="${UNPROVEN_FILE:-}"

failures=0
declare -a unproven=()

if [[ -z "${NEW_CONNECTORS:-}" ]]; then
  echo "No newly added connectors."
  exit 0
fi

if [[ -z "${CONNECTOR_AUTH_FILE_PATH:-}" || ! -s "${CONNECTOR_AUTH_FILE_PATH}" ]]; then
  echo "::error::New connectors were added but no credentials file is available — cannot verify their declared scenarios."
  exit 1
fi

# Style requirement, applied only to connectors that did not exist at the base
# commit. Flow coverage is enforced separately and reads hand-written impls too,
# so this is about keeping new code on one pattern, not about correctness.
# Scoping it to new connectors is what makes it enforceable with no exception
# list for a future author to add themselves to.
require_prerequisites_macro() {
  local name="$1" src="${SRC_ROOT}/$1.rs"
  [[ -f "${src}" ]] || return 0
  if ! grep -q 'macros::create_all_prerequisites!(' "${src}"; then
    echo "::error::New connector '${name}' does not use create_all_prerequisites! in ${src}. New connectors must declare their flows through the macro; hand-written ConnectorIntegrationV2 impls are read by the coverage check but are not accepted as the primary flow declaration for new code."
    return 1
  fi
}

# Runs one declared scenario and confirms it actually executed. run-tests exits
# non-zero on failure; a scenario that was skipped rather than run is caught by
# test_ucs itself, which treats a named scenario that never ran as an error.
verify_scenario() {
  local name="$1" suite="$2" scenario="$3" skip_deps="$4"
  local args=(--skip-setup --no-build --connector "${name}" --suite "${suite}"
              --scenario "${scenario}" --interface grpc --report)
  [[ "${skip_deps}" == "true" ]] && args+=(--skip-dependencies)

  echo "::group::Verify ${name} (${suite} / ${scenario})"
  local rc=0
  ./scripts/run-tests "${args[@]}" || rc=$?
  echo "::endgroup::"
  return "${rc}"
}

IFS=',' read -ra CONNECTORS <<< "${NEW_CONNECTORS}"

for name in "${CONNECTORS[@]}"; do
  [[ -n "${name}" ]] || continue

  require_prerequisites_macro "${name}" || failures=$((failures + 1))

  specs="${SPECS_ROOT}/${name}/specs.json"
  if [[ ! -f "${specs}" ]]; then
    echo "::error::New connector '${name}' has no specs.json at ${specs}"
    failures=$((failures + 1))
    continue
  fi

  entry_count=$(jq '.verified_scenarios // [] | length' "${specs}")
  if [[ "${entry_count}" -eq 0 ]]; then
    echo "::error::New connector '${name}' declares no \"verified_scenarios\" in ${specs}. Every new connector must declare at least one entry { suite, scenario, has_live_creds, no_creds_reason (when has_live_creds is false) } so its own PR either proves a real sandbox call or records, in the diff, that it did not."
    failures=$((failures + 1))
    continue
  fi

  # Every flow the connector claims must have a scenario behind it. The claim
  # itself is already forced to match the code by check_connector_specs, so this
  # closes the last link: code -> supported_suites -> verified_scenarios. A flow
  # that genuinely cannot be exercised still needs an entry, with
  # has_live_creds: false and a reason — unproven is acceptable, unmentioned is
  # not. Without this a connector could ship six flows and certify one.
  uncovered=$(jq -r '
    (.supported_suites // []) as $declared
    | [ (.verified_scenarios // [])[].suite ] as $covered
    | $declared - $covered
    | .[]' "${specs}")
  if [[ -n "${uncovered}" ]]; then
    while IFS= read -r suite; do
      echo "::error::New connector '${name}' declares support for ${suite} but no verified_scenarios entry covers it. Add one — with has_live_creds: false and a no_creds_reason if it cannot be exercised in CI."
      failures=$((failures + 1))
    done <<< "${uncovered}"
  fi

  for i in $(seq 0 $((entry_count - 1))); do
    entry=$(jq -c ".verified_scenarios[$i]" "${specs}")
    suite=$(jq -r '.suite // empty' <<< "${entry}")
    scenario=$(jq -r '.scenario // empty' <<< "${entry}")
    skip_deps=$(jq -r '.skip_dependencies // false' <<< "${entry}")
    has_live_creds=$(jq -r '.has_live_creds // false' <<< "${entry}")
    no_creds_reason=$(jq -r '.no_creds_reason // empty' <<< "${entry}")

    if [[ -z "${suite}" || -z "${scenario}" ]]; then
      echo "::error::New connector '${name}' verified_scenarios[$i] is missing suite/scenario in ${specs}"
      failures=$((failures + 1))
      continue
    fi

    if [[ "${has_live_creds}" != "true" ]]; then
      if [[ -z "${no_creds_reason}" ]]; then
        echo "::error::New connector '${name}' verified_scenarios[$i] (${suite} / ${scenario}) sets has_live_creds: false with no no_creds_reason. State why, e.g. \"sandbox access requested, pending <team/ticket>\"."
        failures=$((failures + 1))
        continue
      fi
      echo "::warning::'${name}' (${suite} / ${scenario}) is merging WITHOUT live sandbox proof. Reason: ${no_creds_reason}"
      unproven+=("\`${name}\` — ${suite} / ${scenario} — _${no_creds_reason}_")
      continue
    fi

    if ! jq -e --arg c "${name}" 'has($c)' "${CONNECTOR_AUTH_FILE_PATH}" >/dev/null; then
      echo "::error::New connector '${name}' verified_scenarios[$i] sets has_live_creds: true but has no entry in the CI credentials file — fix the declaration or add the credentials"
      failures=$((failures + 1))
      continue
    fi

    if ! verify_scenario "${name}" "${suite}" "${scenario}" "${skip_deps}"; then
      echo "::error::Verification failed for new connector ${name} (${suite} / ${scenario})"
      failures=$((failures + 1))
    fi
  done
done

# Surface unproven declarations where a reviewer sees them without opening the
# job log. The file is handed to the commenting step through an explicit path
# and an output flag rather than a well-known location.
if [[ ${#unproven[@]} -gt 0 ]]; then
  {
    echo "## ⚠️ New connectors merging without live sandbox proof"
    echo ""
    for u in "${unproven[@]}"; do echo "- ${u}"; done
    echo ""
    echo "Each line above is a \`has_live_creds: false\` entry in the connector's \`specs.json\`. Confirm the stated reason is real before approving."
  } >> "${SUMMARY}"

  if [[ -n "${UNPROVEN_FILE}" ]]; then
    printf '%s\n' "${unproven[@]}" > "${UNPROVEN_FILE}"
    [[ -n "${GITHUB_OUTPUT:-}" ]] && echo "unproven=true" >> "${GITHUB_OUTPUT}"
  fi
fi

if [[ "${failures}" -gt 0 ]]; then
  echo "::error::${failures} new connector scenario(s) failed verification"
  exit 1
fi

echo "All newly added connectors verified their declared scenarios."
exit 0
