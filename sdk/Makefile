# SDK Build Makefile
# Builds FFI binaries for all SDK platforms
# This Makefile should be run from the repository root

.PHONY: help build build-all build-macos build-linux clean check-cargo generate install-deps \
        generate-descriptor generate-rust generate-flows-python generate-flows-javascript generate-flows-java \
        test test-rust test-grpc test-python test-javascript test-java \
        dist dist-python dist-javascript dist-java

# Default target
.DEFAULT_GOAL := help

# Include common SDK build configuration
MAKEFILE_DIR := $(dir $(abspath $(lastword $(MAKEFILE_LIST))))
SDK_ROOT     := $(MAKEFILE_DIR)
REPO_ROOT    := $(shell cd $(MAKEFILE_DIR).. && pwd)
include $(MAKEFILE_DIR)/common.mk

# Connectors to test (comma-separated, default: stripe)
CONNECTORS ?= stripe

# Target platforms (for cross-compilation)
TARGET_MACOS_x86_64 := x86_64-apple-darwin
TARGET_MACOS_AARCH64 := aarch64-apple-darwin
TARGET_LINUX_x86_64 := x86_64-unknown-linux-gnu
TARGET_LINUX_AARCH64 := aarch64-unknown-linux-gnu

# Detect current platform
UNAME := $(shell uname -s)
ARCH := $(shell uname -m)

help: ## Show this help message
	@echo "SDK Build Makefile - Available targets:"
	@echo ""
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-30s\033[0m %s\n", $$1, $$2}'
	@echo ""
	@echo "Current platform: $(UNAME) $(ARCH)"

# ---------------------------------------------------------------------------
# install-deps
# Installs Python build-time dependencies required for code generation.
# Checks for tool existence before installing. In Nix environments tools are
# provided by the shell so installation is skipped (IN_NIX_SHELL is set).
# Outside Nix, installs grpcio-tools and mypy-protobuf if absent.
# ---------------------------------------------------------------------------
install-deps: ## Install Python dependencies (skipped in Nix shell)
	@command -v python3 >/dev/null 2>&1 || \
		(echo "Error: python3 not found. Install via nix or your system package manager." && exit 1)
	@command -v cargo >/dev/null 2>&1 || \
		(echo "Error: cargo not found. Install Rust via rustup." && exit 1)
	@if [ -z "$${IN_NIX_SHELL:-}" ]; then \
		python3 -c "import grpc_tools" 2>/dev/null || \
			(echo "  → Installing grpcio-tools..." && \
			 pip3 install grpcio-tools --quiet 2>/dev/null || \
			 pip3 install grpcio-tools --user --quiet); \
		python3 -c "import mypy_protobuf" 2>/dev/null || \
			(echo "  → Installing mypy-protobuf..." && \
			 pip3 install mypy-protobuf --quiet 2>/dev/null || \
			 pip3 install mypy-protobuf --user --quiet); \
		python3 -c "import jinja2" 2>/dev/null || \
			(echo "  → Installing jinja2..." && \
			 pip3 install jinja2 --quiet 2>/dev/null || \
			 pip3 install jinja2 --user --quiet); \
	fi
	@echo "All required tools present."

build: check-cargo ## Build UniFFI library for current platform
	@echo "Building UniFFI library for current platform..."
	@cd $(FFI_CRATE) && cargo build --no-default-features --features uniffi --release
	@echo "Build complete: $(REPO_ROOT)/target/release/libconnector_service_ffi.$(LIB_EXT)"

# macOS builds
build-macos-x86_64: check-cargo ## Build for macOS Intel (x86_64)
	@echo "Building for macOS Intel (x86_64)..."
	@cd $(FFI_CRATE) && cargo build --no-default-features --features uniffi --release --target $(TARGET_MACOS_x86_64)
	@echo "Build complete: target/$(TARGET_MACOS_x86_64)/release/libconnector_service_ffi.dylib"

build-macos-aarch64: check-cargo ## Build for macOS Apple Silicon (aarch64)
	@echo "Building for macOS Apple Silicon (aarch64)..."
	@cd $(FFI_CRATE) && cargo build --no-default-features --features uniffi --release --target $(TARGET_MACOS_AARCH64)
	@echo "Build complete: target/$(TARGET_MACOS_AARCH64)/release/libconnector_service_ffi.dylib"

build-macos-universal: build-macos-x86_64 build-macos-aarch64 ## Build universal binary for macOS
	@echo "Creating universal binary for macOS..."
	@mkdir -p $(REPO_ROOT)/target/universal/release
	@lipo -create \
		$(REPO_ROOT)/target/$(TARGET_MACOS_x86_64)/release/libconnector_service_ffi.dylib \
		$(REPO_ROOT)/target/$(TARGET_MACOS_AARCH64)/release/libconnector_service_ffi.dylib \
		-output $(REPO_ROOT)/target/universal/release/libconnector_service_ffi.dylib
	@echo "Universal binary created: $(REPO_ROOT)/target/universal/release/libconnector_service_ffi.dylib"

build-macos: ## Build for all macOS platforms
	$(if $(filter Darwin,$(UNAME)),,$(error "macOS builds can only be done on macOS"))
	@$(MAKE) build-macos-x86_64 build-macos-aarch64 build-macos-universal

# Linux builds (native - no zig/cross needed for native platforms)
build-linux-x86_64: check-cargo ## Build for Linux x86_64
	@echo "Building for Linux x86_64..."
	@cd $(FFI_CRATE) && cargo build --no-default-features --features uniffi --release --target $(TARGET_LINUX_x86_64)
	@echo "Build complete: target/$(TARGET_LINUX_x86_64)/release/libconnector_service_ffi.so"

build-linux-aarch64: check-cargo ## Build for Linux aarch64
	@echo "Building for Linux aarch64..."
	@cd $(FFI_CRATE) && cargo zigbuild --no-default-features --features uniffi --release --target $(TARGET_LINUX_AARCH64)
	@echo "Build complete: target/$(TARGET_LINUX_AARCH64)/release/libconnector_service_ffi.so"

build-linux: build-linux-x86_64 build-linux-aarch64 ## Build for all Linux platforms

# All platforms
build-all: ## Build for all supported platforms
	@echo "Building for all platforms..."
	$(if $(filter Darwin,$(UNAME)),\
		$(MAKE) build-macos-x86_64 build-macos-aarch64 build-macos-universal build-linux-x86_64 build-linux-aarch64\
	)
	@echo "All builds complete!"

generate-rust: ## Regenerate Rust FFI flow registrations (handlers + bindings)
	@echo "Generating Rust FFI flow registrations..."
	@python3 $(REPO_ROOT)/scripts/generators/code/generate.py --lang rust

generate-grpc-client: ## Regenerate Rust gRPC client (sdk/rust/src/_generated_grpc_client.rs)
	@echo "Generating Rust gRPC client..."
	@python3 $(REPO_ROOT)/scripts/generators/code/generate.py --lang grpc

generate-python: ## Regenerate Python SDK client (flows + protobuf stubs)
	@$(MAKE) -C $(MAKEFILE_DIR)python generate-flows

generate-javascript: ## Regenerate JavaScript/TypeScript SDK client (flows)
	@$(MAKE) -C $(MAKEFILE_DIR)javascript generate-flows

generate-kotlin: ## Regenerate Kotlin SDK client (flows + bindings)
	@$(MAKE) -C $(MAKEFILE_DIR)java generate-bindings generate-flows

# ---------------------------------------------------------------------------
# generate-descriptor
# Generates the proto descriptor file used by all SDK codegen scripts.
# This is a prerequisite for generate-client-* targets.
# ---------------------------------------------------------------------------
generate-descriptor:
	@echo "Generating proto descriptor..."
	@mkdir -p $(REPO_ROOT)/scripts/generators/code
	@protoc \
		--proto_path=$(REPO_ROOT)/crates/types-traits/grpc-api-types/proto \
		--descriptor_set_out=$(REPO_ROOT)/scripts/generators/code/services.desc \
		--include_source_info \
		$(REPO_ROOT)/crates/types-traits/grpc-api-types/proto/services.proto \
		$(REPO_ROOT)/crates/types-traits/grpc-api-types/proto/payment.proto \
		$(REPO_ROOT)/crates/types-traits/grpc-api-types/proto/payouts.proto \
		$(REPO_ROOT)/crates/types-traits/grpc-api-types/proto/payment_methods.proto \
		$(REPO_ROOT)/crates/types-traits/grpc-api-types/proto/sdk_config.proto
	@echo "Descriptor written to scripts/generators/code/services.desc"

# ---------------------------------------------------------------------------
# generate-client-python
# Delegates to python/Makefile to generate Python client flow methods.
# ---------------------------------------------------------------------------
generate-client-python: generate-descriptor
	@$(MAKE) -C $(REPO_ROOT)/sdk/python generate-client

# ---------------------------------------------------------------------------
# generate-client-javascript
# Delegates to javascript/Makefile to generate JavaScript client flow methods.
# ---------------------------------------------------------------------------
generate-client-javascript: generate-descriptor
	@$(MAKE) -C $(REPO_ROOT)/sdk/javascript generate-client

# ---------------------------------------------------------------------------
# generate-client-java
# Delegates to java/Makefile to generate Java/Kotlin client flow methods.
# ---------------------------------------------------------------------------
generate-client-java: generate-descriptor
	@$(MAKE) -C $(REPO_ROOT)/sdk/java generate-client

# ---------------------------------------------------------------------------
# generate
# Main entry point for regenerating all SDK clients.
# Generates the descriptor, then delegates to each SDK's generate-client target.
# ---------------------------------------------------------------------------
generate: install-deps generate-descriptor generate-rust generate-grpc-client generate-python generate-javascript generate-kotlin ## Auto-discover flows and regenerate all SDK clients
	@echo
	@echo "Generating Python protobuf stubs (_pb2.pyi)..."
	@mkdir -p $(REPO_ROOT)/sdk/python/src/payments/generated
	@MYPY_BIN="$$(python3 -c "import sysconfig; print(sysconfig.get_path('scripts', 'posix_user'))")" && \
		PATH="$$MYPY_BIN:$$PATH" python3 -m grpc_tools.protoc \
		--proto_path=$(REPO_ROOT)/crates/types-traits/grpc-api-types/proto \
		--python_out=$(REPO_ROOT)/sdk/python/src/payments/generated \
		--mypy_out=$(REPO_ROOT)/sdk/python/src/payments/generated \
		$(REPO_ROOT)/crates/types-traits/grpc-api-types/proto/payment.proto \
		$(REPO_ROOT)/crates/types-traits/grpc-api-types/proto/payouts.proto \
		$(REPO_ROOT)/crates/types-traits/grpc-api-types/proto/payment_methods.proto \
		$(REPO_ROOT)/crates/types-traits/grpc-api-types/proto/sdk_config.proto
	@echo "All SDK clients regenerated successfully!"

# ---------------------------------------------------------------------------
# Test targets - delegate to SDK-specific test-package
# ---------------------------------------------------------------------------
test: test-rust test-python test-javascript test-java ## Run all SDK smoke tests

test-rust: ## Run Rust FFI smoke test
	@$(MAKE) -C $(REPO_ROOT)/sdk/rust test-package CONNECTORS=$(CONNECTORS)

GRPC_CONNECTOR ?= $(CONNECTORS)
GRPC_PROFILE   ?= release-fast
QUIET          ?= 1

# Filter output if QUIET=1 (suppress build logs, show only test results)
ifeq ($(QUIET),1)
  OUTPUT_FILTER = 2>&1 | grep -v "^\[INFO\]\|^\[WARNING\]\|^Building\|^Running\|^Checking\|^Compiling\|^Finished\|^Build\|^Running\|cargo check\|Compiling\|^   Compiling\|^   Finished\|^     Running\|^     Finished\|^cargo\|^\s*-->\|^\s*::\|^Note:\|^warning:" || true
  BUILD_FLAGS = --quiet 2>/dev/null || true
else
  OUTPUT_FILTER = 
  BUILD_FLAGS = 
endif

GRPC_SERVER_BIN := $(REPO_ROOT)/target/$(GRPC_PROFILE)/grpc-server

# Internal target: wait for gRPC server to be ready (checks TCP port 8000)
_wait-for-server:
	@echo "Waiting for gRPC server (up to 30s)..."
	@for i in $$(seq 1 30); do \
		if nc -z localhost 8000 2>/dev/null || bash -c 'exec 3<>/dev/tcp/localhost/8000' 2>/dev/null; then \
			echo "Server ready."; \
			exit 0; \
		fi; \
		sleep 1; \
	done; \
	echo "Server did not start within 30s"; \
	exit 1

test-grpc: ## Run gRPC smoke tests for all SDKs (Rust + JS + Python + Kotlin) with a combined summary. Use QUIET=0 for verbose output.
	@{ \
		set -o pipefail; \
		RUST_EXIT=0; JS_EXIT=0; PY_EXIT=0; KT_EXIT=0; \
		\
		printf "\n\033[1m═══ gRPC Smoke Tests ═══\033[0m\n\n"; \
		\
		printf "Building grpc-server ($(GRPC_PROFILE))...\n"; \
		cd $(REPO_ROOT) && cargo build -p grpc-server --profile $(GRPC_PROFILE) 2>&1 | grep -v "^   Compiling\|^   Finished" || true; \
		\
		printf "\nStarting gRPC server (shared across all tests)...\n"; \
		pkill -KILL -f grpc-server 2>/dev/null || true; \
		sleep 1; \
		cd $(REPO_ROOT) && $(GRPC_SERVER_BIN) > /tmp/grpc-server.log 2>&1 & \
		$(MAKE) -C $(REPO_ROOT)/sdk _wait-for-server; \
		\
		printf "\n\033[1m── Rust gRPC ──\033[0m\n"; \
		$(MAKE) -C $(REPO_ROOT)/sdk/rust test-grpc-no-server \
			GRPC_CONNECTOR=$(GRPC_CONNECTOR) GRPC_PROFILE=$(GRPC_PROFILE) 2>&1 | grep -v "^   Compiling\|^   Finished\|^     Running" || true; \
		RUST_EXIT=$$?; \
		\
		printf "\n\033[1m── JavaScript gRPC ──\033[0m\n"; \
		$(MAKE) -C $(REPO_ROOT)/sdk/javascript test-grpc-no-server \
			CONNECTORS=$(CONNECTORS) GRPC_PROFILE=$(GRPC_PROFILE) 2>&1 | tail -20; \
		JS_EXIT=$$?; \
		\
		printf "\n\033[1m── Python gRPC ──\033[0m\n"; \
		$(MAKE) -C $(REPO_ROOT)/sdk/python test-grpc-no-server \
			CONNECTORS=$(CONNECTORS) GRPC_PROFILE=$(GRPC_PROFILE) 2>&1 | tail -20; \
		PY_EXIT=$$?; \
		\
		printf "\n\033[1m── Kotlin gRPC ──\033[0m\n"; \
		$(MAKE) -C $(REPO_ROOT)/sdk/java test-grpc-no-server \
			CONNECTORS=$(CONNECTORS) GRPC_PROFILE=$(GRPC_PROFILE) 2>&1 | tail -30; \
		KT_EXIT=$$?; \
		\
		printf "\nStopping gRPC server...\n"; \
		pkill -f grpc-server 2>/dev/null || true; \
		\
		printf "\n\033[1m══ gRPC Results Summary ══\033[0m\n"; \
		[ $$RUST_EXIT -eq 0 ] \
			&& printf "  \033[32m✓\033[0m Rust\n" \
			|| printf "  \033[31m✗\033[0m Rust (exit $$RUST_EXIT)\n"; \
		[ $$JS_EXIT -eq 0 ] \
			&& printf "  \033[32m✓\033[0m JavaScript\n" \
			|| printf "  \033[31m✗\033[0m JavaScript (exit $$JS_EXIT)\n"; \
		[ $$PY_EXIT -eq 0 ] \
			&& printf "  \033[32m✓\033[0m Python\n" \
			|| printf "  \033[31m✗\033[0m Python (exit $$PY_EXIT)\n"; \
		[ $$KT_EXIT -eq 0 ] \
			&& printf "  \033[32m✓\033[0m Kotlin\n" \
			|| printf "  \033[31m✗\033[0m Kotlin (exit $$KT_EXIT)\n"; \
		\
		TOTAL=$$(( RUST_EXIT | JS_EXIT | PY_EXIT | KT_EXIT )); \
		printf "\n"; \
		[ $$TOTAL -eq 0 ] \
			&& printf "\033[32mAll gRPC tests passed.\033[0m\n" \
			|| printf "\033[31mSome gRPC tests FAILED.\033[0m\n"; \
		exit $$TOTAL; \
	}

test-python: build-ffi-lib ## Build FFI → Run Python smoke test
	@$(MAKE) -C $(REPO_ROOT)/sdk/python test-package CONNECTORS=$(CONNECTORS)

test-javascript: build-ffi-lib ## Build FFI → Run JavaScript smoke test
	@$(MAKE) -C $(REPO_ROOT)/sdk/javascript test-package CONNECTORS=$(CONNECTORS)

test-java: build-ffi-lib ## Build FFI → Run Java smoke test
	@$(MAKE) -C $(REPO_ROOT)/sdk/java test-package CONNECTORS=$(CONNECTORS)

# ---------------------------------------------------------------------------
# Distribution targets - delegate to SDK-specific dist
# ---------------------------------------------------------------------------
dist: dist-python dist-javascript dist-java ## Build all SDK distribution packages

dist-python: build-ffi-lib ## Build FFI → Build Python wheel
	@$(MAKE) -C $(REPO_ROOT)/sdk/python dist

dist-javascript: build-ffi-lib ## Build FFI → Build JS npm package
	@$(MAKE) -C $(REPO_ROOT)/sdk/javascript dist

dist-java: build-ffi-lib ## Build FFI → Build Java JAR
	@$(MAKE) -C $(REPO_ROOT)/sdk/java dist

# ---------------------------------------------------------------------------
# Clean - comprehensive cleanup across all SDKs
# ---------------------------------------------------------------------------
clean: ## Clean all build artifacts
	@echo "Cleaning build artifacts..."
	@cd $(FFI_CRATE) && cargo clean
	@rm -f $(REPO_ROOT)/scripts/generators/code/services.desc
	@rm -f $(REPO_ROOT)/crates/ffi/ffi/src/handlers/_generated_flow_registrations.rs
	@rm -f $(REPO_ROOT)/crates/ffi/ffi/src/bindings/_generated_ffi_flows.rs
	@$(MAKE) -C $(REPO_ROOT)/sdk/python clean
	@$(MAKE) -C $(REPO_ROOT)/sdk/javascript clean
	@$(MAKE) -C $(REPO_ROOT)/sdk/java clean
	@echo "Clean complete."
