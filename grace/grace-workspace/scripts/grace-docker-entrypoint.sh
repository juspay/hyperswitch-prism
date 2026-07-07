#!/usr/bin/env bash
# grace-docker-entrypoint.sh
#
# Lazily populates ${HOME}/.grace-docker-cli/ with linux builds of claude and
# opencode on first container start, then exec's CMD. Subsequent starts
# short-circuit (binaries already cached on the host-side bind mount).
#
# Why this exists: the host (macOS) and the container (linux) have different
# OS ABIs, so we can't bind-mount the host's claude/opencode binaries
# directly. Instead, we keep a *linux* install in a host-side cache dir
# (path-mirrored into the container at the same absolute path), populated
# once and reused across `docker compose down/up` cycles.

set -euo pipefail

CLI_CACHE="${HOME}/.grace-docker-cli"
CLAUDE_PREFIX="${CLI_CACHE}/claude-npm"
OPENCODE_DIR="${CLI_CACHE}/opencode"

# Pinned to match the versions the image used to bake in. Bump deliberately;
# `rm -rf ~/.grace-docker-cli` on the host forces a re-fetch on next up.
CLAUDE_VERSION="${CLAUDE_VERSION:-2}"
OPENCODE_VERSION="${OPENCODE_VERSION:-1.3.10}"

mkdir -p "${CLI_CACHE}"

# npm defaults its cache to ${HOME}/.npm. Inside the container, HOME is
# path-mirrored to the host HOME (e.g. /Users/tushar.shukla/), whose parent
# is root-owned at the overlay-FS level (it's only auto-created by Docker to
# host the bind mounts) — the non-root `node` user cannot mkdir there.
# Redirect both cache and userconfig into the bind-mounted CLI cache, which
# is writable.
export npm_config_cache="${CLI_CACHE}/.npm-cache"
export npm_config_userconfig="${CLI_CACHE}/.npmrc"
mkdir -p "${npm_config_cache}"

# `grace` and `opencode-serve` start near-simultaneously and share this
# bind-mounted cache. flock serializes them so only one container does the
# install; the other waits, acquires the lock, sees the binaries already
# present, and short-circuits past both blocks. The lock file lives inside
# the cache so it's visible to all containers via the shared mount.
exec 9>"${CLI_CACHE}/.bootstrap.lock"
flock -x 9

if [ ! -x "${CLAUDE_PREFIX}/bin/claude" ]; then
  echo "[grace-entrypoint] First run: installing claude @${CLAUDE_VERSION} into ${CLAUDE_PREFIX}…"
  # Clean any partial state from a previous interrupted/failed install.
  rm -rf "${CLAUDE_PREFIX}"
  mkdir -p "${CLAUDE_PREFIX}"
  # `-g` with `--prefix` lays out ${PREFIX}/bin/claude + ${PREFIX}/lib/node_modules/…
  # Without `-g`, npm uses the "local install" layout (${PREFIX}/node_modules/.bin/)
  # which doesn't match the `bin/claude` check above.
  npm install -g --prefix "${CLAUDE_PREFIX}" "@anthropic-ai/claude-code@${CLAUDE_VERSION}"
fi

# The opencode installer hardcodes $HOME/.opencode/ as its target. The real
# container HOME is path-mirrored to the host HOME, which on macOS already
# contains a *darwin* ~/.opencode/ — letting the installer write there would
# corrupt it. Sandbox the install with a temp HOME and move the result.
#
# Note on env-var scoping: `HOME=X curl … | bash` sets HOME only for curl —
# the bash on the right of the pipe inherits the *parent shell's* HOME and
# would still write to the real ~/.opencode/. The right-hand pipe segment is
# where the installer actually runs, so the env override must go there.
if [ ! -x "${OPENCODE_DIR}/bin/opencode" ]; then
  echo "[grace-entrypoint] First run: installing opencode ${OPENCODE_VERSION} into ${OPENCODE_DIR}…"
  TMP_HOME="$(mktemp -d)"
  curl -fsSL https://opencode.ai/install \
    | HOME="${TMP_HOME}" bash -s -- --version "${OPENCODE_VERSION}" >/dev/null
  mkdir -p "${OPENCODE_DIR}/bin"
  mv "${TMP_HOME}/.opencode/bin/opencode" "${OPENCODE_DIR}/bin/opencode"
  rm -rf "${TMP_HOME}"
fi

# Release the lock before exec'ing — the CMD runs without it.
flock -u 9
exec 9>&-

# Redirect XDG state/cache/config dirs into the writable cache mount. The
# defaults (${HOME}/.local/state, ${HOME}/.cache, ${HOME}/.config) live under
# the path-mirrored HOME, whose parent is root-owned at the overlay-FS layer
# and not writable by the non-root `node` user. opencode in particular dies
# at startup with EACCES on ${HOME}/.local/state if these aren't redirected.
# (XDG_DATA_HOME is left at its default so opencode still finds its auth at
# ${HOME}/.local/share/opencode, which compose bind-mounts read-only.)
export XDG_STATE_HOME="${CLI_CACHE}/xdg-state"
export XDG_CACHE_HOME="${CLI_CACHE}/xdg-cache"
export XDG_CONFIG_HOME="${CLI_CACHE}/xdg-config"
mkdir -p "${XDG_STATE_HOME}" "${XDG_CACHE_HOME}" "${XDG_CONFIG_HOME}"

export PATH="${CLAUDE_PREFIX}/bin:${OPENCODE_DIR}/bin:${PATH}"

exec "$@"
