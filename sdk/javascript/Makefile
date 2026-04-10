.PHONY: install-deps install-codegen-deps generate-bindings generate-proto generate-grpc-bindings \
        generate-all build-ts pack-archive install test-package test-composite test-grpc test-grpc-no-server dist publish clean \
        publish-check-auth publish-check-version publish-check-dry-run publish-check

# SDK-specific paths (define BEFORE including common.mk)
MAKEFILE_DIR := $(dir $(abspath $(lastword $(MAKEFILE_LIST))))
SDK_ROOT     := $(MAKEFILE_DIR)

# Connectors to test (comma-separated, default: stripe)
CONNECTORS ?= stripe

# Include common SDK build configuration
include $(SDK_ROOT)/../common.mk

# SDK-specific paths continued
GENERATED_OUT    := $(SDK_ROOT)/src/payments/generated
PACKAGE_DIR      := $(ARTIFACTS_DIR)/sdk-javascript
PACKAGE_VERSION  := $(shell node -p "require('$(SDK_ROOT)/package.json').version")
JS_TARBALL       := $(PACKAGE_DIR)/hyperswitch-prism-$(PACKAGE_VERSION).tgz

# ---------------------------------------------------------------------------
# install-deps
# Checks for required build tools. In Nix environments tools are provided by
# the shell so this is effectively a no-op. Outside Nix it exits with a helpful
# message for any missing tool rather than failing silently mid-build.
# ---------------------------------------------------------------------------
install-deps:
	@command -v node >/dev/null 2>&1 || \
		(echo "Error: node not found. Install via nix or a Node version manager." && exit 1)
	@command -v npm >/dev/null 2>&1 || \
		(echo "Error: npm not found. Install Node.js which includes npm." && exit 1)
	@command -v cargo >/dev/null 2>&1 || \
		(echo "Error: cargo not found. Install Rust via rustup." && exit 1)
	@command -v protoc >/dev/null 2>&1 || \
		(echo "Error: protoc not found. Install via nix or 'apt install protobuf-compiler'." && exit 1)
	@echo "All required tools present."
	@cd $(SDK_ROOT) && npm install --silent

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
		$(PROTO_DIR)/payment.proto $(PROTO_DIR)/payouts.proto $(PROTO_DIR)/payment_methods.proto \
		$(PROTO_DIR)/sdk_config.proto \
		-o $(GENERATED_OUT)/proto.js
	@echo "Proto JS generated: $(GENERATED_OUT)/proto.js"
	@cd $(SDK_ROOT) && ./node_modules/.bin/pbts \
		-o $(GENERATED_OUT)/proto.d.ts $(GENERATED_OUT)/proto.js
	@echo "Proto TS generated: $(GENERATED_OUT)/proto.d.ts"

# ---------------------------------------------------------------------------
# generate-grpc-bindings
# Copies the pre-built gRPC FFI native library into the generated output
# directory so it is bundled with the npm package.
# Requires: build-grpc-ffi-lib to have been run first.
# ---------------------------------------------------------------------------
generate-grpc-bindings:
	@[ -f "$(GRPC_FFI_LIBRARY)" ] || \
		(echo "Error: gRPC FFI library not found at $(GRPC_FFI_LIBRARY). Run 'make build-grpc-ffi-lib' first." && exit 1)
	@echo "Copying gRPC FFI library to $(GENERATED_OUT)/..."
	@mkdir -p $(GENERATED_OUT)
	@cp -f $(GRPC_FFI_LIBRARY) $(GENERATED_OUT)/
	@echo "gRPC library copied to $(GENERATED_OUT)/"

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
# codegen script.
# Depends on install-codegen-deps so jinja2 etc. are available (CI and local).
# ---------------------------------------------------------------------------
generate-flows: install-codegen-deps
	@echo "Generating JavaScript flow methods..."
	@python3 $(REPO_ROOT)/scripts/generators/code/generate.py --lang javascript
	@echo "JavaScript flow methods generated."

# ---------------------------------------------------------------------------
# build-ts
# Compiles TypeScript sources to dist/ and copies the generated proto stubs
# and native FFI library into dist/src/payments/generated/ so that npm pack
# includes them. Must run after generate-all (requires generated/ to be populated).
# ---------------------------------------------------------------------------
build-ts: install-deps
	@echo "Compiling TypeScript..."
	@npm run build
	@echo "TypeScript compilation complete."

# ---------------------------------------------------------------------------
# pack-archive
# Packages the SDK into a tarball for distribution.
# ---------------------------------------------------------------------------
pack-archive: build-ts
	@echo "Packing npm archive..."
	@mkdir -p $(PACKAGE_DIR)
	@npm pack --pack-destination $(PACKAGE_DIR)
	@echo "Tarball created: $(JS_TARBALL)"

# ---------------------------------------------------------------------------
# install
# Local install: builds TypeScript and links for development.
# ---------------------------------------------------------------------------
install: install-deps generate-all build-ts
	@npm install

# ---------------------------------------------------------------------------
# test-package
# End-to-end test: build FFI → pack → install in temp dir → run smoke test.
# Also runs composite flow tests.
# ---------------------------------------------------------------------------
test-package: build-ffi-lib generate-all build-ts pack-archive
	@echo "Testing in isolation..."
	@rm -rf /tmp/test-hyperswitch-js
	@mkdir -p /tmp/test-hyperswitch-js
	@test -f $(JS_TARBALL) || (echo "Error: Tarball not found: $(JS_TARBALL)" && exit 1)
	@cd /tmp/test-hyperswitch-js && npm init -y --silent && \
		npm install $(JS_TARBALL) typescript @types/node --silent
	@cp $(SDK_ROOT)/smoke-test/test_smoke.ts /tmp/test-hyperswitch-js/
	@cp $(SDK_ROOT)/smoke-test/tsconfig.json      /tmp/test-hyperswitch-js/
	@cp $(REPO_ROOT)/sdk/generated/flows.json /tmp/test-hyperswitch-js/
	@if [ -f $(REPO_ROOT)/creds.json ]; then \
		cp $(REPO_ROOT)/creds.json /tmp/test-hyperswitch-js/; \
	fi
	@cd /tmp/test-hyperswitch-js && \
		NODE_PATH=/tmp/test-hyperswitch-js/node_modules npx tsx test_smoke.ts \
		--connectors $(CONNECTORS) --examples-dir $(REPO_ROOT)/examples
	@rm -rf /tmp/test-hyperswitch-js
	@echo "Testing composite flows in isolation..."
	@rm -rf /tmp/test-hyperswitch-js-composite
	@mkdir -p /tmp/test-hyperswitch-js-composite
	@test -f $(JS_TARBALL) || (echo "Error: Tarball not found: $(JS_TARBALL)" && exit 1)
	@cd /tmp/test-hyperswitch-js-composite && npm init -y --silent && \
		npm install $(JS_TARBALL) typescript @types/node --silent
	@cp $(SDK_ROOT)/smoke-test/test_smoke_composite.ts /tmp/test-hyperswitch-js-composite/
	@cp $(SDK_ROOT)/smoke-test/tsconfig.json      /tmp/test-hyperswitch-js-composite/
	@if [ -f $(REPO_ROOT)/creds.json ]; then \
		cp $(REPO_ROOT)/creds.json /tmp/test-hyperswitch-js-composite/; \
	fi
	@cd /tmp/test-hyperswitch-js-composite && npx tsc && \
		NODE_PATH=/tmp/test-hyperswitch-js-composite/node_modules node test_smoke_composite.js \
		--connectors $(CONNECTORS) --examples-dir $(REPO_ROOT)/examples
	@rm -rf /tmp/test-hyperswitch-js-composite
	@echo "Running webhook smoke test..."
	@rm -rf /tmp/test-hyperswitch-js-webhook
	@mkdir -p /tmp/test-hyperswitch-js-webhook
	@test -f $(JS_TARBALL) || (echo "Error: Tarball not found: $(JS_TARBALL)" && exit 1)
	@cd /tmp/test-hyperswitch-js-webhook && npm init -y --silent && \
		npm install $(JS_TARBALL) typescript @types/node --silent
	@cp $(SDK_ROOT)/smoke-test/test_smoke_webhook.ts /tmp/test-hyperswitch-js-webhook/
	@cp $(SDK_ROOT)/smoke-test/tsconfig.json /tmp/test-hyperswitch-js-webhook/
	@cd /tmp/test-hyperswitch-js-webhook && \
		NODE_PATH=/tmp/test-hyperswitch-js-webhook/node_modules npx tsx test_smoke_webhook.ts
	@rm -rf /tmp/test-hyperswitch-js-webhook

# ---------------------------------------------------------------------------
# test-composite
# End-to-end composite flow test: build FFI → pack → install → run composite test.
# ---------------------------------------------------------------------------
test-composite: build-ffi-lib generate-all build-ts pack-archive
	@echo "Testing composite flows in isolation..."
	@rm -rf /tmp/test-hyperswitch-js-composite
	@mkdir -p /tmp/test-hyperswitch-js-composite
	@test -f $(JS_TARBALL) || (echo "Error: Tarball not found: $(JS_TARBALL)" && exit 1)
	@cd /tmp/test-hyperswitch-js-composite && npm init -y --silent && \
		npm install $(JS_TARBALL) typescript @types/node --silent
	@cp $(SDK_ROOT)/smoke-test/test_smoke_composite.ts /tmp/test-hyperswitch-js-composite/
	@cp $(SDK_ROOT)/smoke-test/tsconfig.json      /tmp/test-hyperswitch-js-composite/
	@if [ -f $(REPO_ROOT)/creds.json ]; then \
		cp $(REPO_ROOT)/creds.json /tmp/test-hyperswitch-js-composite/; \
	fi
	@cd /tmp/test-hyperswitch-js-composite && npx tsc && \
		NODE_PATH=/tmp/test-hyperswitch-js-composite/node_modules node test_smoke_composite.js \
		--connectors $(CONNECTORS) --examples-dir $(REPO_ROOT)/examples
	@rm -rf /tmp/test-hyperswitch-js-composite

# ---------------------------------------------------------------------------
# test-package-mock
# Same as test-package but runs in MOCK mode (no real HTTP calls).
# Uses generated harnesses instead of examples and verifies req_transformer only.
# ---------------------------------------------------------------------------
test-package-mock: build-ffi-lib generate-all build-ts pack-archive
	@echo "Testing in MOCK mode (HTTP intercepted)..."
	@rm -rf /tmp/test-hyperswitch-js-mock
	@mkdir -p /tmp/test-hyperswitch-js-mock
	@test -f $(JS_TARBALL) || (echo "Error: Tarball not found: $(JS_TARBALL)" && exit 1)
	@cd /tmp/test-hyperswitch-js-mock && npm init -y --silent && \
		npm install $(JS_TARBALL) typescript @types/node --silent
	@cp $(SDK_ROOT)/smoke-test/test_smoke.ts /tmp/test-hyperswitch-js-mock/
	@cp $(SDK_ROOT)/smoke-test/tsconfig.json /tmp/test-hyperswitch-js-mock/
	@cp $(REPO_ROOT)/sdk/generated/flows.json /tmp/test-hyperswitch-js-mock/
	@if [ -f $(REPO_ROOT)/creds_dummy.json ]; then \
		cp $(REPO_ROOT)/creds_dummy.json /tmp/test-hyperswitch-js-mock/creds.json; \
	fi
	@# Copy harness to the location test_smoke.ts expects in mock mode
	@mkdir -p /tmp/test-hyperswitch-js-mock/smoke-test/generated/$(CONNECTORS)
	@cp $(SDK_ROOT)/smoke-test/generated/$(CONNECTORS)/$(CONNECTORS).ts /tmp/test-hyperswitch-js-mock/smoke-test/generated/$(CONNECTORS)/
	@cd /tmp/test-hyperswitch-js-mock && \
		NODE_PATH=/tmp/test-hyperswitch-js-mock/node_modules npx tsx test_smoke.ts \
		--connectors $(CONNECTORS) --creds-file creds.json --mock --examples-dir ./smoke-test/generated
	@rm -rf /tmp/test-hyperswitch-js-mock
	@echo "Package mock test passed."

# ---------------------------------------------------------------------------
# test-grpc
# End-to-end gRPC smoke test: builds both FFI libraries, packs the tarball,
# installs it in an isolated temp project, then runs test_smoke_grpc.ts.
# ---------------------------------------------------------------------------
GRPC_PROFILE    ?= release-fast
GRPC_SERVER_BIN := $(REPO_ROOT)/target/$(PLATFORM)/$(GRPC_PROFILE)/grpc-server

# Run gRPC test assuming server is already running (used by top-level test-grpc)
test-grpc-no-server: build-ffi-lib build-grpc-ffi-lib generate-all generate-grpc-bindings build-ts pack-archive
	@echo "Testing gRPC flows in isolation..."
	@rm -rf /tmp/test-hyperswitch-js-grpc
	@mkdir -p /tmp/test-hyperswitch-js-grpc
	@test -f $(JS_TARBALL) || (echo "Error: Tarball not found: $(JS_TARBALL)" && exit 1)
	@cd /tmp/test-hyperswitch-js-grpc && npm init -y --silent && \
		npm install $(JS_TARBALL) typescript @types/node --silent
	@cp $(SDK_ROOT)/smoke-test/test_smoke_grpc.ts /tmp/test-hyperswitch-js-grpc/
	@cp $(SDK_ROOT)/smoke-test/tsconfig.json      /tmp/test-hyperswitch-js-grpc/
	@cp $(REPO_ROOT)/sdk/generated/flows.json /tmp/test-hyperswitch-js-grpc/
	@if [ -f $(REPO_ROOT)/creds.json ]; then \
		cp $(REPO_ROOT)/creds.json /tmp/test-hyperswitch-js-grpc/; \
	else \
		echo "  Note: no creds.json found, test will use placeholder credentials"; \
	fi
	@cd /tmp/test-hyperswitch-js-grpc && \
		NODE_PATH=/tmp/test-hyperswitch-js-grpc/node_modules FORCE_COLOR=1 npx tsx test_smoke_grpc.ts \
		--connectors $(CONNECTORS) --examples-dir $(REPO_ROOT)/examples
	@rm -rf /tmp/test-hyperswitch-js-grpc

test-grpc: build-ffi-lib build-grpc-ffi-lib generate-all generate-grpc-bindings build-ts pack-archive
	@echo "Building grpc-server ($(GRPC_PROFILE))..."
	@cd $(REPO_ROOT) && cargo build -p grpc-server --profile $(GRPC_PROFILE) --target $(PLATFORM) 2>&1
	@echo "Starting gRPC server..."
	@pkill -KILL -f grpc-server 2>/dev/null || true
	@sleep 1
	@cd $(REPO_ROOT) && $(GRPC_SERVER_BIN) > /tmp/grpc-server-js.log 2>&1 &
	@echo "Testing gRPC flows in isolation..."
	@rm -rf /tmp/test-hyperswitch-js-grpc
	@mkdir -p /tmp/test-hyperswitch-js-grpc
	@test -f $(JS_TARBALL) || (echo "Error: Tarball not found: $(JS_TARBALL)" && exit 1)
	@cd /tmp/test-hyperswitch-js-grpc && npm init -y --silent && \
		npm install $(JS_TARBALL) typescript @types/node --silent
	@cp $(SDK_ROOT)/smoke-test/test_smoke_grpc.ts /tmp/test-hyperswitch-js-grpc/
	@cp $(SDK_ROOT)/smoke-test/tsconfig.json      /tmp/test-hyperswitch-js-grpc/
	@cp $(REPO_ROOT)/sdk/generated/flows.json /tmp/test-hyperswitch-js-grpc/
	@if [ -f $(REPO_ROOT)/creds.json ]; then \
		cp $(REPO_ROOT)/creds.json /tmp/test-hyperswitch-js-grpc/; \
	else \
		echo "  Note: no creds.json found, test will use placeholder credentials"; \
	fi
	@cd /tmp/test-hyperswitch-js-grpc && npx tsc && \
		NODE_PATH=/tmp/test-hyperswitch-js-grpc/node_modules FORCE_COLOR=1 node test_smoke_grpc.js \
		--connectors $(CONNECTORS) --examples-dir $(REPO_ROOT)/examples; \
		EXIT=$$?; \
		pkill -f grpc-server 2>/dev/null || true; \
		rm -rf /tmp/test-hyperswitch-js-grpc; \
		exit $$EXIT

# ---------------------------------------------------------------------------
# publish
# Publishes the package to npm registry.
# Requires NPM_TOKEN to be set or logged in via npm login.
# ---------------------------------------------------------------------------
publish: build-ts
	@if [ -z "$(NPM_TOKEN)" ]; then \
		echo "Error: NPM_TOKEN is not set"; \
		echo "Set it with: export NPM_TOKEN=npm_xxxxx"; \
		exit 1; \
	fi
	@echo "Configuring npm with authentication token..."
	@npm config set //registry.npmjs.org/:_authToken=$(NPM_TOKEN)
	@echo "Publishing hyperswitch-prism to npm..."
	@npm publish --access public
	@echo "Published hyperswitch-prism to npm registry."

# ---------------------------------------------------------------------------
# Pre-publish checks
# ---------------------------------------------------------------------------

# publish-check-auth
# Verifies that NPM_TOKEN environment variable is set or npm whoami succeeds.
publish-check-auth:
	@if [ -n "$(NPM_TOKEN)" ]; then \
		echo "NPM_TOKEN is set."; \
	elif npm whoami >/dev/null 2>&1; then \
		echo "npm authenticated as: $$(npm whoami)"; \
	else \
		echo "Error: Neither NPM_TOKEN nor npm authentication is configured"; \
		echo "Set NPM_TOKEN or run 'npm login' first."; \
		exit 1; \
	fi

# publish-check-version
# Checks if $(PACKAGE_VERSION) already exists on npm registry.
# Exits with error if version already exists (prevents accidental republish).
publish-check-version:
	@echo "Checking if hyperswitch-prism $(PACKAGE_VERSION) exists on npm..."
	@if npm view hyperswitch-prism@$(PACKAGE_VERSION) version >/dev/null 2>&1; then \
		echo "Error: Version $(PACKAGE_VERSION) already exists on npm"; \
		echo "Increment the version in package.json before publishing."; \
		exit 1; \
	fi
	@echo "Version $(PACKAGE_VERSION) does not exist on npm - OK to publish."

# publish-check-dry-run
# Simulates publish without actually publishing.
# Uses npm publish --dry-run and npm pack to verify package contents.
publish-check-dry-run: build-ts
	@echo "Running npm publish dry-run..."
	@npm publish --dry-run --access public
	@echo "Creating local tarball..."
	@npm pack --pack-destination /tmp
	@echo "Tarball contents:"
	@tar -tzf /tmp/hyperswitch-prism-$(PACKAGE_VERSION).tgz | head -20
	@rm -f /tmp/hyperswitch-prism-$(PACKAGE_VERSION).tgz
	@echo "Dry-run validation passed."

# publish-check
# Runs all pre-publish checks in sequence.
publish-check: publish-check-auth publish-check-version publish-check-dry-run
	@echo "All JavaScript pre-publish checks passed."

clean:
	@rm -rf $(GENERATED_OUT) node_modules
	@rm -f $(SDK_ROOT)src/payments/_generated_flows.js \
	        $(SDK_ROOT)src/payments/_generated_connector_client_flows.ts \
	        $(SDK_ROOT)src/payments/_generated_uniffi_client_flows.ts
