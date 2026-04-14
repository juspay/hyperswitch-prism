.PHONY: install-deps install-codegen-deps generate-bindings generate-proto generate-all \
        generate-flows pack-archive install test-package test-grpc test-grpc-no-server dist publish clean \
        publish-check-auth publish-check-version publish-check-dry-run publish-check

# SDK-specific paths (define BEFORE including common.mk)
MAKEFILE_DIR := $(dir $(abspath $(lastword $(MAKEFILE_LIST))))
SDK_ROOT     := $(MAKEFILE_DIR)

# Connectors to test (comma-separated, default: stripe)
CONNECTORS ?= stripe

# Include common SDK build configuration
include $(SDK_ROOT)/../common.mk

# SDK-specific paths continued
GENERATED_KOTLIN := $(SDK_ROOT)/src/main/kotlin/generated
GENERATED_JAVA   := $(SDK_ROOT)/src/main/java/generated
NATIVE_DIR       := $(SDK_ROOT)/src/main/resources/native
PACKAGE_DIR      := $(ARTIFACTS_DIR)/sdk-java
PACKAGE_VERSION  := $(shell cd $(SDK_ROOT) && ./gradlew properties -q 2>/dev/null | grep "^version:" | awk '{print $$2}')

# Note: BINDGEN is defined in common.mk

# ---------------------------------------------------------------------------
# install-deps
# Checks for required build tools. In Nix environments tools are provided by
# the shell so this is effectively a no-op. Outside Nix it exits with a helpful
# message for any missing tool rather than failing silently mid-build.
# ---------------------------------------------------------------------------
install-deps:
	@command -v protoc >/dev/null 2>&1 || \
		(echo "Error: protoc not found. Install via nix or 'apt install protobuf-compiler'." && exit 1)
	@command -v java >/dev/null 2>&1 || \
		(echo "Error: java not found. Install via nix or a JDK package." && exit 1)
	@command -v cargo >/dev/null 2>&1 || \
		(echo "Error: cargo not found. Install Rust via rustup." && exit 1)
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
	@echo "Generating UniFFI Kotlin bindings from $(LIBRARY)..."
	@mkdir -p $(GENERATED_KOTLIN) $(NATIVE_DIR)
	@rm -f $(NATIVE_DIR)/libconnector_service_ffi.$(LIB_EXT)
	@cp -f $(LIBRARY) $(NATIVE_DIR)/
	@$(BINDGEN) generate \
		--library $(LIBRARY) \
		--language kotlin \
		--out-dir $(GENERATED_KOTLIN)
	@find $(GENERATED_KOTLIN) -name '*.kt' | grep -q . || \
		(echo "ERROR: uniffi-bindgen generated no Kotlin files." \
		 "Check that $(LIBRARY) was built with --features uniffi (uniffi::setup_scaffolding)." && exit 1)
	@# TODO: UniFFI 0.28 codegen produces IntegrationException(val message: String)
	@# which conflicts with the inherited Exception.message property. Patch until
	@# upstream fixes Kotlin codegen. See: https://github.com/mozilla/uniffi-rs/issues/2182
	@find $(GENERATED_KOTLIN) -name '*.kt' -exec sed -i.bak \
		's/^        val `message`: kotlin\.String/        override val `message`: kotlin.String/' {} +
	@find $(GENERATED_KOTLIN) -name '*.kt' -exec sed -i.bak \
		'/class IntegrationException/,/^    }/{/^        override val message/d;/^            get() = "message=/d;}' {} +
	@find $(GENERATED_KOTLIN) -name '*.kt.bak' -delete
	@echo "Kotlin bindings generated in $(GENERATED_KOTLIN)/"

# ---------------------------------------------------------------------------
# generate-proto
# Generates Java protobuf stubs from .proto files into the Java sources
# directory, where they are compiled alongside the UniFFI Kotlin bindings.
# ---------------------------------------------------------------------------
generate-proto:
	@echo "Generating Java protobuf stubs..."
	@mkdir -p $(GENERATED_JAVA)
	@protoc \
		-I $(PROTO_DIR) \
		--java_out=$(GENERATED_JAVA) \
		$(PROTO_DIR)/payment.proto $(PROTO_DIR)/payouts.proto $(PROTO_DIR)/payment_methods.proto $(PROTO_DIR)/sdk_config.proto
	@echo "Proto stubs generated in $(GENERATED_JAVA)/"

# ---------------------------------------------------------------------------
# generate-all
# Runs all code-generation steps: UniFFI Kotlin bindings and Java protobuf
# stubs. Proto is generated first since the Kotlin code may reference those
# types at compile time.
# ---------------------------------------------------------------------------
generate-all: generate-proto generate-bindings generate-flows

# ---------------------------------------------------------------------------
# generate-flows
# Generates flow methods (authorize, capture, etc.) by running the SDK
# codegen script. Assumes descriptor is already generated.
# Depends on install-codegen-deps so jinja2 etc. are available (CI and local).
# ---------------------------------------------------------------------------
generate-flows: install-codegen-deps
	@echo "Generating Kotlin flow methods..."
	@python3 $(REPO_ROOT)/scripts/generators/code/generate.py --lang kotlin
	@echo "Kotlin flow methods generated."

# ---------------------------------------------------------------------------
# pack-archive
# Builds the SDK distribution JAR using Gradle.
# Assumes generated/ directories and native/ already contain the UniFFI
# bindings, proto stubs, and the native FFI library respectively.
# Does not rebuild any sources — call generate-all first if needed.
# ---------------------------------------------------------------------------
pack-archive:
	@mkdir -p $(PACKAGE_DIR)
	@cd $(SDK_ROOT) && $(SDK_ROOT)/gradlew jar
	@rm -f $(PACKAGE_DIR)/prism-*.jar
	@cp $(SDK_ROOT)/build/libs/prism-$(PACKAGE_VERSION).jar $(PACKAGE_DIR)/prism-$(PACKAGE_VERSION).jar
	@echo "JAR built in $(PACKAGE_DIR)/"

# ---------------------------------------------------------------------------
# install
# Builds the SDK JAR and publishes it to the local Maven repository (~/.m2)
# so that other projects on this machine can depend on it via Maven coordinates
# (io.hyperswitch:prism:0.1.0).
# ---------------------------------------------------------------------------
install: pack-archive
	@$(SDK_ROOT)/gradlew publishToMavenLocal
	@echo "Published to ~/.m2/repository/io/hyperswitch/prism/$(PACKAGE_VERSION)/"

# ---------------------------------------------------------------------------
# test-package
# Full end-to-end local test: builds the FFI library, generates all code
# artifacts, installs the SDK to local Maven, then runs the smoke-test Gradle
# project in isolation to verify the published package works correctly.
# ---------------------------------------------------------------------------
test-package: generate-all install
	@echo "Running smoke test..."
	@# Copy credentials file if it exists
	@if [ -f $(REPO_ROOT)/creds.json ]; then \
		cp $(REPO_ROOT)/creds.json $(SDK_ROOT)/smoke-test/; \
	else \
		echo "  Note: creds.json not found, test will use placeholder credentials"; \
	fi
	@cd $(SDK_ROOT)/smoke-test && $(SDK_ROOT)/gradlew run --args="--connectors $(CONNECTORS)"
	@echo "Running composite smoke test..."
	@cd $(SDK_ROOT)/smoke-test && $(SDK_ROOT)/gradlew runComposite
	@echo "Package test passed."

# ---------------------------------------------------------------------------
# test-package-mock
# Same as test-package but runs in MOCK mode (no real HTTP calls).
# Uses generated harnesses instead of examples and verifies req_transformer only.
# ---------------------------------------------------------------------------
test-package-mock: generate-all install
	@echo "Running Kotlin smoke test in MOCK mode..."
	@# Copy generated harness to smoke-test source
	@mkdir -p $(SDK_ROOT)/smoke-test/src/main/kotlin/generated/$(CONNECTORS)
	@cp $(SDK_ROOT)/smoke-test/generated/$(CONNECTORS)/$(CONNECTORS).kt $(SDK_ROOT)/smoke-test/src/main/kotlin/generated/$(CONNECTORS)/
	@if [ -f $(REPO_ROOT)/creds_dummy.json ]; then \
		cp $(REPO_ROOT)/creds_dummy.json $(SDK_ROOT)/smoke-test/creds.json; \
	fi
	@cd $(SDK_ROOT)/smoke-test && $(SDK_ROOT)/gradlew run --args="--connectors $(CONNECTORS) --mock"
	@echo "Package mock test passed."

# ---------------------------------------------------------------------------
# test-grpc
# Builds the grpc-ffi library and runs the Kotlin gRPC smoke test.
# Requires the grpc-server to be running (started by the top-level test-grpc).
# ---------------------------------------------------------------------------
GRPC_PROFILE    ?= release-fast
GRPC_SERVER_BIN := $(REPO_ROOT)/target/$(PLATFORM)/$(GRPC_PROFILE)/grpc-server

# Run gRPC test assuming server is already running (used by top-level test-grpc)
test-grpc-no-server: install
	@echo "Running Kotlin gRPC smoke test..."
	@mkdir -p $(SDK_ROOT)/smoke-test/src/main/resources/native
	@cp -f $(GRPC_FFI_LIBRARY) $(SDK_ROOT)/smoke-test/src/main/resources/native/
	@if [ -f $(REPO_ROOT)/creds.json ]; then \
		cp $(REPO_ROOT)/creds.json $(SDK_ROOT)/smoke-test/; \
	fi
	@cd $(SDK_ROOT)/smoke-test && $(SDK_ROOT)/gradlew runGrpc --args="--connectors $(CONNECTORS)" 2>&1

test-grpc: install generate-all ## Build grpc-ffi lib + grpc-server → start server → run Kotlin gRPC smoke test → stop server
	@echo "Building gRPC FFI library ($(GRPC_PROFILE))..."
	@cd $(REPO_ROOT) && cargo build -p hyperswitch-grpc-ffi --profile $(GRPC_PROFILE) 2>&1
	@echo "Building gRPC server ($(GRPC_PROFILE))..."
	@cd $(REPO_ROOT) && cargo build -p grpc-server --profile $(GRPC_PROFILE) 2>&1
	@echo "Starting gRPC server..."
	@pkill -KILL -f grpc-server 2>/dev/null || true
	@sleep 1
	@cd $(REPO_ROOT) && $(GRPC_SERVER_BIN) > /tmp/grpc-server-kt.log 2>&1 &
	@sleep 2
	@$(MAKE) test-grpc-no-server; \
	EXIT=$$?; \
	pkill -f grpc-server 2>/dev/null || true; \
	exit $$EXIT

# ---------------------------------------------------------------------------
# dist
# Builds the distribution JAR for all supported platforms.
# Copies pre-built native binaries from each platform's target directory into
# native/, generates proto stubs, then packs the JAR. UniFFI Kotlin bindings
# are expected to be already present (generated by a prior generate-bindings
# step or downloaded from a CI artifact).
# ---------------------------------------------------------------------------
dist:
	@echo "Building Java SDK distribution JAR..."
	@mkdir -p $(NATIVE_DIR)
	@cp -f $(REPO_ROOT)/target/x86_64-unknown-linux-gnu/release/libconnector_service_ffi.so \
		$(NATIVE_DIR)/ 2>/dev/null || echo "  Note: Linux x86_64 binary not found (skipping)"
	@cp -f $(REPO_ROOT)/target/aarch64-apple-darwin/release/libconnector_service_ffi.dylib \
		$(NATIVE_DIR)/ 2>/dev/null || echo "  Note: macOS aarch64 binary not found (skipping)"
	@$(MAKE) -C $(SDK_ROOT) pack-archive
	@echo "Distribution JAR created in $(PACKAGE_DIR)/"

# ---------------------------------------------------------------------------
# publish
# Publishes to Maven Central via Central Portal (requires proper credentials and signing).
# For local testing, use 'install' which publishes to ~/.m2.
# Requires CENTRAL_TOKEN_USERNAME and CENTRAL_TOKEN_PASSWORD to be set for authentication.
# Also requires GPG_SIGNING_KEY and GPG_SIGNING_KEY_PASSWORD for artifact signing.
# Uses Central Portal Publisher Gradle plugin (central.sonatype.com).
# ---------------------------------------------------------------------------
publish:
	@if [ -z "$(CENTRAL_TOKEN_USERNAME)" ] || [ -z "$(CENTRAL_TOKEN_PASSWORD)" ]; then \
		echo "Error: CENTRAL_TOKEN_USERNAME and/or CENTRAL_TOKEN_PASSWORD not set"; \
		echo "Set them with:"; \
		echo "  export CENTRAL_TOKEN_USERNAME=your_token_username"; \
		echo "  export CENTRAL_TOKEN_PASSWORD=your_token_password"; \
		exit 1; \
	fi
	@if [ -z "$${GPG_SIGNING_KEY}" ] || [ -z "$(GPG_SIGNING_KEY_PASSWORD)" ]; then \
		echo "Error: GPG_SIGNING_KEY and/or GPG_SIGNING_KEY_PASSWORD not set"; \
		exit 1; \
	fi
	@echo "Publishing io.hyperswitch:prism to Maven Central via Central Portal..."
	@$(SDK_ROOT)/gradlew publishToCentral
	@echo "Published io.hyperswitch:prism to Maven Central."

# ---------------------------------------------------------------------------
# Pre-publish checks
# ---------------------------------------------------------------------------

MAVEN_GROUP := io.hyperswitch
MAVEN_ARTIFACT := prism

# publish-check-auth
# Verifies that CENTRAL_TOKEN_USERNAME and CENTRAL_TOKEN_PASSWORD environment variables are set.
publish-check-auth:
	@if [ -z "$(CENTRAL_TOKEN_USERNAME)" ]; then \
		echo "Error: CENTRAL_TOKEN_USERNAME is not set"; \
		exit 1; \
	fi
	@if [ -z "$(CENTRAL_TOKEN_PASSWORD)" ]; then \
		echo "Error: CENTRAL_TOKEN_PASSWORD is not set"; \
		exit 1; \
	fi
	@if [ -z "$${GPG_SIGNING_KEY}" ]; then \
		echo "Error: GPG_SIGNING_KEY is not set"; \
		exit 1; \
	fi
	@if [ -z "$(GPG_SIGNING_KEY_PASSWORD)" ]; then \
		echo "Error: GPG_SIGNING_KEY_PASSWORD is not set"; \
		exit 1; \
	fi
	@echo "Maven Central (Central Portal) authentication credentials are configured."

# publish-check-version
# Checks if $(PACKAGE_VERSION) already exists on Maven Central.
# Uses Maven Central REST API to check for existing artifact POM.
publish-check-version:
	@echo "Checking if $(MAVEN_GROUP):$(MAVEN_ARTIFACT):$(PACKAGE_VERSION) exists on Maven Central..."
	@POM_URL="https://repo1.maven.org/maven2/$(subst .,/,$(MAVEN_GROUP))/$(MAVEN_ARTIFACT)/$(PACKAGE_VERSION)/$(MAVEN_ARTIFACT)-$(PACKAGE_VERSION).pom"; \
	if curl -s -f -I "$$POM_URL" >/dev/null 2>&1; then \
		echo "Error: Version $(PACKAGE_VERSION) already exists on Maven Central"; \
		echo "Increment the version in build.gradle.kts before publishing."; \
		exit 1; \
	fi
	@echo "Version $(PACKAGE_VERSION) does not exist on Maven Central - OK to publish."

# publish-check-dry-run
# Simulates publish by creating bundle (requires signing key).
publish-check-dry-run:
	@echo "Creating deployment bundle (dry-run)..."
	@$(SDK_ROOT)/gradlew bundleArtifacts
	@echo "Dry-run validation passed."

# publish-check
# Runs all pre-publish checks in sequence.
publish-check: publish-check-auth publish-check-version publish-check-dry-run
	@echo "All Java pre-publish checks passed."

clean:
	@rm -rf $(GENERATED_KOTLIN) $(GENERATED_JAVA) $(NATIVE_DIR)
	@rm -f $(SDK_ROOT)src/main/kotlin/GeneratedFlows.kt
	@$(SDK_ROOT)/gradlew clean 2>/dev/null || true
