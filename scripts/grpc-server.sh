#!/usr/bin/env bash
#
# Start and stop the gRPC server used by the test runner.
#
# Sourced rather than executed, so callers can own the lifecycle across several
# runs instead of paying a start and a shutdown per scenario.
#
# The server serves whatever binary existed when it started. A caller that
# rebuilds — certification swaps to merge-base sources and rebuilds there — MUST
# stop and start it again, or the newly built code is never the code under test.

GRPC_SERVER_PID=""

# grpc_server_start <binary> <host> <port>
grpc_server_start() {
  local binary="$1" host="$2" port="$3"

  if [[ ! -x "${binary}" ]]; then
    echo "gRPC server binary not found at ${binary}" >&2
    return 1
  fi

  # Sandboxes fail slowly: fiserv holds a dead call for 23s before returning 504.
  # A healthy call there is ~2s, so 10s is ample and stops one bad connector
  # eating the whole sweep budget. Override with CS__PROXY__CONNECTOR_REQUEST_TIMEOUT.
  CS__SERVER__HOST="${host}" \
  CS__SERVER__PORT="${port}" \
  CS__COMMON__ENVIRONMENT=development \
  CS__PROXY__CONNECTOR_REQUEST_TIMEOUT="${CS__PROXY__CONNECTOR_REQUEST_TIMEOUT:-10}" \
  RUST_LOG=error \
  RUST_MIN_STACK="${RUST_MIN_STACK:-16777216}" \
    "${binary}" > /dev/null 2>&1 &
  GRPC_SERVER_PID=$!

  for _ in $(seq 1 40); do
    if nc -z 127.0.0.1 "${port}" 2>/dev/null; then
      return 0
    fi
    # Nothing is listening and the process is already gone: it failed to boot,
    # so waiting out the remaining attempts only delays the error.
    if ! kill -0 "${GRPC_SERVER_PID}" 2>/dev/null; then
      echo "gRPC server exited before becoming ready" >&2
      GRPC_SERVER_PID=""
      return 1
    fi
    sleep 0.5
  done

  echo "gRPC server did not become ready on port ${port} within 20s" >&2
  grpc_server_stop
  return 1
}

grpc_server_stop() {
  [[ -n "${GRPC_SERVER_PID}" ]] || return 0

  kill "${GRPC_SERVER_PID}" 2>/dev/null || true

  # Bounded, then forced. An unbounded `wait` here held a CI job open for two
  # hours after the scenario had already passed: the server does not exit on
  # SIGTERM promptly, and nothing else was going to release the shell.
  for _ in $(seq 1 40); do
    kill -0 "${GRPC_SERVER_PID}" 2>/dev/null || break
    sleep 0.25
  done
  kill -9 "${GRPC_SERVER_PID}" 2>/dev/null || true

  GRPC_SERVER_PID=""
}

# Convenience for callers that rebuild between runs.
grpc_server_restart() {
  grpc_server_stop
  grpc_server_start "$@"
}
