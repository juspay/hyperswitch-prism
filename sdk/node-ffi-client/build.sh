#!/bin/bash
set -e

# Navigate to project root (go up 2 directories from this script)
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

# Build release binary
cd "$PROJECT_ROOT/crates/ffi/ffi"
# cargo build --release --features napi
# cargo build --features napi
npm i && npm run build:debug



# Create artifacts directory
mkdir -p "$PROJECT_ROOT/sdk/node-ffi-client/artifacts"

# Copy binary to artifacts folder with .node extension
if [ -f "$PROJECT_ROOT/target/debug/libffi.dylib" ]; then
    cp "$PROJECT_ROOT/target/debug/libffi.dylib" \
       "$PROJECT_ROOT/sdk/node-ffi-client/artifacts/ffi.node"
elif [ -f "$PROJECT_ROOT/target/release/libffi.so" ]; then
    cp "$PROJECT_ROOT/target/release/libffi.so" \
       "$PROJECT_ROOT/sdk/node-ffi-client/artifacts/ffi.node"
elif [ -f "$PROJECT_ROOT/target/release/ffi.dll" ]; then
    cp "$PROJECT_ROOT/target/release/ffi.dll" \
       "$PROJECT_ROOT/sdk/node-ffi-client/artifacts/ffi.node"
else
    echo "Error: Native binary not found in target/release/"
    exit 1
fi

echo "Build complete: connector_service_ffi.node → sdk/node-ffi-client/artifacts/"