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

# Cargo uses 'debug' as the output directory name for the built-in 'dev' profile.
# All other profiles (release, release-fast, custom) use their name as the directory.
ifeq ($(PROFILE),dev)
  _PROFILE_DIR := debug
else
  _PROFILE_DIR := $(PROFILE)
endif

# Pre-built FFI library for the current platform (output of build-ffi-lib).
LIBRARY          := $(REPO_ROOT)/target/$(PLATFORM)/$(_PROFILE_DIR)/libconnector_service_ffi.$(LIB_EXT)
# Pre-built gRPC FFI library (output of build-grpc-ffi-lib).
# Built with --target $(PLATFORM) so artifacts share the same build cache as all other crates.
GRPC_FFI_LIBRARY := $(REPO_ROOT)/target/$(PLATFORM)/$(_PROFILE_DIR)/libhyperswitch_grpc_ffi.$(LIB_EXT)

# UniFFI bindgen binary path (used by Python/Java for code generation)
BINDGEN := $(REPO_ROOT)/target/$(PLATFORM)/$(_PROFILE_DIR)/uniffi-bindgen

# ---------------------------------------------------------------------------
# build-ffi-lib
# Builds the Rust FFI shared library for the auto-detected platform.
# Output: target/<PLATFORM>/$(PROFILE)/libconnector_service_ffi.<ext>
# Using --target consistently means local builds and CI builds share the same
# directory layout — no special-case LIBRARY= or TARGET_TRIPLE= variables needed.
# This target is NOT .PHONY - it skips the build if the library already exists
# to save time when running multiple SDK tests.
# ---------------------------------------------------------------------------
build-ffi-lib:
	@if [ "$(FFI_SKIP_BUILD)" = "1" ]; then \
		if [ -f "$(LIBRARY)" ]; then \
			echo "FFI library found: $(LIBRARY)"; \
		else \
			echo "ERROR: FFI_SKIP_BUILD=1 but library not found: $(LIBRARY)"; \
			exit 1; \
		fi; \
	elif [ -f "$(LIBRARY)" ]; then \
		echo "FFI library already exists: $(LIBRARY)"; \
	else \
		echo "Building FFI shared library for $(PLATFORM) ($(PROFILE))..."; \
		cd $(REPO_ROOT) && cargo build -p ffi --no-default-features --features ffi/uniffi \
			--profile $(PROFILE) --target $(PLATFORM); \
		echo "Build complete: $(LIBRARY)"; \
	fi

# ---------------------------------------------------------------------------
# build-grpc-ffi-lib
# Builds the gRPC FFI library for SDKs that use gRPC functionality.
# Output: target/$(PLATFORM)/$(PROFILE)/libhyperswitch_grpc_ffi.<ext>
# Uses --target $(PLATFORM) so compiled dep artifacts are shared with all other
# crates (ffi, grpc-server, uniffi-bindgen) that also use --target.
# ---------------------------------------------------------------------------
build-grpc-ffi-lib:
	@if [ -f "$(GRPC_FFI_LIBRARY)" ]; then \
		echo "gRPC FFI library already exists: $(GRPC_FFI_LIBRARY)"; \
	else \
		echo "Building gRPC FFI shared library for $(PLATFORM) ($(PROFILE))..."; \
		cd $(REPO_ROOT) && cargo build -p hyperswitch-grpc-ffi \
			--profile $(PROFILE) --target $(PLATFORM); \
		echo "Build complete: $(GRPC_FFI_LIBRARY)"; \
	fi

# ---------------------------------------------------------------------------
# check-cargo
# Verifies cargo command is installed before attempting builds.
# ---------------------------------------------------------------------------
.PHONY: check-cargo
check-cargo:
	@which cargo > /dev/null 2>&1 || (echo "Error: cargo is not installed" && exit 1)
