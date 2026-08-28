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
#   CHANGED_CONNECTORS        comma-separated connector names touched by this PR
#   NEW_CONNECTORS            comma-separated names added by this PR (gated elsewhere)
#   SHARED_CHANGED            "true" when core, proto or harness sources changed
#   SPECS_ROOT                override for the connector_specs directory (tests)
#   BASE_SHA                  merge base to arbitrate against; empty disables arbitration
#   CONNECTOR_AUTH_FILE_PATH  decrypted connector credentials
#   ATTEMPT_TIMEOUT           whole-connector sweep timeout in seconds (default 300)
#   SCENARIO_TIMEOUT          single-scenario timeout in seconds (default 90)
#
# Exit status: 0 unless at least one scenario is proven to regress at HEAD.

set -uo pipefail

ATTEMPT_TIMEOUT="${ATTEMPT_TIMEOUT:-300}"
# One scenario plus its dependency chain is at most 7 calls, each bounded by the
# server's 10s per-call timeout. 300s was the whole-connector budget reused here.
SCENARIO_TIMEOUT="${SCENARIO_TIMEOUT:-90}"
SPECS_ROOT="${SPECS_ROOT:-crates/internal/integration-tests/src/connector_specs}"
ALPHA_FILE="${SPECS_ROOT}/alpha_connectors.json"
BASE_SHA="${BASE_SHA:-}"
SUMMARY="${GITHUB_STEP_SUMMARY:-/dev/null}"

HEAD_SHA="$(git rev-parse HEAD)"
BASE_PREPARED=false

GRPC_HOST="${GRPC_HOST:-0.0.0.0}"
GRPC_PORT="${GRPC_PORT:-8000}"
SERVER_BINARY="target/debug/grpc-server"

# shellcheck source=../../scripts/grpc-server.sh
source "scripts/grpc-server.sh"

# Listed in alpha_connectors.json: no credentials in CI, so nothing to certify.
is_alpha() {
  [[ -f "${ALPHA_FILE}" ]] || return 1
  jq -e --arg n "$1" '.connectors | has($n)' "${ALPHA_FILE}" >/dev/null 2>&1
}

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
trap 'grpc_server_stop; restore_head' EXIT

build_binaries() {
  cargo build --workspace --bins 2>&1 | tail -5
  return "${PIPESTATUS[0]}"
}
# Runs everything one connector declares; the framework derives suites and
# scenarios from its specs.json. Echoes one {name, suite, scenario, head_error}
# object per failed scenario.
run_connector() {
  local name="$1"
  local report="${RUNNER_TEMP:-/tmp}/certify-${name}.json"
  rm -f "${report}"

  local args=(--skip-setup --no-build --no-server --connector "${name}"
              --interface grpc --report)

  # The caller captures this function's stdout, so the run's own output goes to
  # stderr. Left on stdout it is captured too, and every line of it becomes a
  # bogus failure target — while the real scenario detail vanishes from the log.
  UCS_RUN_TEST_REPORT_PATH="${report}" \
    timeout --kill-after=30s "${ATTEMPT_TIMEOUT}" ./scripts/run-tests "${args[@]}" >&2
  local rc=$?

  local rows=""
  if [[ -s "${report}" ]]; then
    # The report is {"runs": [...]}, not a bare array. Reading it as an array
    # made jq error out, which `|| true` swallowed — so no scenario was ever
    # named and every failure degraded to a whole-connector one.
    rows=$(jq -c --arg n "${name}" '
      [ (.runs // .)[]? | select(.is_dependency != true and .assertion_result == "FAIL") ]
      | map({ name: $n, suite: .suite, scenario: .scenario,
              head_error: (.error // "") })
      | unique_by([.suite, .scenario])
      | .[]' "${report}" 2>/dev/null || true)
  fi

  if [[ -n "${rows}" ]]; then
    printf '%s\n' "${rows}"
    return 0
  fi

  # Non-zero exit with no failed row — a timeout or a crash. Reported against
  # the connector so it cannot read as a pass.
  if [[ "${rc}" -ne 0 ]]; then
    jq -cn --arg n "${name}" --arg rc "${rc}" \
      '{ name: $n, suite: "-", scenario: "-",
         head_error: ("the run failed with status " + $rc + " and named no scenario") }'
  fi
}

# Runs one scenario against whatever is currently checked out and built.
# Sets LAST_RC and LAST_ERROR for the caller.
run_scenario() {
  local name="$1" suite="$2" scenario="$3"
  local report="${RUNNER_TEMP:-/tmp}/certify-report.json"
  rm -f "${report}"

  local args=(--skip-setup --no-build --no-server --connector "${name}"
              --suite "${suite}" --scenario "${scenario}"
              --interface grpc --report)

  UCS_RUN_TEST_REPORT_PATH="${report}" \
    timeout --kill-after=30s "${SCENARIO_TIMEOUT}" ./scripts/run-tests "${args[@]}"
  LAST_RC=$?

  LAST_ERROR=""
  if [[ "${LAST_RC}" -ne 0 && -s "${report}" ]]; then
    LAST_ERROR=$(jq -r --arg s "${scenario}" '
      [ (.runs // .)[]? | select(.scenario == $s and .assertion_result == "FAIL") | .error // "" ]
      | first // ""' "${report}" 2>/dev/null || true)
  fi

  return "${LAST_RC}"
}

# Swaps the sources to the merge base and rebuilds, reusing the same working
# directory and target dir. Third-party dependencies are identical between the
# two commits, so only workspace crates recompile; a separate worktree would
# change every path-dependency fingerprint and force a cold build.
prepare_base() {
  [[ "${BASE_PREPARED}" == "true" ]] && return 0
  echo "::group::Preparing merge base ${BASE_SHA} for arbitration"
  # The workflow checks out at depth 1, so the merge base is normally absent.
  # Fetching just that commit is enough to check out sources from it, and far
  # cheaper than deepening the whole history for every run.
  if ! git cat-file -e "${BASE_SHA}^{commit}" 2>/dev/null; then
    git fetch --no-tags --depth=1 origin "${BASE_SHA}" >/dev/null 2>&1 || true
  fi
  if ! git cat-file -e "${BASE_SHA}^{commit}" 2>/dev/null; then
    echo "::warning::Merge base ${BASE_SHA} could not be fetched into this checkout"
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
  # The running server is still serving the HEAD binary. Left alone it would
  # answer every merge-base scenario with HEAD's code, and the comparison the
  # whole verdict rests on would be meaningless.
  if ! grpc_server_restart "${SERVER_BINARY}" "${GRPC_HOST}" "${GRPC_PORT}"; then
    echo "::warning::Could not restart the gRPC server on merge-base build"
    echo "::endgroup::"
    return 1
  fi
  echo "::endgroup::"
  return 0
}

# ── Which connectors ─────────────────────────────────────────────────────────
# Decided from git facts only; what each one runs comes from its specs.json.
declare -a CONNECTORS=()

if [[ "${SHARED_CHANGED:-}" == "true" ]]; then
  # All 100+ is the nightly's job; a PR clears the ones carrying real traffic.
  echo "Shared code changed — certifying connectors live in production"
  for specs in "${SPECS_ROOT}"/*/specs.json; do
    [[ -f "${specs}" ]] || continue
    [[ "$(jq -r '.live_in_production // false' "${specs}")" == "true" ]] || continue
    CONNECTORS+=("$(basename "$(dirname "${specs}")")")
  done
  if [[ ${#CONNECTORS[@]} -eq 0 ]]; then
    echo "::error::Shared code changed but no connector declares live_in_production — nothing would be certified."
    exit 1
  fi
else
  echo "Certifying connectors touched by this PR"
  IFS=',' read -ra TOUCHED <<< "${CHANGED_CONNECTORS:-}"
  for name in ${TOUCHED[@]+"${TOUCHED[@]}"}; do
    [[ -n "${name}" ]] || continue
    specs="${SPECS_ROOT}/${name}/specs.json"
    [[ -f "${specs}" ]] || continue

    # Run by verify-new-connectors.sh instead: it has no merge base to
    # arbitrate against, so every failure there is this PR's by definition.
    if [[ ",${NEW_CONNECTORS:-}," == *",${name},"* ]]; then
      echo "  ${name}: added by this PR — certified by the new-connector gate"
      continue
    fi

    if is_alpha "${name}"; then
      echo "  ${name}: listed in alpha_connectors.json — not certified by this PR"
      continue
    fi

    CONNECTORS+=("${name}")
  done
fi

count=${#CONNECTORS[@]}
if [[ "${count}" -eq 0 ]]; then
  echo "No connectors selected for certification."
  exit 0
fi
echo "Selected ${count} connector(s): ${CONNECTORS[*]}"

# Certification was selected, so absent credentials are a hard failure here.
# Falling back to placeholder credentials would let a scenario "pass" without
# ever reaching the connector.
if [[ -z "${CONNECTOR_AUTH_FILE_PATH:-}" || ! -s "${CONNECTOR_AUTH_FILE_PATH}" ]]; then
  echo "::error::${count} connector(s) selected for certification but no credentials file is available."
  exit 1
fi

# One server for the whole run, rather than a start and a shutdown wrapped
# around every scenario. prepare_base restarts it after rebuilding.
if ! grpc_server_start "${SERVER_BINARY}" "${GRPC_HOST}" "${GRPC_PORT}"; then
  echo "::error::Could not start the gRPC server for certification"
  exit 1
fi

echo "## Connector certification" >> "${SUMMARY}"
echo "" >> "${SUMMARY}"

# ── Pass 1: run every selected connector at HEAD ─────────────────────────────
declare -a FAILED=()
declare -a PASSED=()
declare -a NO_CREDS=()

for name in "${CONNECTORS[@]}"; do
  if ! valid_name "${name}"; then
    echo "::error::Invalid connector name: '${name}'"
    exit 1
  fi

  # Selected for certification and not listed as unproven, so the only reason
  # to be here without credentials is a mistake. Warning and exiting 0 would
  # certify nothing and read as a pass.
  if ! jq -e --arg c "${name}" 'has($c)' "${CONNECTOR_AUTH_FILE_PATH}" >/dev/null; then
    echo "::error::No credentials in CI for '${name}', so it cannot be certified. Add its credentials, or list it in ${ALPHA_FILE} with a reason."
    NO_CREDS+=("${name}")
    continue
  fi

  echo "::group::Certify ${name}"
  failures=$(run_connector "${name}")
  echo "::endgroup::"

  if [[ -z "${failures}" ]]; then
    PASSED+=("${name}")
    continue
  fi
  while IFS= read -r target; do
    [[ -n "${target}" ]] || continue
    FAILED+=("${target}")
  done <<< "${failures}"
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

  # Re-checked by rerunning the whole connector: a timeout is the transient
  # this pass exists to absorb.
  if [[ "${suite}" == "-" ]]; then
    echo "::group::Re-check ${name} (whole connector)"
    recheck=$(run_connector "${name}")
    echo "::endgroup::"
    if [[ -z "${recheck}" ]]; then
      echo "Passed on re-check — treating the first failure as a flake."
      FLAKY+=("${name}")
    else
      CONFIRMED+=("${target}")
    fi
    continue
  fi

  echo "::group::Re-check ${name} (${suite} / ${scenario})"
  if run_scenario "${name}" "${suite}" "${scenario}"; then
    echo "Passed on re-check — treating the first failure as a flake."
    FLAKY+=("${name} (${suite} / ${scenario})")
  else
    # Every confirmed failure is arbitrated, including an unreachable connector.
    # Skipping arbitration here would absolve a PR that made the connector
    # unreachable itself — a broken base_url reads as "availability" too, and
    # the merge base is the only thing that tells the two apart.
    CONFIRMED+=("$(jq -c --arg e "${LAST_ERROR}" \
                     '. + {head_error: $e}' <<< "${target}")")
  fi
  echo "::endgroup::"
done

# ── Pass 3: arbitrate confirmed failures against the merge base ──────────────
# Passing at the base and failing here is the only proof of a regression, so it
# is the only verdict that blocks. Everything else is reported with both errors.
declare -a REGRESSIONS=()
declare -a NOT_ATTRIBUTABLE=()

# "<label> — <why no baseline>" for a failure the merge base could not judge.
unattributed() {
  local target="$1" reason="$2" base_error="${3:-}"
  local label
  label="$(jq -r '"\(.name) (\(.suite) / \(.scenario))"' <<< "${target}")"
  local head_error
  head_error="$(jq -r '.head_error // ""' <<< "${target}")"
  local line="${label} — ${reason}"
  [[ -n "${head_error}" ]] && line+=" · here: ${head_error}"
  [[ -n "${base_error}" ]] && line+=" · at the merge base: ${base_error}"
  NOT_ATTRIBUTABLE+=("${line}")
}

if [[ ${#CONFIRMED[@]} -gt 0 ]]; then
  if [[ -z "${BASE_SHA}" ]]; then
    for target in "${CONFIRMED[@]}"; do
      unattributed "${target}" "no merge base to compare against"
    done
  elif ! prepare_base; then
    for target in "${CONFIRMED[@]}"; do
      unattributed "${target}" "merge base unavailable"
    done
  else
    for target in "${CONFIRMED[@]}"; do
      name=$(jq -r '.name' <<< "${target}")
      suite=$(jq -r '.suite' <<< "${target}")
      scenario=$(jq -r '.scenario' <<< "${target}")
      label="${name} (${suite} / ${scenario})"

      # No scenario to compare, so the whole connector runs at the base.
      if [[ "${suite}" == "-" ]]; then
        echo "::group::Arbitrate ${name} at merge base (whole connector)"
        base_failures=$(run_connector "${name}")
        echo "::endgroup::"
        if [[ -z "${base_failures}" ]]; then
          REGRESSIONS+=("${label}")
        else
          unattributed "${target}" "also fails at the merge base" \
            "$(jq -r '.head_error // ""' <<< "$(head -1 <<< "${base_failures}")")"
        fi
        continue
      fi

      echo "::group::Arbitrate ${label} at merge base"
      if run_scenario "${name}" "${suite}" "${scenario}"; then
        # Works without this PR, fails with it. The only verdict this design
        # can prove, and the only one that fails the build.
        REGRESSIONS+=("${label}")
      else
        unattributed "${target}" "also fails at the merge base" "${LAST_ERROR}"
      fi
      echo "::endgroup::"
    done
  fi
fi

# ── Report ───────────────────────────────────────────────────────────────────
{
  for x in ${PASSED[@]+"${PASSED[@]}"};   do echo "- ✅ **passed** — ${x}"; done
  for x in ${FLAKY[@]+"${FLAKY[@]}"};     do echo "- 🔁 **flaky** — ${x} (failed once, passed on re-check)"; done
  for x in ${NO_CREDS[@]+"${NO_CREDS[@]}"}; do echo "- ❌ **could not be certified** — ${x}: no credentials in CI"; done
  for x in ${NOT_ATTRIBUTABLE[@]+"${NOT_ATTRIBUTABLE[@]}"}; do echo "- ⚠️ **not attributable to this PR** — ${x}"; done
  for x in ${REGRESSIONS[@]+"${REGRESSIONS[@]}"}; do echo "- ❌ **regressed in this PR** — ${x}"; done
} >> "${SUMMARY}"

# These stay broken in main until someone fixes them; the nightly gate is what
# holds that line. Both errors are on the line so a reviewer can judge whether
# the two failures are really the same thing — this build does not guess.
for x in ${NOT_ATTRIBUTABLE[@]+"${NOT_ATTRIBUTABLE[@]}"}; do
  echo "::warning::Not attributable to this PR: ${x}"
done

blocking=0
for x in ${NO_CREDS[@]+"${NO_CREDS[@]}"}; do
  blocking=1
done
for x in ${REGRESSIONS[@]+"${REGRESSIONS[@]}"}; do
  echo "::error::${x} passes at the merge base and fails here — this PR breaks it."
  blocking=1
done

if [[ "${blocking}" -eq 1 ]]; then
  exit 1
fi

echo "No certification regression attributable to this PR."
exit 0
