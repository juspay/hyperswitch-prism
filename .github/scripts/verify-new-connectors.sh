#!/usr/bin/env bash
#
# Verifies a newly added connector against the suites it declares.
#
# The connector's own specs.json is the statement: supported_suites says what it
# supports, and live_creds says whether CI can prove it. When live_creds is true
# every scenario of every declared suite runs against the real sandbox here. A
# connector may say false, but never silently — it must give a reason, and the
# reason is surfaced on the PR.
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
SUITES_ROOT="crates/internal/integration-tests/src/global_suites"
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

# Runs one declared scenario and confirms it actually executed. run-tests exits
# non-zero on failure; a scenario that was skipped rather than run is caught by
# test_ucs itself, which treats a named scenario that never ran as an error.
verify_scenario() {
  local name="$1" suite="$2" scenario="$3"
  local args=(--skip-setup --no-build --connector "${name}" --suite "${suite}"
              --scenario "${scenario}" --interface grpc --report)

  echo "::group::Verify ${name} (${suite} / ${scenario})"
  local rc=0
  ./scripts/run-tests "${args[@]}" || rc=$?
  echo "::endgroup::"
  return "${rc}"
}

IFS=',' read -ra CONNECTORS <<< "${NEW_CONNECTORS}"

for name in "${CONNECTORS[@]}"; do
  [[ -n "${name}" ]] || continue

  specs="${SPECS_ROOT}/${name}/specs.json"
  if [[ ! -f "${specs}" ]]; then
    echo "::error::New connector '${name}' has no specs.json at ${specs}"
    failures=$((failures + 1))
    continue
  fi

  # supported_suites is where the author states, by hand, what the connector
  # supports. It is what check_connector_specs verifies the code against and
  # what certification runs from, so an empty list means nothing is covered.
  # Reading it needs no inference about the source, so it cannot misfire.
  suite_count=$(jq '.supported_suites // [] | length' "${specs}")
  if [[ "${suite_count}" -eq 0 ]]; then
    echo "::error::New connector '${name}' declares no \"supported_suites\" in ${specs}. List every suite the connector supports; nothing is certified without it."
    failures=$((failures + 1))
    continue
  fi

  # live_creds is the author's statement about whether CI can prove any of this.
  # It must be present: a new connector arriving with no statement is the case
  # that used to pass silently and certify nothing.
  if [[ "$(jq 'has("live_creds")' "${specs}")" != "true" ]]; then
    echo "::error::New connector '${name}' does not declare \"live_creds\" in ${specs}. Set it to true when the connector has credentials in the CI file, or false with a \"no_creds_reason\" saying why it cannot be proven yet."
    failures=$((failures + 1))
    continue
  fi

  live_creds=$(jq -r '.live_creds' "${specs}")
  no_creds_reason=$(jq -r '.no_creds_reason // empty' "${specs}")

  if [[ "${live_creds}" != "true" ]]; then
    if [[ -z "${no_creds_reason}" ]]; then
      echo "::error::New connector '${name}' sets live_creds: false with no no_creds_reason in ${specs}. State why, e.g. \"sandbox access requested, pending <team/ticket>\"."
      failures=$((failures + 1))
      continue
    fi
    echo "::warning::'${name}' is merging WITHOUT live sandbox proof. Reason: ${no_creds_reason}"
    unproven+=("\`${name}\` — _${no_creds_reason}_")
    continue
  fi

  if ! jq -e --arg c "${name}" 'has($c)' "${CONNECTOR_AUTH_FILE_PATH}" >/dev/null; then
    echo "::error::New connector '${name}' sets live_creds: true but has no entry in the CI credentials file — add the credentials, or set live_creds: false with a reason. Without an entry the sweep would skip it, and a connector missing from the report reads as no regression."
    failures=$((failures + 1))
    continue
  fi

  # Every declared suite runs. The author said the connector supports it; this is
  # where that claim meets the sandbox.
  while IFS= read -r suite; do
    [[ -n "${suite}" ]] || continue
    suite_file="${SUITES_ROOT}/${suite//\//_}/scenario.json"
    if [[ ! -f "${suite_file}" ]]; then
      echo "::warning::'${name}' declares ${suite}, which has no scenario.json — nothing to run."
      continue
    fi
    while IFS= read -r scenario; do
      [[ -n "${scenario}" ]] || continue
      if ! verify_scenario "${name}" "${suite}" "${scenario}"; then
        echo "::error::Verification failed for new connector ${name} (${suite} / ${scenario})"
        failures=$((failures + 1))
      fi
    done < <(jq -r 'keys[]' "${suite_file}")
  done < <(jq -r '(.supported_suites // [])[]' "${specs}")
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
    echo "Each line above is a \`live_creds: false\` declaration in the connector's \`specs.json\`. Confirm the stated reason is real before approving."
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
