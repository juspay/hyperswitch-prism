.PHONY: build package dist clean example-run test-package test-grpc test-grpc-no-server

# SDK-specific paths (define BEFORE including common.mk)
MAKEFILE_DIR := $(dir $(abspath $(lastword $(MAKEFILE_LIST))))
SDK_ROOT     := $(MAKEFILE_DIR)

# Connectors to test (comma-separated, default: stripe)
CONNECTORS ?= stripe

# Include common SDK build configuration
include $(SDK_ROOT)/../common.mk

# ---------------------------------------------------------------------------
# Build targets
# ---------------------------------------------------------------------------

build: ## Build the Rust SDK package
	@cd $(SDK_ROOT) && cargo build --release

# ---------------------------------------------------------------------------
# Package / Distribution
# ---------------------------------------------------------------------------

package: ## Package the Rust SDK (preparation for publishing)
	@cd $(SDK_ROOT) && cargo package --allow-dirty

dist: package ## Build distribution package (alias for package)

# ---------------------------------------------------------------------------
# Testing
# ---------------------------------------------------------------------------

test-package: build-ffi-lib ## Build FFI → Run Rust smoke test
	@echo "Running Rust smoke test..."
	@cd $(REPO_ROOT) && HARNESS_DIR=$(SDK_ROOT)/../../examples CONNECTORS=$(CONNECTORS) cargo run --profile $(PROFILE) -p hyperswitch-smoke-test -- --connectors $(CONNECTORS) 2>&1

test-package-mock: build-ffi-lib ## Build FFI → Run Rust smoke test in MOCK mode (uses examples/connector/connector.rs)
	@echo "Running Rust smoke test in MOCK mode..."
	@if [ ! -f $(REPO_ROOT)/creds_dummy.json ]; then \
		echo "ERROR: creds_dummy.json not found"; \
		exit 1; \
	fi
	@# Use examples directory (via HARNESS_DIR env var)
	@cd $(REPO_ROOT) && HARNESS_DIR=$(SDK_ROOT)/../../examples CONNECTORS=$(CONNECTORS) cargo run --profile $(PROFILE) -p hyperswitch-smoke-test -- --creds-file $(REPO_ROOT)/creds.json --connectors $(CONNECTORS) --mock 2>&1

# test-grpc
# Builds the grpc-server binary, starts it in background, runs gRPC smoke test binary.
test-grpc-no-server: build-ffi-lib build-grpc-ffi-lib
	@echo "Running gRPC smoke test..."
	@cd $(REPO_ROOT) && cargo run --profile $(PROFILE) -p hyperswitch-smoke-test --bin grpc-smoke-test -- --connectors $(GRPC_CONNECTOR) 2>&1 | tail -30

# test-grpc
# Builds the grpc-server binary, starts it as a background process, runs
# the gRPC smoke test against it, then shuts it down.
#
# Usage:
#   make test-grpc
#   make test-grpc GRPC_CONNECTOR=adyen
#   make test-grpc GRPC_PROFILE=release
# ---------------------------------------------------------------------------
GRPC_CONNECTOR  ?= stripe
GRPC_AUTH_TYPE  ?= header-key
GRPC_API_KEY    ?=
GRPC_ENDPOINT   ?= http://localhost:8000
GRPC_PROFILE    ?= release-fast
GRPC_SERVER_BIN := $(REPO_ROOT)/target/$(GRPC_PROFILE)/grpc-server

test-grpc: ## Build grpc-server → start in background → run gRPC smoke test → stop
	@echo "Building grpc-server ($(GRPC_PROFILE))…"
	@cd $(REPO_ROOT) && cargo build -p grpc-server --profile $(GRPC_PROFILE) 2>&1
	@echo "Starting gRPC server…"
	@pkill -KILL -f grpc-server 2>/dev/null || true
	@sleep 1
	@cd $(REPO_ROOT) && $(GRPC_SERVER_BIN) > /tmp/grpc-server.log 2>&1 &
	@$(MAKE) test-grpc-no-server; \
	EXIT=$$?; \
	pkill -f grpc-server 2>/dev/null || true; \
	exit $$EXIT

# Run the example
example-run:
	@cd $(SDK_ROOT) && STRIPE_API_KEY=$(STRIPE_API_KEY) cargo run --example basic --release

clean:
	@echo "Cleaning Rust SDK artifacts..."
	@cd $(SDK_ROOT) && cargo clean
	@rm -rf $(PACKAGE_DIR)
	@echo "Clean complete."
