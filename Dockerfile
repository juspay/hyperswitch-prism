# Enable buildkit features
# syntax = docker/dockerfile:1.4

########################################
# 1. Base image with necessary tools
########################################
# Migrated to Alpine 3.21 for SCA compliance (zero CVEs), matching unified-authentication-service.
FROM public.ecr.aws/docker/library/rust:alpine3.21 AS base

# Install system dependencies and clean up.
# - build-base, musl-dev, pkgconf: C/C++ compilation tools
# - openssl-dev, openssl-libs-static: OpenSSL headers and static libraries
# - zlib-dev, zlib-static: zlib dependencies (upgrades to 1.4-r0 for CVE-2026-4738)
# - perl, clang, clang-dev: zstd-sys / bindgen build-time requirement
# - protobuf-dev, protoc: gRPC protobuf compilation
# - libpq-dev: PostgreSQL client library headers
# - bash: needed by rdkafka-sys configure script
# - findutils: provides GNU find for -printf support
# - sccache: fast compilation caching
RUN apk --no-cache upgrade \
    && apk add --no-cache \
       build-base \
       musl-dev \
       pkgconf \
       openssl-dev \
       openssl-libs-static \
       zlib-dev \
       zlib-static \
       perl \
       clang \
       clang-dev \
       protobuf-dev \
       protoc \
       libpq-dev \
       bash \
       findutils \
       sccache

# Dynamic linking on musl enables dlopen for bindgen/libclang and produces dynamically linked binary
ENV RUSTFLAGS="-C target-feature=-crt-static"

# Install cargo-chef for dependency caching
RUN cargo install cargo-chef --version ^0.1

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

# Shared libraries the binary needs at runtime; the Alpine runtime must
# provide every one of them (check this output when changing features).
RUN ldd /app/dist/grpc-server

# Output sccache statistics
RUN sccache --show-stats

########################################
# 4. Runtime stage
########################################
# Alpine 3.21 runtime for SCA compliance (zero CVEs), matching unified-authentication-service.
# Packages upgraded to latest patch releases (e.g. zlib 1.4-r0).
FROM public.ecr.aws/docker/library/alpine:3.21 AS runtime

RUN apk --no-cache upgrade \
    && apk add --no-cache \
       ca-certificates \
       tzdata \
       libpq \
       libssl3 \
       libcrypto3 \
       libgcc \
       libstdc++

# Create non-root user (uid 65532 matches standard nonroot convention)
RUN adduser -D -u 65532 -s /sbin/nologin rustuser

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
