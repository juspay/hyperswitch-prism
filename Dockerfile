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
# 4. Minimal runtime dependencies
########################################
# Keep complete package payloads, licenses and version records for scanning.
# Do not overlay glibc or OpenSSL from another image onto distroless.
FROM public.ecr.aws/docker/library/debian:trixie-slim AS runtime-deps
WORKDIR /packages
RUN apt-get update \
    && apt-get download libgcc-s1 gcc-14-base \
    && mkdir -p /rootfs/var/lib/dpkg/status.d \
    && for package in ./*.deb; do \
         dpkg-deb --extract "$package" /rootfs; \
         name="$(dpkg-deb --field "$package" Package)"; \
         { dpkg-deb --field "$package"; printf 'Status: install ok installed\n'; } \
           > "/rootfs/var/lib/dpkg/status.d/$name"; \
       done \
    && rm -rf /var/lib/apt/lists/*

# The supplied release build needs glibc and libgcc, but no shared OpenSSL,
# libstdc++ or libgomp. CA certificates and timezone data come from distroless.
# Vendored OpenSSL remains part of the binary and must be audited separately.
FROM gcr.io/distroless/base-nossl-debian13:nonroot AS runtime-rootfs
COPY --from=runtime-deps /rootfs/ /

########################################
# 5. Check dependencies against the actual runtime filesystem
########################################
FROM builder AS runtime-validation
COPY --from=runtime-rootfs / /runtime-root/
COPY --from=builder /app/dist/grpc-server /runtime-root/app/bin/grpc-server
# Use the target runtime loader, not the builder's ldd/library search path.
# This fails the build if a new feature requires a missing shared library.
RUN set -eu; \
    loader="$(find /runtime-root -type f -name 'ld-linux-*.so.*' -print)"; \
    test -n "$loader"; \
    chroot /runtime-root "${loader#/runtime-root}" --list /app/bin/grpc-server

########################################
# 6. Runtime stage
########################################
FROM runtime-rootfs AS runtime

COPY --from=runtime-validation --chown=65532:65532 /runtime-root/app/bin/grpc-server /app/bin/grpc-server
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
