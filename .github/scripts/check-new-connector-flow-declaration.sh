#!/usr/bin/env bash
#
# New connectors must declare their flows through create_all_prerequisites!.
#
# check_connector_specs derives flow -> suite coverage from the macro's api:
# list, and prints [SKIP] for a connector that has no macro. A new connector
# without one therefore gets no flow coverage at all: nothing checks that the
# flows it implements appear in its specs.json. Requiring the macro is what
# brings the connector under that check.
#
# Scoped to connectors that did not exist at the PR's base commit. Existing
# connectors are left alone: some hand-write a flow because the macro cannot
# express it — two flows sharing a request or response type collide on the
# marker structs it emits, and a flow whose response type is decided at runtime
# does not fit the single ResponseBody a Bridge carries. Scoping to new code
# keeps this enforceable without an exception list.
#
# Inputs:
#   NEW_CONNECTORS  comma-separated connector names added in this PR
#
# Exit status: 0 unless a new connector is missing the macro or declares no flows.

set -uo pipefail

# Payment connectors only. check_connector_specs draws its universe from this
# directory alone, and every payout flow maps to None in flow_to_suites, so the
# macro buys no coverage there — 7 of the 9 payout connectors do not use it.
SRC_ROOT=crates/integrations/connector-integration/src/connectors

failures=0

if [[ -z "${NEW_CONNECTORS:-}" ]]; then
  echo "No newly added connectors."
  exit 0
fi

IFS=',' read -ra CONNECTORS <<< "${NEW_CONNECTORS}"

for name in "${CONNECTORS[@]}"; do
  [[ -n "${name}" ]] || continue

  # Names reach this script from paths in the diff, so keep them to the shape a
  # connector name can have before they are used to build a path.
  if [[ ! "${name}" =~ ^[a-z0-9_]+$ ]]; then
    echo "::error::Not a valid connector name: '${name}'"
    failures=$((failures + 1))
    continue
  fi

  # Names are extracted from paths, and "connectors/" also matches inside
  # "payout_connectors/", so a payout or authenticator connector can arrive
  # here. Those live outside this rule; skip rather than fail. A payment
  # connector with a spec directory and no source is already caught by
  # check_connector_specs' phase 1 parity check.
  src="${SRC_ROOT}/${name}.rs"
  if [[ ! -f "${src}" ]]; then
    echo "Skipping '${name}': no ${src} (not a payment connector)"
    continue
  fi

  if ! grep -q 'macros::create_all_prerequisites!(' "${src}"; then
    echo "::error::New connector '${name}' does not use create_all_prerequisites! in ${src}. check_connector_specs derives flow -> suite coverage from the macro's api: list and skips connectors that have none, so this connector's flows would never be checked against its specs.json. Declare them through the macro."
    failures=$((failures + 1))
    continue
  fi

  # An empty api: [] satisfies a presence check while declaring nothing.
  if ! grep -Pzoq 'macros::create_all_prerequisites!\((?s).*?api:\s*\[\s*\(' "${src}"; then
    echo "::error::New connector '${name}' calls create_all_prerequisites! with an empty api: [] in ${src}. Declare at least one flow."
    failures=$((failures + 1))
  fi
done

if [[ "${failures}" -gt 0 ]]; then
  echo "::error::${failures} new connector(s) failed the flow-declaration check"
  exit 1
fi

echo "All newly added connectors declare their flows through create_all_prerequisites!."
exit 0
