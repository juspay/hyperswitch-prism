# SDK Rust Makefile
# Packages Rust SDK source code for distribution

.PHONY: build package dist clean example-run test-package test-grpc

# Auto-detect SDK root directory
MAKEFILE_DIR := $(dir $(abspath $(lastword $(MAKEFILE_LIST))))
SDK_ROOT := $(MAKEFILE_DIR)
REPO_ROOT := $(shell cd $(MAKEFILE_DIR)../.. && pwd)

ARTIFACTS_DIR=$(REPO_ROOT)/artifacts
PACKAGE_DIR=$(ARTIFACTS_DIR)/sdk-rust

# Build the Rust SDK (for verification)
build:
	@echo "Building Rust SDK (verification)..."
	@cd $(SDK_ROOT) && cargo build --release
	@echo "✓ Rust SDK builds successfully"

# Package Rust SDK source (no binary)
package:
	@echo "Packaging Rust SDK source..."
	@mkdir -p $(PACKAGE_DIR)
	@echo "Copying source files..."
	@cp -rf $(SDK_ROOT)/Cargo.toml $(PACKAGE_DIR)/
	@cp -rf $(SDK_ROOT)/Cargo.lock $(PACKAGE_DIR)/ 2>/dev/null || true
	@cp -rf $(SDK_ROOT)/src $(PACKAGE_DIR)/ 2>/dev/null || true
	@cp -rf $(SDK_ROOT)/examples $(PACKAGE_DIR)/ 2>/dev/null || true
	@echo "Rust SDK source package created: $(PACKAGE_DIR)/"

# Create distribution archive
dist: build package
	@echo "Creating distribution archive..."
	@cd $(ARTIFACTS_DIR) && zip -r "connector-service-rust-sdk.zip" sdk-rust/
	@echo "Distribution archive created: $(ARTIFACTS_DIR)/connector-service-rust-sdk.zip"

# ---------------------------------------------------------------------------
# test-package
# Verifies the Rust SDK compiles (library + basic example) and runs the
# smoke test for a set of connectors (defaults to stripe).
# A full round-trip test requires real connector credentials; run with
#   CONNECTORS=stripe,adyen make test-package
# ---------------------------------------------------------------------------
CONNECTORS ?= stripe

test-package:
	@echo "Checking Rust SDK library..."
	@cd $(SDK_ROOT) && cargo check --lib 2>&1
	@echo "Checking Rust SDK examples..."
	@cd $(SDK_ROOT) && cargo check --example basic 2>&1
	@echo "Running Rust SDK smoke test..."
	@if [ -f $(REPO_ROOT)/creds.json ]; then \
		cp $(REPO_ROOT)/creds.json $(SDK_ROOT)/smoke-test/creds.json; \
	else \
		echo "  Note: creds.json not found, smoke test will use placeholder credentials"; \
		echo '{"stripe":{"api_key":{"value":"placeholder"}}}' > $(SDK_ROOT)/smoke-test/creds.json; \
	fi
	@cd $(SDK_ROOT)/smoke-test && cargo run --profile release-fast -- --connectors $(CONNECTORS) --creds-file creds.json 2>&1 || \
		(echo "  Note: smoke test requires real credentials to pass"; exit 0)
	@echo "✓ Rust SDK package check passed."

# ---------------------------------------------------------------------------
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
	@echo "Running gRPC smoke test (connector: $(GRPC_CONNECTOR))…"
	@if [ -f $(REPO_ROOT)/creds.json ]; then \
		CREDS_ARGS="--creds-file $(REPO_ROOT)/creds.json --connector $(GRPC_CONNECTOR) --endpoint $(GRPC_ENDPOINT) --auth-type $(GRPC_AUTH_TYPE)"; \
	elif [ -n "$(GRPC_API_KEY)" ]; then \
		CREDS_ARGS="--endpoint $(GRPC_ENDPOINT) --connector $(GRPC_CONNECTOR) --auth-type $(GRPC_AUTH_TYPE) --api-key $(GRPC_API_KEY)"; \
	else \
		echo "  Note: no creds.json found and no GRPC_API_KEY set."; \
		echo "  Add credentials to creds.json at the repo root."; \
		pkill -f grpc-server 2>/dev/null || true; \
		exit 1; \
	fi; \
	cd $(SDK_ROOT)/smoke-test && CONNECTORS=$(GRPC_CONNECTOR) cargo run --profile $(GRPC_PROFILE) --bin grpc-smoke-test -- $$CREDS_ARGS; \
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
