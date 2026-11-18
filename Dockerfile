# Enable buildkit features
# syntax = docker/dockerfile:1.4

########################################
# 1. Base image with necessary tools
########################################
FROM public.ecr.aws/docker/library/rust:slim-bookworm AS base

# Install system dependencies and clean up.
# clang/libclang-dev: zstd-sys (pulled by rdkafka's `zstd` feature, which the
# deja Kafka sink needs for compression.type=zstd) generates its bindings with
# bindgen, which loads libclang at build time.
RUN apt-get update \
    && apt-get upgrade -y --no-install-recommends \
    && apt-get install -y --no-install-recommends \
       pkg-config \
       libssl-dev \
       g++ \
       make \
       perl \
       clang \
       libclang-dev \
    && apt-get clean \
    && rm -rf /var/lib/apt/lists/*

# Install cargo-chef and sccache for dependency caching. sccache installs from
# its own Cargo.lock: resolved fresh it needs `zip ^0.6`, every version of
# which is yanked, and the install fails.
RUN cargo install cargo-chef --version ^0.1 \
    && cargo install sccache --locked

########################################
# 2. Planner stage (cargo-chef)
########################################
FROM base AS planner
WORKDIR /app
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

########################################
# 3. Builder stage
########################################
FROM base AS builder
WORKDIR /app

# Which cargo profile builds the shipped binary. `release` is the production
# build and the default, so a build that passes nothing gets exactly the image
# it always did. `release-fast` (Cargo.toml: release semantics, no LTO, parallel
# codegen, opt-level 1) exists for throwaway candidates such as replay runs,
# where a build that finishes in a fraction of the time matters more than the
# last few percent of runtime speed. Same code, same features, same behaviour;
# only optimisation effort differs. Incremental compilation is forced off: a
# fresh container has nothing to reuse and the profile's `incremental = true`
# would only cost time.
ARG CARGO_BUILD_PROFILE=release
ENV CARGO_INCREMENTAL=0

# Copy the prepared recipe and cook dependencies
COPY --from=planner /app/recipe.json ./recipe.json

# Configure sccache
ENV SCCACHE_DIR=/sccache
ENV SCCACHE_CACHE_SIZE=5G

# Cook dependencies using cargo-chef with caching.
# `deja` is baked into the image (CI has no per-build feature knob): the feature
# is fail-closed inert — deja.mode defaults to Disabled and every boundary is a
# pure passthrough — so this image behaves identically to a non-deja build until
# CS__DEJA__MODE=record|replay is set on the pod.
#
# Scoped to `grpc-server`, matching the `cargo build` below. Without `-p` this
# cooks dependencies for all 24 workspace members, including crates the image
# never ships — `ffi`, `uniffi-bindgen`, `integration-tests`, `field-probe`, the
# SDKs — and their build-only trees (`uniffi`, `proptest`, `inquire`, `rayon`).
# The cached layer is only ever consumed by the `grpc-server` build in the next
# stage, so cooking the rest costs build time and peak memory for nothing.
RUN --mount=type=cache,target=/sccache \
    cargo chef cook --profile ${CARGO_BUILD_PROFILE} --features kafka,connector-request-kafka,otel,log-transformations,deja --recipe-path recipe.json -p grpc-server

# Install additional build-time dependencies
RUN apt-get update \
    && apt-get install -y \
       protobuf-compiler \
       libpq-dev \
    && apt-get clean \
    && rm -rf /var/lib/apt/lists/*

# Build only the binary shipped by the runtime stage; skips test/SDK crates.
COPY . .
RUN --mount=type=cache,target=/sccache \
    cargo build --profile ${CARGO_BUILD_PROFILE} --features kafka,connector-request-kafka,otel,log-transformations,deja -p grpc-server

# Stage what the runtime stage ships under profile-independent paths: cargo
# writes the `dev` profile to target/debug and every other profile to
# target/<profile>, and the runtime stage must not have to know that. Also the
# compiled FileDescriptorSet the build emits: the deja replay driver
# (deja-kernel, KERNEL_DESCRIPTOR_SET) decodes recorded gRPC bodies with it.
# Found by name, not by path: cargo 1.100 moved `build/<pkg>-<hash>/out` to
# `build/<pkg>/<hash>/out`. Newest mtime picks the real build's OUT_DIR over
# any chef-stub leftover.
RUN PROFILE_DIR="$([ "${CARGO_BUILD_PROFILE}" = "dev" ] && echo debug || echo "${CARGO_BUILD_PROFILE}")" \
    && mkdir -p /app/dist \
    && cp "/app/target/${PROFILE_DIR}/grpc-server" /app/dist/grpc-server \
    && cp "$(find /app/target -type f -name connector_service_descriptor.bin -printf '%T@ %p\n' | sort -n | tail -1 | cut -d' ' -f2-)" /app/dist/

# Shared libraries the binary needs at runtime; the distroless runtime must
# provide every one of them (check this output when changing features).
RUN ldd /app/dist/grpc-server

# Output sccache statistics
RUN sccache --show-stats

########################################
# 4a. Security patcher: latest Debian 13 security patches
########################################
# Fetches the latest patched versions of system libraries from Debian 13
# (trixie) and stages them for overlay into the distroless runtime.
# Addresses 60 CVEs identified in container scan REF: ECR-2026-49.
FROM public.ecr.aws/docker/library/debian:trixie-slim AS security-patcher
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
       libc6 \
       libssl3t64 \
       libgcc-s1 \
       libstdc++6 \
       libgomp1 \
       ca-certificates \
    && apt-get upgrade -y \
    && rm -rf /var/lib/apt/lists/*

# Stage only the specific shared libraries that the scan flagged into /patch
# so we overlay the minimum set of files onto the distroless runtime.
RUN set -eux; \
    MA="$(ls -d /lib/*-linux-gnu | head -1 | xargs basename)"; \
    mkdir -p /patch/lib/${MA} /patch/usr/lib/${MA} /patch/etc/ssl; \
    # libc6: CVE-2026-5435, CVE-2026-84243, CVE-2026-19499, CVE-2026-6791, \
    #   CVE-2019-1010022 (CVSS 9.8), CVE-2019-1010023 (CVSS 8.8), and others \
    for f in libc.so.6 libm.so.6; do \
      [ -f "/lib/${MA}/$f" ] && cp -a "/lib/${MA}/$f" "/patch/lib/${MA}/"; \
    done; \
    for f in /lib/${MA}/ld-linux-*; do [ -e "$f" ] && cp -a "$f" /patch/lib/${MA}/; done; \
    # libssl3t64 (OpenSSL): CVE-2026-95691 (CVSS 8.1), CVE-2026-107493, \
    #   CVE-2026-124709, CVE-2026-88823, CVE-2026-95692, CVE-2026-107496, etc. \
    for f in /usr/lib/${MA}/libssl.so.3* /usr/lib/${MA}/libcrypto.so.3*; do \
      [ -e "$f" ] && cp -a "$f" /patch/usr/lib/${MA}/; \
    done; \
    # libgcc-s1, libstdc++6, libgomp1: CVE-2026-95619 (CVSS 7.7), \
    #   CVE-2026-102010, CVE-2026-114690, CVE-2026-114691, CVE-2026-115455, etc. \
    for f in /usr/lib/${MA}/libgcc_s.so.1 /usr/lib/${MA}/libstdc++.so.6* /usr/lib/${MA}/libgomp.so.1*; do \
      [ -e "$f" ] && cp -a "$f" /patch/usr/lib/${MA}/; \
    done; \
    # CA certificates \
    cp -a /etc/ssl/certs /patch/etc/ssl/

########################################
# 4b. Runtime stage
########################################
# Distroless Debian 13 (glibc + OpenSSL + libgcc/libstdc++, no shell, no apt,
# non-root uid 65532). The binary is built on bookworm (glibc 2.36) and runs on
# trixie (glibc 2.41). Bookworm has no fixes for the curl/glibc/util-linux/krb5
# CVEs, so the runtime ships none of those packages. curl and libpq-dev are
# gone: nothing in the workspace links libpq, and the in-pod connector probe
# should use `kubectl debug` with an ephemeral container instead of curl.
FROM gcr.io/distroless/cc-debian13:nonroot AS runtime

# Overlay security-patched shared libraries from the patcher stage.
# This addresses 60 CVEs (scan REF: ECR-2026-49) in libc6, libssl3t64,
# libgcc-s1, libstdc++6, and libgomp1 without leaving the distroless base.
COPY --from=security-patcher /patch/lib/ /lib/
COPY --from=security-patcher /patch/usr/lib/ /usr/lib/
COPY --from=security-patcher /patch/etc/ssl/certs/ /etc/ssl/certs/

COPY --from=builder --chown=65532:65532 /app/dist/grpc-server /app/bin/grpc-server
COPY --from=builder --chown=65532:65532 /app/config/development.toml /app/config/development.toml
COPY --from=builder --chown=65532:65532 /app/config/production.toml /app/config/production.toml
COPY --from=builder --chown=65532:65532 /app/config/sandbox.toml /app/config/sandbox.toml
COPY --from=builder --chown=65532:65532 /app/config/superposition.toml /app/config/superposition.toml
# The descriptor set rides the image at a stable path so the replay harness can
# copy/mount it for deja-kernel (KERNEL_DESCRIPTOR_SET) without a prism checkout.
COPY --from=builder --chown=65532:65532 /app/dist/connector_service_descriptor.bin /app/share/connector_service_descriptor.bin

WORKDIR /app
USER 65532:65532
ENTRYPOINT ["/app/bin/grpc-server"]
