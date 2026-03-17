.PHONY: install-deps install-codegen-deps generate-bindings generate-proto generate-all \
        generate-flows build-ts pack-archive install test-package test-composite dist clean

# SDK-specific paths (define BEFORE including common.mk)
MAKEFILE_DIR := $(dir $(abspath $(lastword $(MAKEFILE_LIST))))
SDK_ROOT     := $(MAKEFILE_DIR)

# Connectors to test (comma-separated, default: stripe)
CONNECTORS ?= stripe

# Include common SDK build configuration
include $(SDK_ROOT)/../common.mk

# SDK-specific paths continued
GENERATED_OUT := $(SDK_ROOT)/src/payments/generated
PACKAGE_DIR   := $(ARTIFACTS_DIR)/sdk-javascript

# ---------------------------------------------------------------------------
# install-deps
# Installs Node.js build dependencies required for protobuf code generation.
# Checks for tool existence before installing. In Nix environments, node and
# npm are provided by the shell. Outside Nix, runs npm install only when
# node_modules or the required pbjs/pbts binaries are absent.
# ---------------------------------------------------------------------------
install-deps:
	@command -v node >/dev/null 2>&1 || \
		(echo "Error: node not found. Install via nix or https://nodejs.org" && exit 1)
	@command -v npm >/dev/null 2>&1 || \
		(echo "Error: npm not found. Install via nix or https://nodejs.org" && exit 1)
	@cd $(SDK_ROOT) && \
		(echo "  → Installing npm dependencies..." && npm install)
	@echo "All required tools present."

# ---------------------------------------------------------------------------
# install-codegen-deps
# Ensures jinja2 etc. are installed before generate-flows (used by CI and local).
# Delegates to sdk/Makefile install-deps (idempotent, skips if present).
# ---------------------------------------------------------------------------
install-codegen-deps:
	@$(MAKE) -C $(REPO_ROOT)/sdk install-deps

# ---------------------------------------------------------------------------
# generate-bindings
# Copies the pre-built FFI native library into the generated output directory
# so it is bundled with the npm package. The JavaScript SDK drives the native
# library directly via Node.js FFI; no UniFFI code generation is required.
# Requires: build-ffi-lib to have been run first (or library downloaded in CI).
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
# Generates a static protobuf JavaScript module and its TypeScript declaration
# from the .proto files using protobufjs (pbjs/pbts). Output is placed in the
# generated output directory alongside the native FFI library.
# ---------------------------------------------------------------------------
generate-proto: install-deps
	@echo "Generating static protobuf JS module..."
	@mkdir -p $(GENERATED_OUT)
	@cd $(SDK_ROOT) && ./node_modules/.bin/pbjs \
		-t static-module -w commonjs --no-delimited --no-convert --alt-comment \
		-p $(PROTO_DIR) \
		$(PROTO_DIR)/payment.proto $(PROTO_DIR)/payment_methods.proto \
		$(PROTO_DIR)/sdk_config.proto \
		-o $(GENERATED_OUT)/proto.js
	@echo "Proto JS generated: $(GENERATED_OUT)/proto.js"
	@cd $(SDK_ROOT) && ./node_modules/.bin/pbts \
		-o $(GENERATED_OUT)/proto.d.ts $(GENERATED_OUT)/proto.js
	@echo "Proto TS generated: $(GENERATED_OUT)/proto.d.ts"

# ---------------------------------------------------------------------------
# generate-all
# Runs all code-generation steps: copies the native library and generates the
# protobuf JS/TS module. Proto depends on install-deps; both can run in any
# order relative to each other.
# ---------------------------------------------------------------------------
generate-all: generate-proto generate-bindings generate-flows

# ---------------------------------------------------------------------------
# generate-flows
# Generates flow methods (authorize, capture, etc.) by running the SDK
# codegen script. Assumes descriptor is already generated.
# Depends on install-codegen-deps so jinja2 etc. are available (CI and local).
# ---------------------------------------------------------------------------
generate-flows: install-codegen-deps
	@echo "Generating JavaScript flow methods..."
	@cd $(REPO_ROOT)/sdk/codegen && python3 generate.py --lang javascript
	@echo "JavaScript flow methods generated."

# ---------------------------------------------------------------------------
# build-ts
# Compiles TypeScript sources to dist/ and copies the generated proto stubs
# and native FFI library into dist/src/payments/generated/ so that npm pack
# includes them. Must run after generate-all (requires generated/ to be populated).
# ---------------------------------------------------------------------------
build-ts: install-deps
	@echo "Compiling TypeScript to dist/..."
	@cd $(SDK_ROOT) && npm run build
	@echo "TypeScript compiled to dist/"

# ---------------------------------------------------------------------------
# pack-archive
# Builds the SDK npm tarball (.tgz) using npm pack.
# Assumes dist/ has been populated by build-ts. Copies any symlinks as real
# files first since npm does not follow symlinks.
# ---------------------------------------------------------------------------
pack-archive:
	@mkdir -p $(PACKAGE_DIR)
	@if [ -L $(GENERATED_OUT)/libconnector_service_ffi.$(LIB_EXT) ]; then \
		cp -fL $(GENERATED_OUT)/libconnector_service_ffi.$(LIB_EXT) \
		        $(GENERATED_OUT)/libconnector_service_ffi.$(LIB_EXT).tmp && \
		mv -f   $(GENERATED_OUT)/libconnector_service_ffi.$(LIB_EXT).tmp \
		        $(GENERATED_OUT)/libconnector_service_ffi.$(LIB_EXT); \
	fi
	@cd $(SDK_ROOT) && npm pack --pack-destination $(PACKAGE_DIR)/
	@echo "Tarball built in $(PACKAGE_DIR)/"

# ---------------------------------------------------------------------------
# install
# Builds the SDK tarball. npm tarballs do not require a system-level install
# step — users install directly with 'npm install file:...tgz'. This target
# is a no-op beyond pack-archive and exists for interface parity with the
# other SDK Makefiles.
# ---------------------------------------------------------------------------
install: pack-archive

# ---------------------------------------------------------------------------
# test-package
# Full end-to-end local test: builds the FFI library, generates all code
# artifacts, packs the tarball, then installs it into an isolated temporary
# npm project and runs the smoke test to verify the package works correctly.
# The temp directory is cleaned up after the test.
# Uses package name/version from package.json so it works regardless of naming.
# ---------------------------------------------------------------------------
JS_PKG_NAME := $(shell node -p "require('$(SDK_ROOT)package.json').name")
JS_PKG_VERSION := $(shell node -p "require('$(SDK_ROOT)package.json').version")
JS_TARBALL := $(PACKAGE_DIR)/$(JS_PKG_NAME)-$(JS_PKG_VERSION).tgz

test-package: build-ffi-lib generate-all build-ts pack-archive
	@echo "Testing packed tarball in isolation..."
	@rm -rf /tmp/test-hyperswitch-js
	@mkdir -p /tmp/test-hyperswitch-js
	@test -f $(JS_TARBALL) || (echo "Error: Tarball not found: $(JS_TARBALL)" && exit 1)
	@cd /tmp/test-hyperswitch-js && npm init -y --silent && \
		npm install $(JS_TARBALL) typescript @types/node --silent
	@cp $(SDK_ROOT)/smoke-test/test_smoke.ts /tmp/test-hyperswitch-js/test_smoke.ts
	@cp $(SDK_ROOT)/smoke-test/tsconfig.json /tmp/test-hyperswitch-js/tsconfig.json
	@# Copy credentials file if it exists
	@if [ -f $(REPO_ROOT)/creds.json ]; then \
		cp $(REPO_ROOT)/creds.json /tmp/test-hyperswitch-js/; \
	else \
		echo "  Note: creds.json not found, test will use placeholder credentials"; \
	fi
	@cd /tmp/test-hyperswitch-js && npx tsc && node test_smoke.js --connectors $(CONNECTORS)
	@echo "Running composite flow test..."
	@cp $(SDK_ROOT)/smoke-test/test_smoke_composite.ts /tmp/test-hyperswitch-js/
	@cd /tmp/test-hyperswitch-js && npx ts-node test_smoke_composite.ts
	@rm -rf /tmp/test-hyperswitch-js
	@echo "Package test passed."

# ---------------------------------------------------------------------------
# test-composite
# Runs the composite flow smoke test (access token + authorize flow).
# Tests the PayPal access token composite flow in an isolated environment.
# ---------------------------------------------------------------------------
test-composite: build-ffi-lib generate-all build-ts pack-archive
	@echo "Testing composite flow in isolation..."
	@rm -rf /tmp/test-hyperswitch-js-composite
	@mkdir -p /tmp/test-hyperswitch-js-composite
	@test -f $(JS_TARBALL) || (echo "Error: Tarball not found: $(JS_TARBALL)" && exit 1)
	@cd /tmp/test-hyperswitch-js-composite && npm init -y --silent && \
		npm install $(JS_TARBALL) typescript @types/node ts-node --silent
	@cp $(SDK_ROOT)/smoke-test/test_smoke_composite.ts /tmp/test-hyperswitch-js-composite/
	@cp $(SDK_ROOT)/smoke-test/tsconfig.json /tmp/test-hyperswitch-js-composite/
	@cd /tmp/test-hyperswitch-js-composite && npx ts-node test_smoke_composite.ts
	@rm -rf /tmp/test-hyperswitch-js-composite
	@echo "Composite flow test passed."

# ---------------------------------------------------------------------------
# dist
# Builds the distribution tarball for all supported platforms.
# Copies pre-built native binaries for each target platform into the generated
# output directory as real files (not symlinks — npm does not follow symlinks),
# generates the proto JS/TS module, then packs the tarball. The single-platform
# library from generate-bindings is overwritten by the multi-platform copies.
# ---------------------------------------------------------------------------
dist:
	@echo "Building JavaScript SDK distribution tarball..."
	@mkdir -p $(GENERATED_OUT)
	@cp -fL $(REPO_ROOT)/target/x86_64-unknown-linux-gnu/release/libconnector_service_ffi.so \
		$(GENERATED_OUT)/ 2>/dev/null || echo "  Note: Linux x86_64 binary not found (skipping)"
	@cp -fL $(REPO_ROOT)/target/aarch64-apple-darwin/release/libconnector_service_ffi.dylib \
		$(GENERATED_OUT)/ 2>/dev/null || echo "  Note: macOS aarch64 binary not found (skipping)"
	@$(MAKE) -C $(SDK_ROOT) build-ts
	@$(MAKE) -C $(SDK_ROOT) pack-archive
	@echo "Distribution tarball created in $(PACKAGE_DIR)/"

clean:
	@rm -rf $(GENERATED_OUT) node_modules
	@rm -f $(SDK_ROOT)src/payments/_generated_flows.js \
	        $(SDK_ROOT)src/payments/_generated_connector_client_flows.ts \
	        $(SDK_ROOT)src/payments/_generated_uniffi_client_flows.ts
