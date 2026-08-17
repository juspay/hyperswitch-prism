#!/usr/bin/env bash
#
# Decides which certified scenarios this run should execute.
#
# A connector enrols itself by declaring, in its own specs.json, a scenario it
# proved against live credentials. There is no central opt-in list: the
# declaration is the enrolment, so a new integration is covered by the PR that
# adds it, and adding a flow or payment method later touches only that
# connector's file rather than shared state.
#
# Inputs (environment):
#   RUST_CORE_CHANGED    "true" when core/harness sources changed
#   PROTO_CHANGED        "true" when proto definitions changed
#   CHANGED_CONNECTORS   comma-separated connector names touched by this PR
#   SPECS_ROOT           override for the connector_specs directory (tests)
#   GITHUB_OUTPUT        step output file (optional)
#
# Emits a JSON array of {name, suite, scenario, skip_dependencies} on stdout,
# and as the `targets` step output when GITHUB_OUTPUT is set.

set -uo pipefail

SPECS_ROOT="${SPECS_ROOT:-crates/internal/integration-tests/src/connector_specs}"

# Shared code has the widest blast radius, so it certifies every enrolled
# connector rather than skipping certification. That is affordable because a
# scenario costs seconds against an already-built binary, and safe for unrelated
# PRs because certify-connectors.sh only fails the build for a scenario it can
# show passes at the merge base.
shared_changed="false"
if [[ "${RUST_CORE_CHANGED:-}" == "true" || "${PROTO_CHANGED:-}" == "true" ]]; then
  shared_changed="true"
fi

changed=",${CHANGED_CONNECTORS:-},"
targets="[]"

for specs in "${SPECS_ROOT}"/*/specs.json; do
  [[ -f "${specs}" ]] || continue
  name="$(basename "$(dirname "${specs}")")"

  if [[ "${shared_changed}" != "true" && "${changed}" != *",${name},"* ]]; then
    continue
  fi

  entries=$(jq -c --arg n "${name}" '
    [ (.verified_scenarios // [])[]
      | select(.has_live_creds == true)
      | { name: $n, suite, scenario,
          skip_dependencies: (.skip_dependencies // false) } ]' "${specs}")

  targets=$(jq -c -s 'add' <<< "${targets}"$'\n'"${entries}")
done

if [[ "${shared_changed}" == "true" ]]; then
  echo "Shared code changed — selecting every certified connector" >&2
else
  echo "Selecting certified connectors touched by this PR" >&2
fi

[[ -n "${GITHUB_OUTPUT:-}" ]] && echo "targets=${targets}" >> "${GITHUB_OUTPUT}"
echo "${targets}"
