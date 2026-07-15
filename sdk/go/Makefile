.PHONY: all build generate generate-proto generate-bindings generate-flows test test-mock clean install \
        test-package test-package-mock test-composite test-webhook \
        publish publish-check publish-check-auth publish-check-version publish-check-dry-run dist

# SDK-specific paths
MAKEFILE_DIR := $(dir $(abspath $(lastword $(MAKEFILE_LIST))))
SDK_ROOT     := $(patsubst %/,%,$(MAKEFILE_DIR))
REPO_ROOT    := $(shell cd $(SDK_ROOT)/../.. && pwd)

# Include common SDK build configuration
include $(REPO_ROOT)/sdk/common.mk

# Go-specific paths
GENERATED_GO     := $(SDK_ROOT)/generated
UNIFFI_GO        := $(SDK_ROOT)/generated/uniffi
PAYMENTS_PKG     := $(SDK_ROOT)/payments
BINDGEN_GO       := $(HOME)/.cargo/bin/uniffi-bindgen-go
PROTOC_GEN_GO    := $(shell which protoc-gen-go 2>/dev/null || echo $(HOME)/go/bin/protoc-gen-go)

# Version for distribution and publishing
PACKAGE_VERSION := $(if $(VERSION),$(VERSION),$(shell \
	cd $(REPO_ROOT) && git describe --tags --match 'sdk/go/v*' --abbrev=0 2>/dev/null | sed 's/sdk\/go\///' || echo "0.0.0-dev"))

# Proto excludes (same as Java SDK)
PROTO_EXCLUDES := health_check.proto composite_payment.proto composite_services.proto
PROTO_FILES_GO := $(shell ls $(PROTO_DIR)/*.proto | xargs -n1 basename | grep -v -E "$(subst . ,|,$(PROTO_EXCLUDES))")

# ---------------------------------------------------------------------------
# all
# Default target: build everything.
# ---------------------------------------------------------------------------
all: generate build

# ---------------------------------------------------------------------------
# install
# Install Go dependencies.
# ---------------------------------------------------------------------------
install:
	@echo "Installing Go dependencies..."
	cd $(SDK_ROOT) && go mod tidy

# ---------------------------------------------------------------------------
# generate / generate-all
# Full regeneration: proto types, UniFFI bindings, and flow clients.
# ---------------------------------------------------------------------------
generate generate-all: generate-proto generate-bindings generate-flows

# ---------------------------------------------------------------------------
# generate-proto
# Generate Go protobuf stubs from .proto files.
# ---------------------------------------------------------------------------
generate-proto:
	@echo "Generating Go protobuf stubs..."
	@mkdir -p $(GENERATED_GO)/payments
	@cd $(REPO_ROOT) && protoc \
		-I $(PROTO_DIR) \
		--plugin=protoc-gen-go=$(PROTOC_GEN_GO) \
		--go_out=$(GENERATED_GO) \
		--go_opt=paths=source_relative \
		$(addprefix $(PROTO_DIR)/,$(PROTO_FILES_GO))
	@mv $(GENERATED_GO)/*.pb.go $(GENERATED_GO)/payments/ 2>/dev/null || true
	@echo "Proto stubs generated in $(GENERATED_GO)/payments/"

# ---------------------------------------------------------------------------
# generate-bindings
# Generate UniFFI Go bindings from the compiled FFI library.
# ---------------------------------------------------------------------------
generate-bindings: build-ffi-lib
	@echo "Generating UniFFI Go bindings..."
	@mkdir -p $(UNIFFI_GO)
	@$(BINDGEN_GO) \
		--library $(LIBRARY) \
		--out-dir $(UNIFFI_GO) \
		--no-format
	@mkdir -p $(UNIFFI_GO)/connector_service_ffi/darwin
	@mkdir -p $(UNIFFI_GO)/connector_service_ffi/linux
ifeq ($(LIB_EXT),dylib)
	@cp $(LIBRARY) $(UNIFFI_GO)/connector_service_ffi/darwin/
else
	@cp $(LIBRARY) $(UNIFFI_GO)/connector_service_ffi/linux/
endif
	@rm -rf $(UNIFFI_GO)/superposition_client $(UNIFFI_GO)/superposition_types
	@echo "Go bindings generated in $(UNIFFI_GO)/"

# ---------------------------------------------------------------------------
# generate-flows
# Generate Go flow methods and per-service clients.
# ---------------------------------------------------------------------------
generate-flows:
	@echo "Generating Go flow methods..."
	@python3 $(REPO_ROOT)/scripts/generators/code/generate.py --lang go
	@echo "Go flow methods generated."

# ---------------------------------------------------------------------------
# build
# Build all Go packages.
# ---------------------------------------------------------------------------
build:
	@echo "Building Go SDK..."
	cd $(SDK_ROOT) && go build ./...
	@echo "Build complete."

# ---------------------------------------------------------------------------
# test
# Run Go tests.
# ---------------------------------------------------------------------------
test:
	@echo "Running Go tests..."
	cd $(SDK_ROOT) && go test -v ./...

# ---------------------------------------------------------------------------
# test-mock
# Run mock tests (no HTTP calls).
# ---------------------------------------------------------------------------
test-mock:
	@echo "Running Go mock tests..."
	cd $(SDK_ROOT) && go test -v -tags mock ./...

# ---------------------------------------------------------------------------
# Smoke test targets
# ---------------------------------------------------------------------------

CREDS_FILE      ?= $(REPO_ROOT)/creds.json
CONNECTORS      ?= stripe
SMOKE_TEST_DIR  := $(SDK_ROOT)/smoke-test

test-package:
	@echo "Running Go smoke test..."
	@# Ensure native library is present in the platform subdirectory for CGO linking
	@if [ -f "$(LIBRARY)" ]; then \
		if [ "$(LIB_EXT)" = "dylib" ] && [ ! -f "$(UNIFFI_GO)/connector_service_ffi/darwin/libconnector_service_ffi.dylib" ]; then \
			cp -f "$(LIBRARY)" "$(UNIFFI_GO)/connector_service_ffi/darwin/"; \
		elif [ "$(LIB_EXT)" = "so" ] && [ ! -f "$(UNIFFI_GO)/connector_service_ffi/linux/libconnector_service_ffi.so" ]; then \
			cp -f "$(LIBRARY)" "$(UNIFFI_GO)/connector_service_ffi/linux/"; \
		fi; \
	fi
	@if [ ! -f $(CREDS_FILE) ]; then \
		echo "Error: $(CREDS_FILE) not found. Set CREDS_FILE=/path/to/creds.json"; \
		exit 1; \
	fi
	@cd $(SMOKE_TEST_DIR) && FLOWS_JSON_PATH=$(REPO_ROOT)/sdk/generated/flows.json \
		go run . --creds-file $(CREDS_FILE) --connectors $(CONNECTORS)
	@echo "Running Go composite smoke test..."
	@cd $(SMOKE_TEST_DIR)/composite && go run . --creds-file $(CREDS_FILE)
	@echo "Running Go webhook smoke test..."
	@cd $(SMOKE_TEST_DIR)/webhook && go run .

test-package-mock:
	@echo "Running Go smoke test in MOCK mode..."
	@# Ensure native library is present in the platform subdirectory for CGO linking
	@if [ -f "$(LIBRARY)" ]; then \
		if [ "$(LIB_EXT)" = "dylib" ] && [ ! -f "$(UNIFFI_GO)/connector_service_ffi/darwin/libconnector_service_ffi.dylib" ]; then \
			cp -f "$(LIBRARY)" "$(UNIFFI_GO)/connector_service_ffi/darwin/"; \
		elif [ "$(LIB_EXT)" = "so" ] && [ ! -f "$(UNIFFI_GO)/connector_service_ffi/linux/libconnector_service_ffi.so" ]; then \
			cp -f "$(LIBRARY)" "$(UNIFFI_GO)/connector_service_ffi/linux/"; \
		fi; \
	fi
	@if [ ! -f $(CREDS_FILE) ]; then \
		echo "Error: $(CREDS_FILE) not found. Set CREDS_FILE=/path/to/creds.json"; \
		exit 1; \
	fi
	@cd $(SMOKE_TEST_DIR) && FLOWS_JSON_PATH=$(REPO_ROOT)/sdk/generated/flows.json \
		go run . --creds-file $(CREDS_FILE) --connectors $(CONNECTORS) --mock

# ---------------------------------------------------------------------------
# Composite test target
# ---------------------------------------------------------------------------

test-composite:
	@echo "Running Go composite smoke test..."
	@cd $(SMOKE_TEST_DIR)/composite && go run . --creds-file $(CREDS_FILE)

# ---------------------------------------------------------------------------
# Webhook test target
# ---------------------------------------------------------------------------

test-webhook:
	@echo "Running Go webhook smoke test..."
	@cd $(SMOKE_TEST_DIR)/webhook && go run .

# ---------------------------------------------------------------------------
# clean
# Remove generated files.
# ---------------------------------------------------------------------------
clean:
	@# Remove generated protobuf files
	rm -f $(GENERATED_GO)/payments/*.pb.go
	@# Remove generated uniffi files but preserve manual files and native libraries
	@find $(UNIFFI_GO)/connector_service_ffi -maxdepth 1 -type f \
		! -name 'cgo.go' \
		! -name 'cgo_darwin.go' \
		! -name 'cgo_linux.go' \
		-delete 2>/dev/null || true
	@rm -rf $(UNIFFI_GO)/superposition_client $(UNIFFI_GO)/superposition_types
	rm -f $(PAYMENTS_PKG)/zz_generated_client.go
	cd $(SDK_ROOT) && go clean ./...

# ---------------------------------------------------------------------------
# dist
# Build distribution archive with native libraries.
# ---------------------------------------------------------------------------
dist: build
	@echo "Building Go SDK distribution..."
	@mkdir -p $(ARTIFACTS_DIR)/sdk-go
	@tar czf $(ARTIFACTS_DIR)/sdk-go/prism-go-$(PACKAGE_VERSION).tar.gz \
		-C $(SDK_ROOT) go.mod payments/ generated/ README.md
	@echo "Distribution created in $(ARTIFACTS_DIR)/sdk-go/"

# ---------------------------------------------------------------------------
# Pre-publish checks
# ---------------------------------------------------------------------------

# publish-check-auth
# Verifies git is available for tagging.
publish-check-auth:
	@which git >/dev/null 2>&1 || (echo "Error: git is not installed" && exit 1)
	@echo "Git is available."

# publish-check-version
# Checks if sdk/go/v$(PACKAGE_VERSION) tag already exists.
publish-check-version:
	@echo "Checking if tag sdk/go/v$(PACKAGE_VERSION) exists..."
	@if git -C $(REPO_ROOT) rev-parse "sdk/go/v$(PACKAGE_VERSION)" >/dev/null 2>&1; then \
		echo "Error: Tag sdk/go/v$(PACKAGE_VERSION) already exists"; \
		exit 1; \
	fi
	@echo "Tag sdk/go/v$(PACKAGE_VERSION) does not exist - OK to publish."

# publish-check-dry-run
# Validates the module builds cleanly before tagging.
publish-check-dry-run: build
	@echo "Module compiles successfully - OK to publish."

# publish-check
# Runs all pre-publish checks in sequence.
publish-check: publish-check-auth publish-check-version publish-check-dry-run
	@echo "All Go pre-publish checks passed."

# ---------------------------------------------------------------------------
# publish
# Creates and pushes the git tag. Go proxy mirrors automatically.
# ---------------------------------------------------------------------------
publish:
	@if [ -z "$(PACKAGE_VERSION)" ] || [ "$(PACKAGE_VERSION)" = "0.0.0-dev" ]; then \
		echo "Error: Cannot publish dev version. Set VERSION=X.Y.Z"; \
		exit 1; \
	fi
	@echo "Creating tag sdk/go/v$(PACKAGE_VERSION)..."
	@git -C $(REPO_ROOT) tag -a "sdk/go/v$(PACKAGE_VERSION)" -m "Release sdk/go v$(PACKAGE_VERSION)"
	@echo "Pushing tag sdk/go/v$(PACKAGE_VERSION)..."
	@git -C $(REPO_ROOT) push origin "sdk/go/v$(PACKAGE_VERSION)"
	@echo "Published! Go proxy will mirror within minutes."
	@echo "Verify with: go list -m -versions github.com/juspay/hyperswitch-prism/sdk/go"
