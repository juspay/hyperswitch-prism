.PHONY: install-deps install-codegen-deps generate-bindings generate-proto generate-all \
        generate-flows pack-archive install test-package test-composite test-grpc test-grpc-no-server dist publish clean \
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
PACKAGE_DIR      := $(ARTIFACTS_DIR)/sdk-python
PACKAGE_NAME     := $(shell python3 -c "t = open('$(SDK_ROOT)/pyproject.toml').read(); print([l.split('=')[1].strip().strip('\"') for l in t.split('\n') if l.startswith('name =')][0])")
PACKAGE_VERSION  := $(shell python3 -c "t = open('$(SDK_ROOT)/pyproject.toml').read(); print([l.split('=')[1].strip().strip('\"') for l in t.split('\n') if l.startswith('version =')][0])")
WHEEL_FILE       := $(PACKAGE_DIR)/$(subst -,_,$(PACKAGE_NAME))-$(PACKAGE_VERSION)-py3-none-any.whl

# Note: BINDGEN is defined in common.mk

# ---------------------------------------------------------------------------
# install-deps
# Installs Python build-time dependencies required for code generation.
# Checks for tool existence before installing. In Nix environments tools are
# provided by the shell so installation is skipped (IN_NIX_SHELL is set).
# Outside Nix, installs grpcio-tools for protoc Python plugin if absent.
# ---------------------------------------------------------------------------
install-deps:
	@command -v python3 >/dev/null 2>&1 || \
		(echo "Error: python3 not found. Install via nix or your system package manager." && exit 1)
	@command -v cargo >/dev/null 2>&1 || \
		(echo "Error: cargo not found. Install Rust via rustup." && exit 1)
	@if [ -z "$${IN_NIX_SHELL:-}" ]; then \
		python3 -c "import grpc_tools" 2>/dev/null || \
			(echo "  → Installing grpcio-tools..." && \
			 pip3 install grpcio-tools --quiet 2>/dev/null || \
			 pip3 install grpcio-tools --user --quiet); \
	fi
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
# Builds the uniffi-bindgen binary then runs uniffi-bindgen generate.
# uniffi-bindgen lives in its own crate (crates/internal/uniffi-bindgen) so the ffi
# library never pulls in the uniffi/cli codegen deps. The output binary lands
# in the same target/<PLATFORM>/release/ directory as before.
# ---------------------------------------------------------------------------
generate-bindings: build-ffi-lib
	@echo "Building uniffi-bindgen..."
	@cd $(REPO_ROOT) && cargo build -p uniffi-bindgen \
		--profile $(PROFILE) --target $(PLATFORM)
	@echo "Generating UniFFI Python bindings from $(LIBRARY)..."
	@mkdir -p $(GENERATED_OUT)
	@cp -f $(LIBRARY) $(GENERATED_OUT)/
	@$(BINDGEN) generate \
		--library $(LIBRARY) \
		--language python \
		--out-dir $(GENERATED_OUT)
	@ls $(GENERATED_OUT)/connector_service_ffi.py > /dev/null 2>&1 || \
		(echo "ERROR: uniffi-bindgen generated no Python files." \
		 "Check that $(LIBRARY) was built with --features uniffi (uniffi::setup_scaffolding)." && exit 1)
	@echo "Python bindings generated in $(GENERATED_OUT)/"

# ---------------------------------------------------------------------------
# generate-proto
# Generates Python protobuf stubs from .proto files into the generated output
# directory, alongside the UniFFI Python bindings.
# Rewrites bare imports to relative package imports so the stubs work correctly
# under the payments package layout.
# ---------------------------------------------------------------------------
generate-proto: install-deps
	@echo "Generating Python protobuf stubs..."
	@mkdir -p $(GENERATED_OUT)
	@python3 -m grpc_tools.protoc \
		-I $(PROTO_DIR) \
		--python_out=$(GENERATED_OUT) \
		$(PROTO_DIR)/payment.proto $(PROTO_DIR)/payment_methods.proto $(PROTO_DIR)/sdk_config.proto
	@# TODO: protoc --python_out generates bare imports (e.g. "import payment_methods_pb2")
	@# which break under a package layout. Rewrite to relative imports until protoc
	@# adds a --python_relative_imports flag or we switch to a package-aware plugin.
	@cd $(GENERATED_OUT) && for f in *_pb2.py; do \
		sed -i.bak 's/^import \(.*_pb2\) as/from . import \1 as/' "$$f" && rm -f "$$f.bak"; \
	done
	@touch $(GENERATED_OUT)/__init__.py
	@echo "Proto stubs generated in $(GENERATED_OUT)/"

# ---------------------------------------------------------------------------
# generate-all
# Runs all code-generation steps: UniFFI Python bindings and Python protobuf
# stubs. Proto is generated first to ensure the __init__.py marker exists
# before the UniFFI wrapper references the package.
# ---------------------------------------------------------------------------
generate-all: generate-proto generate-bindings generate-flows

# ---------------------------------------------------------------------------
# generate-flows
# Generates flow methods (authorize, capture, etc.) by running the SDK
# codegen script. Depends on install-codegen-deps so jinja2 etc. are available (CI and local).
# ---------------------------------------------------------------------------
generate-flows: install-codegen-deps
	@echo "Generating Python flow methods..."
	@python3 $(REPO_ROOT)/scripts/generators/code/generate.py --lang python
	@echo "Python flow methods generated."

# ---------------------------------------------------------------------------
# pack-archive
# Builds the SDK distribution wheel (.whl) using pip.
# Assumes generated/ already contains the UniFFI Python bindings, proto stubs,
# and the native FFI library. Does not rebuild any sources.
# ---------------------------------------------------------------------------
pack-archive:
	@mkdir -p $(PACKAGE_DIR)
	@rm -f $(PACKAGE_DIR)/$(subst -,_,$(PACKAGE_NAME))-*.whl
	@rm -rf $(SDK_ROOT)/build
	@cd $(SDK_ROOT) && python3 -m pip wheel . --no-deps --wheel-dir $(PACKAGE_DIR)/
	@echo "Wheel built: $(WHEEL_FILE)"

# ---------------------------------------------------------------------------
# install
# Builds the SDK wheel. Python wheels do not require a system-level install
# step analogous to Maven local — users install directly from the .whl file.
# This target is a no-op beyond pack-archive and exists for interface parity
# with the other SDK Makefiles.
# ---------------------------------------------------------------------------
install: pack-archive

# ---------------------------------------------------------------------------
# test-package
# Full end-to-end local test: builds the FFI library, generates all code
# artifacts, packs the wheel, then installs it into an isolated temp directory
# and runs the smoke test to verify the package works correctly.
# The temp directory is cleaned up after the test.
# ---------------------------------------------------------------------------
test-package: generate-all pack-archive
	@echo "Testing packed wheel in isolation..."
	@rm -rf /tmp/test-hyperswitch-py
	@mkdir -p /tmp/test-hyperswitch-py
	@cp $(SDK_ROOT)/smoke-test/test_smoke.py /tmp/test-hyperswitch-py/
	@# Copy credentials file if it exists
	@if [ -f $(REPO_ROOT)/creds.json ]; then \
		cp $(REPO_ROOT)/creds.json /tmp/test-hyperswitch-py/; \
	else \
		echo "  Note: creds.json not found, test will use placeholder credentials"; \
	fi
	@python3 -m pip install \
		--target /tmp/test-hyperswitch-py \
		$(WHEEL_FILE) \
		--force-reinstall
	@cp $(REPO_ROOT)/sdk/generated/flows.json /tmp/test-hyperswitch-py/
	@cd /tmp/test-hyperswitch-py && python3 test_smoke.py --connectors $(CONNECTORS) --examples-dir $(REPO_ROOT)/examples
	@echo "Running composite flow test..."
	@cp $(SDK_ROOT)/smoke-test/test_smoke_composite.py /tmp/test-hyperswitch-py/
	@cd /tmp/test-hyperswitch-py && python3 test_smoke_composite.py --creds-file creds.json
	@rm -rf /tmp/test-hyperswitch-py
	@echo "Package test passed."

# ---------------------------------------------------------------------------
# test-package-mock
# Same as test-package but runs in MOCK mode (no real HTTP calls).
# Uses generated harnesses instead of examples and verifies req_transformer only.
# ---------------------------------------------------------------------------
test-package-mock: generate-all pack-archive
	@echo "Testing packed wheel in MOCK mode (no real HTTP)..."
	@rm -rf /tmp/test-hyperswitch-py-mock
	@mkdir -p /tmp/test-hyperswitch-py-mock
	@cp $(SDK_ROOT)/smoke-test/test_smoke.py /tmp/test-hyperswitch-py-mock/
	@# Copy mock credentials file
	@if [ -f $(REPO_ROOT)/creds_dummy.json ]; then \
		cp $(REPO_ROOT)/creds_dummy.json /tmp/test-hyperswitch-py-mock/creds.json; \
	else \
		echo "  Note: creds_dummy.json not found, test will use placeholder credentials"; \
	fi
	@python3 -m pip install \
		--target /tmp/test-hyperswitch-py-mock \
		$(WHEEL_FILE) \
		--force-reinstall
	@cp $(REPO_ROOT)/sdk/generated/flows.json /tmp/test-hyperswitch-py-mock/
	@# Copy generated harnesses to examples directory structure expected by smoke test
	@mkdir -p /tmp/test-hyperswitch-py-mock/examples/$(CONNECTORS)
	@cp $(SDK_ROOT)/smoke-test/generated/$(CONNECTORS)/$(CONNECTORS).py /tmp/test-hyperswitch-py-mock/examples/$(CONNECTORS)/
	@cd /tmp/test-hyperswitch-py-mock && python3 test_smoke.py --connectors $(CONNECTORS) --creds-file creds.json --mock
	@rm -rf /tmp/test-hyperswitch-py-mock
	@echo "Package mock test passed."

# ---------------------------------------------------------------------------
# test-composite
# Runs the composite flow smoke test (access token + authorize flow).
# Tests the PayPal access token composite flow in an isolated environment.
# ---------------------------------------------------------------------------
test-composite: generate-all pack-archive
	@echo "Testing composite flow in isolation..."
	@rm -rf /tmp/test-hyperswitch-py-composite
	@mkdir -p /tmp/test-hyperswitch-py-composite
	@cp $(SDK_ROOT)/smoke-test/test_smoke_composite.py /tmp/test-hyperswitch-py-composite/
	@# Copy credentials file if it exists
	@if [ -f $(REPO_ROOT)/creds.json ]; then \
		cp $(REPO_ROOT)/creds.json /tmp/test-hyperswitch-py-composite/; \
	fi
	@python3 -m pip install \
		--target /tmp/test-hyperswitch-py-composite \
		$(WHEEL_FILE) \
		--force-reinstall
	@cd /tmp/test-hyperswitch-py-composite && python3 test_smoke_composite.py --creds-file creds.json
	@rm -rf /tmp/test-hyperswitch-py-composite
	@echo "Composite flow test passed."

# ---------------------------------------------------------------------------
# test-grpc
# End-to-end gRPC smoke test: builds both FFI libraries, packs the wheel,
# installs it in an isolated temp directory, then runs test_smoke_grpc.py.
# ---------------------------------------------------------------------------
GRPC_PROFILE    ?= release-fast
GRPC_SERVER_BIN := $(REPO_ROOT)/target/$(GRPC_PROFILE)/grpc-server

# Run gRPC test assuming server is already running (used by top-level test-grpc)
test-grpc-no-server: build-ffi-lib build-grpc-ffi-lib generate-all pack-archive
	@echo "Testing gRPC flows in isolation..."
	@rm -rf /tmp/test-hyperswitch-py-grpc
	@mkdir -p /tmp/test-hyperswitch-py-grpc
	@python3 -m pip install \
		--target /tmp/test-hyperswitch-py-grpc \
		$(WHEEL_FILE) \
		--force-reinstall
	@cp -f $(GRPC_FFI_LIBRARY) /tmp/test-hyperswitch-py-grpc/payments/generated/
	@cp $(SDK_ROOT)/smoke-test/test_smoke_grpc.py /tmp/test-hyperswitch-py-grpc/
	@cp $(REPO_ROOT)/sdk/generated/flows.json /tmp/test-hyperswitch-py-grpc/
	@if [ -f $(REPO_ROOT)/creds.json ]; then \
		cp $(REPO_ROOT)/creds.json /tmp/test-hyperswitch-py-grpc/; \
	else \
		echo "  Note: no creds.json found, test will use placeholder credentials"; \
	fi
	@cd /tmp/test-hyperswitch-py-grpc && PYTHONPATH=/tmp/test-hyperswitch-py-grpc FORCE_COLOR=1 \
		python3 test_smoke_grpc.py \
		--connectors $(CONNECTORS) --examples-dir $(REPO_ROOT)/examples
	@rm -rf /tmp/test-hyperswitch-py-grpc

test-grpc: build-ffi-lib build-grpc-ffi-lib generate-all pack-archive
	@echo "Building grpc-server ($(GRPC_PROFILE))..."
	@cd $(REPO_ROOT) && cargo build -p grpc-server --profile $(GRPC_PROFILE) 2>&1
	@echo "Starting gRPC server..."
	@pkill -KILL -f grpc-server 2>/dev/null || true
	@sleep 1
	@cd $(REPO_ROOT) && $(GRPC_SERVER_BIN) > /tmp/grpc-server-py.log 2>&1 &
	@sleep 2
	@echo "Testing gRPC flows in isolation..."
	@rm -rf /tmp/test-hyperswitch-py-grpc
	@mkdir -p /tmp/test-hyperswitch-py-grpc
	@python3 -m pip install \
		--target /tmp/test-hyperswitch-py-grpc \
		$(WHEEL_FILE) \
		--force-reinstall
	@cp -f $(GRPC_FFI_LIBRARY) /tmp/test-hyperswitch-py-grpc/payments/generated/
	@cp $(SDK_ROOT)/smoke-test/test_smoke_grpc.py /tmp/test-hyperswitch-py-grpc/
	@if [ -f $(REPO_ROOT)/creds.json ]; then \
		cp $(REPO_ROOT)/creds.json /tmp/test-hyperswitch-py-grpc/; \
	else \
		echo "  Note: no creds.json found, test will use placeholder credentials"; \
	fi
	@cd /tmp/test-hyperswitch-py-grpc && PYTHONPATH=/tmp/test-hyperswitch-py-grpc FORCE_COLOR=1 \
		python3 test_smoke_grpc.py \
		--connectors $(CONNECTORS) --examples-dir $(REPO_ROOT)/examples; \
		EXIT=$$?; \
		pkill -f grpc-server 2>/dev/null || true; \
		rm -rf /tmp/test-hyperswitch-py-grpc; \
		exit $$EXIT

# ---------------------------------------------------------------------------
# dist
# Builds the distribution wheel for all supported platforms.
# Copies pre-built native binaries for each target platform into the generated
# output directory, generates proto stubs, then packs the wheel. UniFFI Python
# bindings are expected to be already present (generated by a prior
# generate-bindings step or downloaded from a CI artifact).
# ---------------------------------------------------------------------------
dist:
	@echo "Building Python SDK distribution wheel..."
	@mkdir -p $(GENERATED_OUT)
	@cp -f $(REPO_ROOT)/target/x86_64-unknown-linux-gnu/release/libconnector_service_ffi.so \
		$(GENERATED_OUT)/ 2>/dev/null || echo "  Note: Linux x86_64 binary not found (skipping)"
	@cp -f $(REPO_ROOT)/target/aarch64-apple-darwin/release/libconnector_service_ffi.dylib \
		$(GENERATED_OUT)/ 2>/dev/null || echo "  Note: macOS aarch64 binary not found (skipping)"
	@# dist always ships release-optimised binaries (lto=thin, codegen-units=1).
	@$(MAKE) -C $(SDK_ROOT) pack-archive
	@echo "Distribution wheel created in $(PACKAGE_DIR)/"

# ---------------------------------------------------------------------------
# publish
# Publishes the package to PyPI.
# Requires TWINE_USERNAME and TWINE_PASSWORD to be set.
# ---------------------------------------------------------------------------
publish: pack-archive
	@if [ -z "$(TWINE_USERNAME)" ] || [ -z "$(TWINE_PASSWORD)" ]; then \
		echo "Error: TWINE_USERNAME or TWINE_PASSWORD is not set"; \
		echo "Set them with:"; \
		echo "  export TWINE_USERNAME=__token__"; \
		echo "  export TWINE_PASSWORD=pypi-xxxxx"; \
		exit 1; \
	fi
	@echo "Publishing hyperswitch-prism to PyPI..."
	@python3 -m pip install twine --quiet
	@python3 -m twine upload $(WHEEL_FILE)
	@echo "Published hyperswitch-prism to PyPI."

# ---------------------------------------------------------------------------
# Pre-publish checks
# ---------------------------------------------------------------------------

# publish-check-auth
# Verifies that TWINE_USERNAME and TWINE_PASSWORD environment variables are set.
publish-check-auth:
	@if [ -z "$(TWINE_USERNAME)" ]; then \
		echo "Error: TWINE_USERNAME is not set"; \
		exit 1; \
	fi
	@if [ -z "$(TWINE_PASSWORD)" ]; then \
		echo "Error: TWINE_PASSWORD is not set"; \
		exit 1; \
	fi
	@echo "PyPI authentication credentials are configured."

# publish-check-version
# Checks if $(PACKAGE_VERSION) already exists on PyPI.
# Exits with error if version already exists (prevents accidental republish).
publish-check-version:
	@echo "Checking if hyperswitch-prism $(PACKAGE_VERSION) exists on PyPI..."
	@if curl -s https://pypi.org/pypi/hyperswitch-prism/json | \
		python3 -c "import sys, json; data=json.load(sys.stdin); versions=list(data.get('releases', {}).keys()); exit(0 if '$(PACKAGE_VERSION)' in versions else 1)" 2>/dev/null; then \
		echo "Error: Version $(PACKAGE_VERSION) already exists on PyPI"; \
		echo "Increment the version in pyproject.toml before publishing."; \
		exit 1; \
	fi
	@echo "Version $(PACKAGE_VERSION) does not exist on PyPI - OK to publish."

# publish-check-dry-run
# Validates the distribution package without publishing.
# Uses twine check to verify wheel metadata and lists wheel contents.
publish-check-dry-run: pack-archive
	@echo "Validating wheel package..."
	@python3 -m pip install twine --quiet 2>/dev/null || true
	@python3 -m twine check $(WHEEL_FILE)
	@echo "Wheel contents:"
	@python3 -m zipfile -l $(WHEEL_FILE) | head -20
	@echo "Dry-run validation passed."

# publish-check
# Runs all pre-publish checks in sequence.
publish-check: publish-check-auth publish-check-version publish-check-dry-run
	@echo "All Python pre-publish checks passed."

clean:
	@rm -rf $(GENERATED_OUT)
	@rm -f $(SDK_ROOT)/src/payments/_generated_flows.py \
	        $(SDK_ROOT)/src/payments/connector_client.pyi
	@rm -rf $(SDK_ROOT)/build $(SDK_ROOT)/*.egg-info $(SDK_ROOT)/src/*.egg-info
