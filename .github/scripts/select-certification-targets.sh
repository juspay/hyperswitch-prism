#!/usr/bin/env bash
#
# Decides which scenarios this run should certify.
#
# Two triggers, two sets, because they answer different questions:
#
#   A connector's own files changed -> certify that connector against every
#   scenario of every suite it declares in supported_suites. The author already
#   stated what the connector supports; running it is how the claim is checked.
#
#   Shared code changed (core, proto, harness) -> certify only the connectors in
#   live-connectors.json. Their failure is the one that means a real payment
#   breaks, and a full sweep of every connector belongs on the nightly, not on a
#   PR that has to turn around in minutes.
#
# Inputs (environment):
#   RUST_CORE_CHANGED    "true" when core/harness sources changed
#   PROTO_CHANGED        "true" when proto definitions changed
#   CHANGED_CONNECTORS   comma-separated connector names touched by this PR
#   NEW_CONNECTORS       comma-separated names added by this PR — excluded here,
#                        see the skip below
#   SPECS_ROOT           override for the connector_specs directory (tests)
#   SUITES_ROOT          override for the global_suites directory (tests)
#   LIVE_CONNECTORS      override for live-connectors.json (tests)
#   GITHUB_OUTPUT        step output file (optional)
#
# Emits a JSON array of {name, suite, scenario} on stdout, and as the `targets`
# step output when GITHUB_OUTPUT is set.

set -uo pipefail

SPECS_ROOT="${SPECS_ROOT:-crates/internal/integration-tests/src/connector_specs}"
SUITES_ROOT="${SUITES_ROOT:-crates/internal/integration-tests/src/global_suites}"
LIVE_CONNECTORS="${LIVE_CONNECTORS:-.github/test/live-connectors.json}"

shared_changed="false"
if [[ "${RUST_CORE_CHANGED:-}" == "true" || "${PROTO_CHANGED:-}" == "true" ]]; then
  shared_changed="true"
fi

# Every scenario of one suite, as {name, suite, scenario} objects.
scenarios_for() {
  local name="$1" suite="$2"
  local file="${SUITES_ROOT}/${suite//\//_}/scenario.json"
  [[ -f "${file}" ]] || return 0
  jq -c --arg n "${name}" --arg s "${suite}" \
    '[ keys[] | { name: $n, suite: $s, scenario: . } ]' "${file}"
}

targets="[]"

if [[ "${shared_changed}" == "true" ]]; then
  if [[ ! -f "${LIVE_CONNECTORS}" ]]; then
    echo "::error::Shared code changed but ${LIVE_CONNECTORS} is missing — nothing would be certified." >&2
    exit 1
  fi
  echo "Shared code changed — certifying the live connector set" >&2
  targets=$(jq -c '[ .entries[] | { name: .connector, suite, scenario } ]' "${LIVE_CONNECTORS}")
else
  echo "Certifying connectors touched by this PR" >&2
  IFS=',' read -ra CHANGED <<< "${CHANGED_CONNECTORS:-}"
  for name in "${CHANGED[@]}"; do
    [[ -n "${name}" ]] || continue
    specs="${SPECS_ROOT}/${name}/specs.json"
    [[ -f "${specs}" ]] || continue

    # A connector this PR adds is handled by verify-new-connectors.sh, which runs
    # the same scenarios. Selecting it here would run every one of them twice
    # against the sandbox, and the arbitration that follows could not judge it
    # anyway: it does not exist at the merge base, so the base run fails for that
    # reason alone and every verdict would come back "not attributable".
    if [[ ",${NEW_CONNECTORS:-}," == *",${name},"* ]]; then
      echo "  ${name}: added by this PR — certified by the new-connector gate" >&2
      continue
    fi

    # A connector states in its own specs.json whether it has credentials in CI.
    # Absent or false means it is not certified here; verify-new-connectors.sh is
    # what insists the statement is present and reasoned for a new connector.
    if [[ "$(jq -r '.live_creds // false' "${specs}")" != "true" ]]; then
      echo "  ${name}: live_creds is not true — not certified by this PR" >&2
      continue
    fi

    while IFS= read -r suite; do
      [[ -n "${suite}" ]] || continue
      entries=$(scenarios_for "${name}" "${suite}")
      [[ -n "${entries}" ]] || continue
      targets=$(jq -c -s 'add' <<< "${targets}"$'\n'"${entries}")
    done < <(jq -r '(.supported_suites // [])[]' "${specs}")
  done
fi

count=$(jq 'length' <<< "${targets}")
echo "Selected ${count} scenario(s)" >&2

[[ -n "${GITHUB_OUTPUT:-}" ]] && echo "targets=${targets}" >> "${GITHUB_OUTPUT}"
echo "${targets}"
