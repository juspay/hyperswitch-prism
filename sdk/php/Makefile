.PHONY: install-deps generate-bindings generate-proto generate-all \
        generate-flows pack-archive install test-package dist clean

# SDK-specific paths (define BEFORE including common.mk)
MAKEFILE_DIR := $(dir $(abspath $(lastword $(MAKEFILE_LIST))))
SDK_ROOT     := $(MAKEFILE_DIR)

# Connectors to test (comma-separated, default: stripe)
CONNECTORS ?= stripe

# Include common SDK build configuration (defines FFI_CRATE, LIBRARY, PLATFORM, etc.)
include $(SDK_ROOT)/../common.mk


# SDK-specific paths continued
GENERATED_OUT := $(SDK_ROOT)/src/Payments/Generated
PACKAGE_DIR   := $(ARTIFACTS_DIR)/sdk-php

# ---------------------------------------------------------------------------
# install-deps
# Installs Composer dependencies required for protobuf and HTTP.
# Checks for tool existence before installing. In Nix environments php and
# composer are provided by the shell. Outside Nix, runs composer install
# only when vendor/ is absent or composer.lock has changed.
# ---------------------------------------------------------------------------
install-deps:
	@command -v php >/dev/null 2>&1 || \
		(echo "Error: php not found. Install via nix ('nix develop') or your system package manager." && exit 1)
	@command -v composer >/dev/null 2>&1 || \
		(echo "Error: composer not found. Install via nix or https://getcomposer.org" && exit 1)
	@php -r "if (!extension_loaded('ffi')) { echo \"Warning: PHP FFI extension not enabled. Add ffi.enable=1 to php.ini or run php with -d ffi.enable=1\n\"; }" 2>/dev/null || true
	@cd $(SDK_ROOT) && composer install --no-interaction --no-progress --ignore-platform-req=ext-ffi
	@echo "All required tools present."

# ---------------------------------------------------------------------------
# generate-bindings
# Copies the pre-built FFI native library into the Generated output directory
# so it is bundled alongside the PHP stubs. The PHP SDK calls the library
# directly via FFI — no UniFFI bindgen step is required for PHP.
# Requires: build-ffi-lib to have been run first.
# ---------------------------------------------------------------------------
generate-bindings:
	@[ -f "$(LIBRARY)" ] || \
		(echo "Error: FFI library not found at $(LIBRARY). Run 'make build-ffi-lib' first." && exit 1)
	@echo "Copying FFI library to $(GENERATED_OUT)/..."
	@mkdir -p $(GENERATED_OUT)
	@cp -f $(LIBRARY) $(GENERATED_OUT)/
	@echo "Library copied to $(GENERATED_OUT)/"

# ---------------------------------------------------------------------------
# generate-proto
# Generates PHP protobuf stubs from .proto files into the Generated output
# directory using protoc --php_out. The proto package 'types' maps to the
# PHP namespace 'Types\', and metadata goes into 'GPBMetadata\'.
# Requires: protoc in PATH (provided by nix develop or system install).
# ---------------------------------------------------------------------------
generate-proto:
	@command -v protoc >/dev/null 2>&1 || \
		(echo "Error: protoc not found. Install via nix ('nix develop') or https://protobuf.dev" && exit 1)
	@echo "Generating PHP protobuf stubs..."
	@mkdir -p $(GENERATED_OUT)
	@protoc \
		--proto_path=$(PROTO_DIR) \
		--php_out=$(GENERATED_OUT) \
		$(PROTO_DIR)/payment.proto \
		$(PROTO_DIR)/payment_methods.proto \
		$(PROTO_DIR)/payouts.proto \
		$(PROTO_DIR)/sdk_config.proto
	@echo "PHP protobuf stubs generated in $(GENERATED_OUT)/"

# ---------------------------------------------------------------------------
# generate-flows
# Generates _GeneratedFlows.php and _GeneratedServiceClients.php by running
# the SDK codegen script. Assumes the proto descriptor (services.desc) is
# already generated (done by the top-level sdk/Makefile generate target).
# ---------------------------------------------------------------------------
generate-flows:
	@echo "Generating PHP flow methods..."
	@python3 $(REPO_ROOT)/scripts/generators/code/generate.py --lang php
	@echo "PHP flow methods generated."

# ---------------------------------------------------------------------------
# generate-all
# Runs all code-generation steps: protobuf stubs, native library copy, and
# flow method codegen. Requires build-ffi-lib and proto descriptor to exist.
# ---------------------------------------------------------------------------
generate-all: generate-proto generate-bindings generate-flows

# ---------------------------------------------------------------------------
# pack-archive
# Creates a Composer-compatible archive of the SDK.
# Users can install directly via: composer require juspay/connector-service-php
# or point to the local archive.
# ---------------------------------------------------------------------------
pack-archive: install-deps
	@mkdir -p $(PACKAGE_DIR)
	@cd $(SDK_ROOT) && composer archive --format=zip --dir=$(PACKAGE_DIR)/ --file=connector-service-php
	@echo "Archive built: $(PACKAGE_DIR)/connector-service-php.zip"

# ---------------------------------------------------------------------------
# install
# Alias for pack-archive (interface parity with other SDK Makefiles).
# ---------------------------------------------------------------------------
install: pack-archive

# ---------------------------------------------------------------------------
# test-package
# Full end-to-end local test: builds FFI lib, generates all artifacts, then
# runs the smoke test using the local sources (no install step needed since
# PHP autoloads directly from src/).
# ---------------------------------------------------------------------------
test-package: build-ffi-lib generate-all install-deps
	@echo "Testing PHP SDK (smoke test)..."
	@rm -rf /tmp/test-hyperswitch-php
	@mkdir -p /tmp/test-hyperswitch-php
	@cp $(SDK_ROOT)/smoke-test/test_smoke.php /tmp/test-hyperswitch-php/
	@# Copy vendor and src so the smoke test can use the local package
	@cp -r $(SDK_ROOT)/vendor /tmp/test-hyperswitch-php/
	@cp -r $(SDK_ROOT)/src /tmp/test-hyperswitch-php/
	@# Copy composer.json for autoloader generation
	@cp $(SDK_ROOT)/composer.json /tmp/test-hyperswitch-php/
	@cd /tmp/test-hyperswitch-php && composer dump-autoload --no-interaction --quiet --ignore-platform-req=ext-ffi
	@# Copy credentials file if it exists
	@if [ -f $(REPO_ROOT)/creds.json ]; then \
		cp $(REPO_ROOT)/creds.json /tmp/test-hyperswitch-php/; \
	else \
		echo "  Note: creds.json not found, test will use placeholder credentials"; \
	fi
	@cd /tmp/test-hyperswitch-php && php -d ffi.enable=1 test_smoke.php --connectors $(CONNECTORS)
	@rm -rf /tmp/test-hyperswitch-php
	@echo "PHP smoke test passed."

# ---------------------------------------------------------------------------
# dist
# Build distribution archive including pre-built native binaries for all
# supported platforms, then pack. Mirrors the pattern in python/Makefile.
# ---------------------------------------------------------------------------
dist:
	@echo "Building PHP SDK distribution archive..."
	@mkdir -p $(GENERATED_OUT)
	@cp -fL $(REPO_ROOT)/target/x86_64-unknown-linux-gnu/release/libconnector_service_ffi.so \
		$(GENERATED_OUT)/ 2>/dev/null || echo "  Note: Linux x86_64 binary not found (skipping)"
	@cp -fL $(REPO_ROOT)/target/aarch64-apple-darwin/release/libconnector_service_ffi.dylib \
		$(GENERATED_OUT)/ 2>/dev/null || echo "  Note: macOS aarch64 binary not found (skipping)"
	@$(MAKE) -C $(SDK_ROOT) pack-archive
	@echo "Distribution archive created in $(PACKAGE_DIR)/"

# ---------------------------------------------------------------------------
# clean
# Remove generated artifacts and Composer vendor directory.
# ---------------------------------------------------------------------------
clean:
	@rm -rf $(GENERATED_OUT)
	@rm -f $(SDK_ROOT)/src/Payments/_GeneratedFlows.php \
	        $(SDK_ROOT)/src/Payments/_GeneratedServiceClients.php
	@rm -rf $(SDK_ROOT)/vendor
