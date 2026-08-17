#!/usr/bin/env bash
#
# Runs live-sandbox certification scenarios and decides, for each failure,
# whether this PR is responsible for it.
#
# A scenario that fails on HEAD is re-run at the PR's merge base. It fails the
# build only if it passes at the merge base and still fails on HEAD. Anything
# already broken before this PR — a sandbox outage, a connector-side behaviour
# change, a pre-existing regression — is reported and does not block. Without
# this, a PR touching shared code would be blocked by a connector its author
# never touched, and one connector's bad day would stop every merge.
#
# Inputs (environment):
#   TARGETS_JSON              JSON array of {name, suite, scenario, skip_dependencies}
#   BASE_SHA                  merge base to arbitrate against; empty disables arbitration
#   CONNECTOR_AUTH_FILE_PATH  decrypted connector credentials
#   ATTEMPT_TIMEOUT           per-attempt timeout in seconds (default 300)
#
# Exit status: 0 unless at least one scenario is proven to regress at HEAD.

set -uo pipefail

ATTEMPT_TIMEOUT="${ATTEMPT_TIMEOUT:-300}"
BASE_SHA="${BASE_SHA:-}"
SUMMARY="${GITHUB_STEP_SUMMARY:-/dev/null}"

HEAD_SHA="$(git rev-parse HEAD)"
BASE_PREPARED=false

# Names are only ever read from the manifest, but they end up in `jq --arg`
# lookups and file paths, so keep them to the shape a connector name can have.
valid_name() { [[ "$1" =~ ^[a-z0-9_]+$ ]]; }

# Only the sources are swapped for arbitration, never the tooling: this script
# and scripts/run-tests must stay at their HEAD versions. A full checkout would
# rewrite this file while bash is still reading it, and would revert run-tests
# to a version that does not understand the flags used below.
CHECKOUT_PATHS=(crates config Cargo.toml Cargo.lock)

# Paths that exist at $1, so `git checkout` is never handed a pathspec the
# commit does not know about — that fails the whole checkout, and arbitration
# would degrade to "inconclusive" for a reason that has nothing to do with the
# connector.
paths_present_at() {
  local sha="$1" p
  for p in "${CHECKOUT_PATHS[@]}"; do
    if git ls-tree --name-only "${sha}" -- "${p}" | grep -q .; then
      printf '%s\n' "${p}"
    fi
  done
}

restore_head() {
  if [[ "${BASE_PREPARED}" == "true" ]]; then
    echo "Restoring HEAD sources (${HEAD_SHA})"
    mapfile -t head_paths < <(paths_present_at "${HEAD_SHA}")
    if [[ ${#head_paths[@]} -gt 0 ]]; then
      git checkout --force "${HEAD_SHA}" -- "${head_paths[@]}" >/dev/null 2>&1 || true
    fi
    # The sources are back at HEAD but the binaries were built from the merge
    # base. Remove them rather than rebuild: any later step using --no-build
    # then fails loudly instead of silently testing merge-base code.
    rm -f target/debug/grpc-server target/debug/test_ucs
  fi
}
trap restore_head EXIT

build_binaries() {
  cargo build --workspace --bins 2>&1 | tail -5
  return "${PIPESTATUS[0]}"
}

# Runs one scenario against whatever is currently checked out and built.
run_scenario() {
  local name="$1" suite="$2" scenario="$3" skip_deps="$4"
  local args=(--skip-setup --no-build --connector "${name}" --suite "${suite}"
              --scenario "${scenario}" --interface grpc --report)
  [[ "${skip_deps}" == "true" ]] && args+=(--skip-dependencies)
  timeout --kill-after=30s "${ATTEMPT_TIMEOUT}" ./scripts/run-tests "${args[@]}"
}

# Swaps the sources to the merge base and rebuilds, reusing the same working
# directory and target dir. Third-party dependencies are identical between the
# two commits, so only workspace crates recompile; a separate worktree would
# change every path-dependency fingerprint and force a cold build.
prepare_base() {
  [[ "${BASE_PREPARED}" == "true" ]] && return 0
  echo "::group::Preparing merge base ${BASE_SHA} for arbitration"
  if ! git cat-file -e "${BASE_SHA}^{commit}" 2>/dev/null; then
    echo "::warning::Merge base ${BASE_SHA} is not available in this checkout"
    echo "::endgroup::"
    return 1
  fi
  mapfile -t base_paths < <(paths_present_at "${BASE_SHA}")
  if [[ ${#base_paths[@]} -eq 0 ]]; then
    echo "::warning::Merge base ${BASE_SHA} contains none of: ${CHECKOUT_PATHS[*]}"
    echo "::endgroup::"
    return 1
  fi
  if ! git checkout --force "${BASE_SHA}" -- "${base_paths[@]}"; then
    echo "::warning::Could not check out merge base sources"
    echo "::endgroup::"
    return 1
  fi
  BASE_PREPARED=true
  if ! build_binaries; then
    echo "::warning::Merge base ${BASE_SHA} does not build"
    echo "::endgroup::"
    return 1
  fi
  echo "::endgroup::"
  return 0
}

count=$(jq 'length' <<< "${TARGETS_JSON}")
if [[ "${count}" -eq 0 ]]; then
  echo "No certification targets selected."
  exit 0
fi

# Certification was selected, so absent credentials are a hard failure here.
# Falling back to placeholder credentials would let a scenario "pass" without
# ever reaching the connector.
if [[ -z "${CONNECTOR_AUTH_FILE_PATH:-}" || ! -s "${CONNECTOR_AUTH_FILE_PATH}" ]]; then
  echo "::error::${count} connector scenario(s) selected for certification but no credentials file is available."
  exit 1
fi

echo "## Connector certification" >> "${SUMMARY}"
echo "" >> "${SUMMARY}"

# ── Pass 1: run every target at HEAD ─────────────────────────────────────────
declare -a FAILED=()
declare -a PASSED=()
declare -a NO_CREDS=()

for i in $(seq 0 $((count - 1))); do
  target=$(jq -c ".[$i]" <<< "${TARGETS_JSON}")
  name=$(jq -r '.name' <<< "${target}")
  suite=$(jq -r '.suite' <<< "${target}")
  scenario=$(jq -r '.scenario' <<< "${target}")
  skip_deps=$(jq -r '.skip_dependencies // false' <<< "${target}")

  if ! valid_name "${name}"; then
    echo "::error::Invalid connector name in certification manifest: '${name}'"
    exit 1
  fi

  if ! jq -e --arg c "${name}" 'has($c)' "${CONNECTOR_AUTH_FILE_PATH}" >/dev/null; then
    echo "::warning::No credentials for '${name}' — cannot certify"
    NO_CREDS+=("${name} (${suite} / ${scenario})")
    continue
  fi

  echo "::group::Certify ${name} (${suite} / ${scenario})"
  if run_scenario "${name}" "${suite}" "${scenario}" "${skip_deps}"; then
    PASSED+=("${name} (${suite} / ${scenario})")
  else
    FAILED+=("${target}")
  fi
  echo "::endgroup::"
done

# ── Pass 2: confirm each failure reproduces at HEAD ──────────────────────────
# A single flake would otherwise send us into an arbitration rebuild and, worse,
# could be read as a regression when the merge base happens to pass.
declare -a CONFIRMED=()
declare -a FLAKY=()

for target in ${FAILED[@]+"${FAILED[@]}"}; do
  name=$(jq -r '.name' <<< "${target}")
  suite=$(jq -r '.suite' <<< "${target}")
  scenario=$(jq -r '.scenario' <<< "${target}")
  skip_deps=$(jq -r '.skip_dependencies // false' <<< "${target}")

  echo "::group::Re-check ${name} (${suite} / ${scenario})"
  if run_scenario "${name}" "${suite}" "${scenario}" "${skip_deps}"; then
    echo "Passed on re-check — treating the first failure as a flake."
    FLAKY+=("${name} (${suite} / ${scenario})")
  else
    CONFIRMED+=("${target}")
  fi
  echo "::endgroup::"
done

# ── Pass 3: arbitrate confirmed failures against the merge base ──────────────
declare -a REGRESSIONS=()
declare -a PRE_EXISTING=()
declare -a INCONCLUSIVE=()

if [[ ${#CONFIRMED[@]} -gt 0 ]]; then
  if [[ -z "${BASE_SHA}" ]]; then
    for target in "${CONFIRMED[@]}"; do
      INCONCLUSIVE+=("$(jq -r '"\(.name) (\(.suite) / \(.scenario))"' <<< "${target}") — no merge base to compare against")
    done
  elif ! prepare_base; then
    for target in "${CONFIRMED[@]}"; do
      INCONCLUSIVE+=("$(jq -r '"\(.name) (\(.suite) / \(.scenario))"' <<< "${target}") — merge base unavailable")
    done
  else
    for target in "${CONFIRMED[@]}"; do
      name=$(jq -r '.name' <<< "${target}")
      suite=$(jq -r '.suite' <<< "${target}")
      scenario=$(jq -r '.scenario' <<< "${target}")
      skip_deps=$(jq -r '.skip_dependencies // false' <<< "${target}")
      label="${name} (${suite} / ${scenario})"

      echo "::group::Arbitrate ${label} at merge base"
      if run_scenario "${name}" "${suite}" "${scenario}" "${skip_deps}"; then
        REGRESSIONS+=("${label}")
      else
        PRE_EXISTING+=("${label}")
      fi
      echo "::endgroup::"
    done
  fi
fi

# ── Report ───────────────────────────────────────────────────────────────────
{
  for x in ${PASSED[@]+"${PASSED[@]}"};       do echo "- ✅ **passed** — ${x}"; done
  for x in ${FLAKY[@]+"${FLAKY[@]}"};         do echo "- 🔁 **flaky** — ${x} (failed once, passed on re-check)"; done
  for x in ${PRE_EXISTING[@]+"${PRE_EXISTING[@]}"}; do echo "- ⚠️ **already failing before this PR** — ${x}"; done
  for x in ${NO_CREDS[@]+"${NO_CREDS[@]}"};   do echo "- ⚠️ **not certified** — ${x}: no credentials in CI"; done
  for x in ${INCONCLUSIVE[@]+"${INCONCLUSIVE[@]}"}; do echo "- ⚠️ **inconclusive** — ${x}"; done
  for x in ${REGRESSIONS[@]+"${REGRESSIONS[@]}"};   do echo "- ❌ **regressed in this PR** — ${x}"; done
} >> "${SUMMARY}"

for x in ${PRE_EXISTING[@]+"${PRE_EXISTING[@]}"}; do
  echo "::warning::${x} was already failing at the merge base — not attributed to this PR. It stays broken in main until someone fixes it; the nightly gate is what holds that line."
done
for x in ${INCONCLUSIVE[@]+"${INCONCLUSIVE[@]}"}; do
  echo "::warning::Could not determine whether this PR caused the failure: ${x}"
done

if [[ ${#REGRESSIONS[@]} -gt 0 ]]; then
  for x in "${REGRESSIONS[@]}"; do
    echo "::error::${x} passes at the merge base and fails here — this PR breaks it."
  done
  exit 1
fi

echo "No certification regression attributable to this PR."
exit 0
