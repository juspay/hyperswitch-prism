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
	@cd $(SDK_ROOT) && cargo build --release --target $(PLATFORM)

# ---------------------------------------------------------------------------
# Package / Distribution
# ---------------------------------------------------------------------------

package: ## Package the Rust SDK (preparation for publishing)
	@cd $(SDK_ROOT) && cargo package --allow-dirty

dist: package ## Build distribution package (alias for package)

# ---------------------------------------------------------------------------
# Testing
# ---------------------------------------------------------------------------

SMOKE_TEST_BIN := $(REPO_ROOT)/target/$(PLATFORM)/$(PROFILE)/hyperswitch-smoke-test
WEBHOOK_SMOKE_TEST_BIN := $(REPO_ROOT)/target/$(PLATFORM)/$(PROFILE)/smoke-test-webhook

# In CI (GITHUB_ACTIONS set), use cached binaries if available. Locally, always use cargo run.
test-package: build-ffi-lib ## Build FFI → Run Rust smoke test (+ webhook smoke)
	@echo "Running Rust smoke test..."
	@if [ -n "$(GITHUB_ACTIONS)" ] && [ -f "$(SMOKE_TEST_BIN)" ]; then \
		echo "Using cached smoke test binary (CI mode)..."; \
		cd $(REPO_ROOT) && HARNESS_DIR=$(SDK_ROOT)/../../examples CONNECTORS=$(CONNECTORS) $(SMOKE_TEST_BIN) --connectors $(CONNECTORS) 2>&1; \
	else \
		cd $(REPO_ROOT) && HARNESS_DIR=$(SDK_ROOT)/../../examples CONNECTORS=$(CONNECTORS) cargo run --profile $(PROFILE) --target $(PLATFORM) -p hyperswitch-smoke-test -- --connectors $(CONNECTORS) 2>&1; \
	fi
	@echo "Running webhook smoke test..."
	@if [ -n "$(GITHUB_ACTIONS)" ] && [ -f "$(WEBHOOK_SMOKE_TEST_BIN)" ]; then \
		echo "Using cached webhook smoke test binary (CI mode)..."; \
		cd $(REPO_ROOT) && $(WEBHOOK_SMOKE_TEST_BIN) 2>&1; \
	else \
		cd $(REPO_ROOT) && cargo run --profile $(PROFILE) --target $(PLATFORM) -p hyperswitch-smoke-test --bin smoke-test-webhook 2>&1; \
	fi

test-package-mock: build-ffi-lib ## Build FFI → Run Rust smoke test in MOCK mode (uses examples/connector/connector.rs)
	@echo "Running Rust smoke test in MOCK mode..."
	@if [ ! -f $(REPO_ROOT)/creds_dummy.json ]; then \
		echo "ERROR: creds_dummy.json not found"; \
		exit 1; \
	fi
	@if [ -n "$(GITHUB_ACTIONS)" ] && [ -f "$(SMOKE_TEST_BIN)" ]; then \
		echo "Using cached smoke test binary (CI mode)..."; \
		cd $(REPO_ROOT) && HARNESS_DIR=$(SDK_ROOT)/../../examples CONNECTORS=$(CONNECTORS) $(SMOKE_TEST_BIN) --creds-file $(REPO_ROOT)/creds.json --connectors $(CONNECTORS) --mock 2>&1; \
	else \
		cd $(REPO_ROOT) && HARNESS_DIR=$(SDK_ROOT)/../../examples CONNECTORS=$(CONNECTORS) cargo run --profile $(PROFILE) --target $(PLATFORM) -p hyperswitch-smoke-test -- --creds-file $(REPO_ROOT)/creds.json --connectors $(CONNECTORS) --mock 2>&1; \
	fi

# gRPC variables — defined before all gRPC targets
GRPC_CONNECTOR      ?= stripe
GRPC_AUTH_TYPE      ?= header-key
GRPC_API_KEY        ?=
GRPC_ENDPOINT       ?= http://localhost:8000
GRPC_PROFILE        ?= release-fast
# Deferred `=` so $(GRPC_PROFILE) is resolved at use-time (after ?= takes effect)
GRPC_SERVER_BIN      = $(REPO_ROOT)/target/$(PLATFORM)/$(GRPC_PROFILE)/grpc-server
GRPC_SMOKE_TEST_BIN  = $(REPO_ROOT)/target/$(PLATFORM)/$(GRPC_PROFILE)/grpc-smoke-test

# test-grpc-no-server
# Runs the pre-built grpc-smoke-test binary directly — no `cargo run` so Cargo
# does not re-evaluate the dep graph for a single package and cause feature
# de-unification (which recompiles all external deps from scratch).
# If the binary is absent (standalone invocation), build all gRPC packages
# together first to ensure correct feature unification.
test-grpc-no-server:
	@echo "Running gRPC smoke test..."
	@if [ ! -f "$(GRPC_SMOKE_TEST_BIN)" ]; then \
		echo "Binary not found — building all gRPC packages together..."; \
		cd $(REPO_ROOT) && cargo build -p grpc-server -p hyperswitch-grpc-ffi -p ffi -p hyperswitch-smoke-test -p uniffi-bindgen \
			--profile $(GRPC_PROFILE) --target $(PLATFORM); \
	fi
	@cd $(REPO_ROOT) && $(GRPC_SMOKE_TEST_BIN) --connectors $(GRPC_CONNECTOR)

# test-grpc
# Builds the grpc-server binary, starts it as a background process, runs
# the gRPC smoke test against it, then shuts it down.
#
# Usage:
#   make test-grpc
#   make test-grpc GRPC_CONNECTOR=adyen
#   make test-grpc GRPC_PROFILE=release

test-grpc: ## Build grpc-server → start in background → run gRPC smoke test → stop
	@echo "Building all gRPC packages ($(GRPC_PROFILE))…"
	@cd $(REPO_ROOT) && cargo build -p grpc-server -p hyperswitch-grpc-ffi -p ffi -p hyperswitch-smoke-test -p uniffi-bindgen \
		--profile $(GRPC_PROFILE) --target $(PLATFORM) 2>&1
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
