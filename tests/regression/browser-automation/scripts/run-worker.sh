#!/usr/bin/env bash
# No execution API, host mounts, inherited container environment, or published ports.
set -euo pipefail

if [[ $# -ne 0 ]]; then
  echo "Supply one JSON job through stdin; CLI arguments are unsupported." >&2
  exit 2
fi
worker_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
container_runtime="${UCS_BROWSER_CONTAINER_RUNTIME:-docker}"
case "$container_runtime" in docker|podman) ;; *) echo "Use docker or podman for browser isolation." >&2; exit 2;; esac
container_name="${UCS_BROWSER_CONTAINER_NAME:-prism-browser-worker-${BASHPID}-${RANDOM}}"
worker_timeout="${WORKER_TIMEOUT_MS:-90000}"
if [[ ! "$worker_timeout" =~ ^[0-9]+$ ]] || (( worker_timeout < 1000 || worker_timeout > 300000 )); then
  echo "Invalid worker deadline." >&2
  exit 2
fi
worker_pid=""
watchdog_pid=""
cleanup() {
  trap - EXIT INT TERM
  if [[ -n "$watchdog_pid" ]]; then
    kill "$watchdog_pid" 2>/dev/null || true
    # The watchdog can send TERM and exit while Bash is already in wait.
    # A nested wait for it in this exit trap can block until the worker exits,
    # preventing the container removal that would terminate that worker.
    # Bash reaps the watchdog; never wait for it before removing the container.
  fi
  timeout --kill-after=1s 5s "$container_runtime" rm --force "$container_name" >/dev/null 2>&1 || true
  if [[ -n "$worker_pid" ]]; then
    kill -TERM "$worker_pid" 2>/dev/null || true
    wait "$worker_pid" 2>/dev/null || true
  fi
}
trap cleanup EXIT
trap 'exit 130' INT TERM

# Rust destructors do not run on SIGTERM. Detect a disappearing harness too.
launcher_pid="$BASHPID"
launcher_parent="$PPID"
(
  while kill -0 "$launcher_parent" 2>/dev/null; do sleep 0.2; done
  kill -TERM "$launcher_pid" 2>/dev/null || true
) &
watchdog_pid="$!"

timeout --kill-after=5s "$((worker_timeout / 1000 + 1))s" \
  "$container_runtime" run --rm --interactive --pull=never \
  --name "$container_name" \
  --read-only --user 10001:10001 --cap-drop=ALL \
  --security-opt "seccomp=${worker_root}/seccomp-profile.json" \
  --pids-limit 128 --memory 768m --cpus 1 --shm-size 128m \
  --tmpfs /tmp:rw,nosuid,nodev,size=256m,mode=1777 \
  --env HOME=/tmp --env "WORKER_TIMEOUT_MS=${worker_timeout}" \
  "${UCS_BROWSER_WORKER_IMAGE:-prism-browser-worker:local}" <&0 &
worker_pid="$!"
wait "$worker_pid"
