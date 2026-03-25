# Common SDK build configuration
# Include this in sdk/{python,javascript,java}/Makefile to share build logic
#
# Prerequisites: Define SDK_ROOT in your Makefile BEFORE including this file:
#   MAKEFILE_DIR := $(dir $(abspath $(lastword $(MAKEFILE_LIST))))
#   SDK_ROOT     := $(MAKEFILE_DIR)
#   include $(SDK_ROOT)../common.mk

# ---------------------------------------------------------------------------
# Paths (SDK_ROOT must be defined by the including Makefile)
# ---------------------------------------------------------------------------
ifndef SDK_ROOT
$(error SDK_ROOT must be defined before including common.mk)
endif

# For sub-makefiles (python/javascript/java), REPO_ROOT is grandparent
# For top-level sdk/Makefile, we define it separately before include
ifndef REPO_ROOT
REPO_ROOT    := $(shell cd $(SDK_ROOT)../.. && pwd)
endif

FFI_CRATE     := $(REPO_ROOT)/crates/ffi/ffi
GRPC_FFI_CRATE    := $(REPO_ROOT)/sdk/grpc-ffi
PROTO_DIR     := $(REPO_ROOT)/crates/types-traits/grpc-api-types/proto
ARTIFACTS_DIR := $(REPO_ROOT)/artifacts

# ---------------------------------------------------------------------------
# Platform detection
# Always building with --target <PLATFORM> keeps the artifact layout identical
# on CI runners and developer machines (target/<PLATFORM>/release/).
# ---------------------------------------------------------------------------
UNAME_S := $(shell uname -s)
UNAME_M := $(shell uname -m)
ifeq ($(UNAME_S), Darwin)
  ifeq ($(UNAME_M), arm64)
    PLATFORM := aarch64-apple-darwin
  else
    PLATFORM := x86_64-apple-darwin
  endif
  LIB_EXT := dylib
else
  ifeq ($(UNAME_M), aarch64)
    PLATFORM := aarch64-unknown-linux-gnu
  else
    PLATFORM := x86_64-unknown-linux-gnu
  endif
  LIB_EXT := so
endif

# Build profile (release or debug)
PROFILE ?= release-fast

# Pre-built FFI library for the current platform (output of build-ffi-lib).
LIBRARY          := $(REPO_ROOT)/target/$(PLATFORM)/$(PROFILE)/libconnector_service_ffi.$(LIB_EXT)
# Pre-built gRPC FFI library (output of build-grpc-ffi-lib).
GRPC_FFI_LIBRARY := $(REPO_ROOT)/target/$(PLATFORM)/$(PROFILE)/libhyperswitch_grpc_ffi.$(LIB_EXT)

# UniFFI bindgen binary path (used by Python/Java for code generation)
BINDGEN := $(REPO_ROOT)/target/$(PLATFORM)/$(PROFILE)/uniffi-bindgen

# ---------------------------------------------------------------------------
# build-ffi-lib
# Builds the Rust FFI shared library for the auto-detected platform.
# Output: target/<PLATFORM>/$(PROFILE)/libconnector_service_ffi.<ext>
# Using --target consistently means local builds and CI builds share the same
# directory layout — no special-case LIBRARY= or TARGET_TRIPLE= variables needed.
# ---------------------------------------------------------------------------
.PHONY: build-ffi-lib
build-ffi-lib:
	@echo "Building FFI shared library for $(PLATFORM) ($(PROFILE))..."
	@cd $(FFI_CRATE) && cargo build --no-default-features --features uniffi \
		--profile $(PROFILE) --target $(PLATFORM)
	@echo "Build complete: $(LIBRARY)"

# ---------------------------------------------------------------------------
# check-cargo
# Verifies cargo command is installed before attempting builds.
# ---------------------------------------------------------------------------
# ---------------------------------------------------------------------------
# build-grpc-ffi-lib
# Builds the gRPC Rust FFI shared library for the auto-detected platform.
# Output: target/<PLATFORM>/$(PROFILE)/libhyperswitch_grpc_ffi.<ext>
# ---------------------------------------------------------------------------
.PHONY: build-grpc-ffi-lib
build-grpc-ffi-lib:
	@echo "Building gRPC FFI shared library for $(PLATFORM) ($(PROFILE))..."
	@cd $(GRPC_FFI_CRATE) && cargo build --profile $(PROFILE) --target $(PLATFORM)
	@echo "Build complete: $(GRPC_FFI_LIBRARY)"

.PHONY: check-cargo
check-cargo:
	@which cargo > /dev/null 2>&1 || (echo "Error: cargo is not installed" && exit 1)
