#!/usr/bin/env bash
set -euo pipefail
worker_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
container_runtime="${UCS_BROWSER_CONTAINER_RUNTIME:-docker}"
case "$container_runtime" in
  docker|podman) ;;
  *) echo "Use docker or podman for browser isolation." >&2; exit 2 ;;
esac
exec "$container_runtime" build --tag "${UCS_BROWSER_WORKER_IMAGE:-prism-browser-worker:local}" "$worker_root"
