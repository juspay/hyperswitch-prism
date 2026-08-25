#!/usr/bin/env bash
#
# Decides which scenarios this run should certify.
#
# Two triggers pick a different set of connectors, then both expand the same way:
# every scenario of every suite the connector declares, minus the payment methods
# it does not support.
#
#   A connector's own files changed -> certify that connector. The author stated
#   what it supports; running it is how the claim is checked.
#
#   Shared code changed (core, proto, harness) -> certify the connectors marked
#   live_in_production. Their failure is the one that means a real payment
#   breaks. A full sweep of all 100+ connectors belongs on the nightly, not on a
#   PR that has to turn around in minutes.
#
# What runs is derived from each connector's own specs.json, never from a
# separately curated list — supported_suites and supported_payment_methods
# already say it, and a second list restating a subset would be one more thing
# to keep in step.
#
# Inputs (environment):
#   RUST_CORE_CHANGED    "true" when core/harness sources changed
#   PROTO_CHANGED        "true" when proto definitions changed
#   CHANGED_CONNECTORS   comma-separated connector names touched by this PR
#   NEW_CONNECTORS       comma-separated names added by this PR — excluded, see below
#   SPECS_ROOT           override for the connector_specs directory (tests)
#   SUITES_ROOT          override for the global_suites directory (tests)
#   GITHUB_OUTPUT        step output file (optional)
#
# Emits a JSON array of {name, suite, scenario} on stdout, and as the `targets`
# step output when GITHUB_OUTPUT is set.

set -uo pipefail

SPECS_ROOT="${SPECS_ROOT:-crates/internal/integration-tests/src/connector_specs}"
SUITES_ROOT="${SUITES_ROOT:-crates/internal/integration-tests/src/global_suites}"

shared_changed="false"
if [[ "${RUST_CORE_CHANGED:-}" == "true" || "${PROTO_CHANGED:-}" == "true" ]]; then
  shared_changed="true"
fi

# Every scenario of one suite the connector can actually run, as
# {name, suite, scenario} objects.
#
# A scenario naming a payment_method the connector does not declare is left out.
# The runner filters the same way, so selecting one here would name a scenario
# that never runs — and test_ucs treats a named scenario that did not run as an
# error. An empty supported_payment_methods means the connector has not declared
# any, which selects everything.
scenarios_for() {
  local name="$1" suite="$2" specs="$3"
  local file="${SUITES_ROOT}/${suite//\//_}/scenario.json"
  [[ -f "${file}" ]] || return 0
  jq -c --arg n "${name}" --arg s "${suite}" \
     --argjson pms "$(jq -c '.supported_payment_methods // []' "${specs}")" \
    '[ to_entries[]
       | select(
           ($pms | length) == 0
           or (.value.grpc_req.payment_method // {} | keys | length) == 0
           or ((.value.grpc_req.payment_method | keys[0]) as $m | $pms | index($m))
         )
       | { name: $n, suite: $s, scenario: .key } ]' "${file}"
}

targets="[]"

# Expands one connector into its scenarios and appends them to `targets`.
add_connector() {
  local name="$1" specs="${SPECS_ROOT}/$1/specs.json"
  [[ -f "${specs}" ]] || return 0
  local suite entries
  while IFS= read -r suite; do
    [[ -n "${suite}" ]] || continue
    entries=$(scenarios_for "${name}" "${suite}" "${specs}")
    [[ -n "${entries}" ]] || continue
    targets=$(jq -c -s 'add' <<< "${targets}"$'\n'"${entries}")
  done < <(jq -r '(.supported_suites // [])[]' "${specs}")
}

if [[ "${shared_changed}" == "true" ]]; then
  echo "Shared code changed — certifying connectors live in production" >&2
  live=0
  for specs in "${SPECS_ROOT}"/*/specs.json; do
    [[ -f "${specs}" ]] || continue
    [[ "$(jq -r '.live_in_production // false' "${specs}")" == "true" ]] || continue
    name="$(basename "$(dirname "${specs}")")"
    add_connector "${name}"
    live=$((live + 1))
  done
  if [[ "${live}" -eq 0 ]]; then
    echo "::error::Shared code changed but no connector declares live_in_production — nothing would be certified." >&2
    exit 1
  fi
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

    add_connector "${name}"
  done
fi

count=$(jq 'length' <<< "${targets}")
echo "Selected ${count} scenario(s)" >&2

[[ -n "${GITHUB_OUTPUT:-}" ]] && echo "targets=${targets}" >> "${GITHUB_OUTPUT}"
echo "${targets}"
