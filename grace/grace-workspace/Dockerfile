# grace-workspace · build-once service container
#
# Companion to `make grace-workspace`. Use this when you don't want to install
# Node, claude CLI, gh, opencode on your host. See README.md's
# "Docker (alternative entry)" section for the full prereqs + workflow.
#
# Image baseline: node:20-bookworm-slim. Bookworm gives apt-installable git,
# sqlite3, gnupg (for the gh apt-repo signing dance), and a recent-enough libc
# for the opencode prebuilt binary.

FROM node:20-bookworm-slim

# ── OS deps ───────────────────────────────────────────────────────────────────
# git: worktrees + parity dashboard. sqlite3: ad-hoc state inspection.
# curl + ca-certificates: opencode installer downloads from github.com.
# gnupg: gh apt repo signing key. jq: shell-side JSON for setup.sh.
RUN apt-get update && apt-get install -y --no-install-recommends \
      git sqlite3 curl ca-certificates gnupg jq \
    && curl -fsSL https://cli.github.com/packages/githubcli-archive-keyring.gpg \
        | gpg --dearmor -o /usr/share/keyrings/githubcli-archive-keyring.gpg \
    && echo "deb [arch=$(dpkg --print-architecture) signed-by=/usr/share/keyrings/githubcli-archive-keyring.gpg] https://cli.github.com/packages stable main" \
        > /etc/apt/sources.list.d/github-cli.list \
    && apt-get update && apt-get install -y --no-install-recommends gh \
    && rm -rf /var/lib/apt/lists/*

# ── claude + opencode CLIs (host-cached, not baked in) ───────────────────────
# Both CLIs now live in a host-side cache dir (${HOME}/.grace-docker-cli/),
# bind-mounted in via docker-compose. The bootstrap script below populates the
# cache with linux builds on first container start, then short-circuits on
# subsequent runs. Versions stay pinned (claude @2 major, opencode 1.3.10)
# inside the script for reproducibility — bump there, then
# `rm -rf ~/.grace-docker-cli` on the host to refetch.
COPY scripts/grace-docker-entrypoint.sh /usr/local/bin/grace-docker-entrypoint.sh
RUN chmod +x /usr/local/bin/grace-docker-entrypoint.sh

# ── pnpm via corepack ─────────────────────────────────────────────────────────
# Matches the host bootstrap path; corepack ships with node:20.
#
# COREPACK_HOME is moved out of $HOME/.cache because at runtime we set
# HOME=/Users/<host-user> (for path-mirroring of ~/.tenxgrace etc.) and that
# parent dir is only writable where bind mounts cover it — corepack's own
# cache writes would otherwise fail with EACCES. /opt/corepack is owned by
# node:node after the chown below, so the non-root runtime user can read +
# write the pnpm cache normally.
ENV COREPACK_HOME=/opt/corepack
RUN mkdir -p /opt/corepack \
    && corepack enable \
    && corepack prepare pnpm@latest --activate

# ── Workspace install + build ─────────────────────────────────────────────────
WORKDIR /grace

# Copy lockfile + package manifests first so `pnpm install` is cached separately
# from the source tree — re-edits to source don't bust the dep layer.
COPY pnpm-lock.yaml pnpm-workspace.yaml package.json ./
COPY packages/core/package.json        packages/core/
COPY packages/cli/package.json         packages/cli/
COPY packages/dashboard/package.json   packages/dashboard/
COPY packages/parity-core/package.json packages/parity-core/

RUN pnpm install --frozen-lockfile

# Now copy the rest of the source and build.
COPY . .

# Dashboard's prebuild (packages/dashboard/scripts/parse-connectors.ts) reads
# `../../../../../docs-generated/all_connector.md`. From the script's location
# at /grace/packages/dashboard/scripts/, that normalizes to
# /docs-generated/all_connector.md. The file lives in the *parent* repo
# outside this build context — pull it in via docker-compose's
# `additional_contexts: repo-root: ../..` (or `docker build
# --build-context repo-root=../..`).
COPY --from=repo-root docs-generated/all_connector.md /docs-generated/all_connector.md

RUN pnpm build

# ── Non-root runtime user (BUG#7 fix) ─────────────────────────────────────────
# claude CLI refuses --dangerously-skip-permissions when running as root for
# security. The dashboard's connectorDiscoveryPlugin spawns claude with that
# flag, so the wizard's "Discover from web" feature was returning HTTP 500.
# Fix: switch to the `node` user (UID 1000) that's pre-created in the
# node:20-bookworm-slim base image.
#
# macOS Docker Desktop / OrbStack translate UIDs for bind mounts automatically,
# so UID mismatch between host (typically 501-502) and container (1000) is not
# an issue. On Linux hosts where this matters, override via:
#     docker compose build --build-arg HOST_UID=$(id -u) ...
# (Linux support is best-effort; matching host UID is a Linux-only concern.)
RUN chown -R node:node /grace /opt/corepack
USER node

# ── Runtime ───────────────────────────────────────────────────────────────────
EXPOSE 3141 3142

# Default CMD: supervisor + vite-dev dashboard, run concurrently in one
# container. Mirrors the process model of native `pnpm dev`.
#
# Why `vite dev` (not `vite preview`)? The dashboard's /api/* endpoints
# (parityApiPlugin, connectorDiscoveryPlugin in vite.config.ts) are
# Vite *dev-server* plugins — they only run with `vite dev`. With
# `vite preview` (static-server mode) they're absent and the endpoints
# return 404. The /api/* surface is used by the connector-discovery
# wizard and parity panels, so we need dev mode. Source watchers fire
# in this mode but the source is baked into the image, so they're idle.
# docker-compose overrides this CMD for the opencode-serve sidecar.
#
# ENTRYPOINT bootstraps claude + opencode into ${HOME}/.grace-docker-cli/ on
# first run, prepends their bin dirs to PATH, then exec's CMD. The sidecar
# inherits this entrypoint, so its `command:` override also benefits from the
# populated PATH (opencode is on PATH when `opencode serve` is invoked).
ENTRYPOINT ["/usr/local/bin/grace-docker-entrypoint.sh"]
CMD ["pnpm", "docker-up"]
