# Multi-stage build for the twin Axum service.
#
# The workspace does not pin a specific Rust toolchain (no rust-toolchain.toml).
# Mirror the root Dockerfile and track the latest stable slim image.

FROM rust:slim-bookworm AS builder
WORKDIR /app

# protobuf-compiler is required by upstream workspace crates that get pulled in
# during dependency resolution even when building only -p twin.
RUN apt-get update \
 && apt-get install -y --no-install-recommends \
      pkg-config \
      libssl-dev \
      protobuf-compiler \
      ca-certificates \
 && rm -rf /var/lib/apt/lists/*

COPY . .
RUN cargo build -p twin --release

FROM debian:bookworm-slim
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates \
 && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/twin /usr/local/bin/twin

ENV TWIN_BIND=0.0.0.0:8777
EXPOSE 8777

ENTRYPOINT ["/usr/local/bin/twin"]
