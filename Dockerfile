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
    && apt-get install -y \
       pkg-config \
       libssl-dev \
       g++ \
       make \
       perl \
       clang \
       libclang-dev \
    && apt-get clean \
    && rm -rf /var/lib/apt/lists/*

# Install cargo-chef and sccache for dependency caching
RUN cargo install cargo-chef --version ^0.1 \
    && cargo install sccache

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
RUN --mount=type=cache,target=/sccache \
    cargo chef cook --release --features kafka,connector-request-kafka,otel,log-transformations,deja --recipe-path recipe.json

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
    cargo build --release --features kafka,connector-request-kafka,otel,log-transformations,deja -p grpc-server

# Stage the compiled FileDescriptorSet the build emits: the deja replay driver
# (deja-kernel, KERNEL_DESCRIPTOR_SET) decodes recorded gRPC bodies with it.
# `ls -t | head -1` picks the real build's OUT_DIR over any chef-stub leftover.
RUN mkdir -p /app/dist \
    && cp "$(ls -t /app/target/release/build/grpc-api-types-*/out/connector_service_descriptor.bin | head -1)" /app/dist/

# Output sccache statistics
RUN sccache --show-stats

########################################
# 4. Runtime stage
########################################
FROM public.ecr.aws/docker/library/debian:bookworm-slim AS runtime
WORKDIR /app

# Install only runtime dependencies and clean up
# curl: probe outbound connector reachability from inside the pod.
RUN apt-get update \
    && apt-get install -y \
       libpq-dev \
       ca-certificates \
       curl \
    && apt-get clean \
    && rm -rf /var/lib/apt/lists/*

# Create a non-root user for security
RUN useradd -ms /bin/bash rustuser
RUN chown -R rustuser:rustuser /app
USER rustuser

# Copy built binary and config
RUN mkdir -p bin config
COPY --from=builder /app/target/release/grpc-server bin/grpc-server
COPY --from=builder /app/config config
# The descriptor set rides the image at a stable path so the replay harness can
# copy/mount it for deja-kernel (KERNEL_DESCRIPTOR_SET) without a prism checkout.
COPY --from=builder /app/dist/connector_service_descriptor.bin share/connector_service_descriptor.bin

ENTRYPOINT ["/app/bin/grpc-server"]
