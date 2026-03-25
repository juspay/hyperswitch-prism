#!/usr/bin/env bash
# Installs the pre-push credential check hook into .git/hooks/.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
HOOK_SRC="${SCRIPT_DIR}/pre-push-cred-check"
HOOK_DST="${REPO_ROOT}/.git/hooks/pre-push"

if [ ! -f "${HOOK_SRC}" ]; then
    echo "error: hook source not found: ${HOOK_SRC}"
    exit 1
fi

if [ -f "${HOOK_DST}" ]; then
    echo "warning: ${HOOK_DST} already exists — backing up to ${HOOK_DST}.bak"
    cp "${HOOK_DST}" "${HOOK_DST}.bak"
fi

cp "${HOOK_SRC}" "${HOOK_DST}"
chmod +x "${HOOK_DST}"

echo "Installed pre-push hook: ${HOOK_DST}"
