#!/usr/bin/env bash
#
# Verifies a newly added connector against the suites it declares.
#
# live_creds true runs every scenario of every declared suite against the
# sandbox. False is allowed but needs a reason, which is surfaced on the PR.
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

# Runs everything the connector declares; the framework derives suites and
# scenarios from its specs.json.
verify_connector() {
  local name="$1"
  local args=(--skip-setup --no-build --connector "${name}" --interface grpc --report)

  echo "::group::Verify ${name}"
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

  # An empty list means nothing is covered.
  suite_count=$(jq '.supported_suites // [] | length' "${specs}")
  if [[ "${suite_count}" -eq 0 ]]; then
    echo "::error::New connector '${name}' declares no \"supported_suites\" in ${specs}. List every suite the connector supports; nothing is certified without it."
    failures=$((failures + 1))
    continue
  fi

  # Must be stated: silence here used to pass while certifying nothing.
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

  # Everything the connector declares runs against the sandbox.
  if ! verify_connector "${name}"; then
    echo "::error::Verification failed for new connector ${name}"
    failures=$((failures + 1))
  fi
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
