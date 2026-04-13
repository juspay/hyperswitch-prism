# Makefile

# Use nightly for rustfmt
NIGHTLY := +nightly

# CI mode? (set CI=true to enforce warnings-as-errors)
CI ?= false
ifeq ($(CI),true)
	CLIPPY_EXTRA := -- -D warnings
endif

# Connector test parameters (override on command line)
connector ?=
suite     ?=
scenario  ?=
interface ?= grpc

# gRPC server settings
# The test harness connects to localhost:50051 by default.
# Override with: make test-connector connector=stripe GRPC_PORT=9090
GRPC_PORT  ?= 50051
GRPC_HOST  ?= 0.0.0.0
# PID file used to track the background server process
GRPC_PID_FILE := .grpc-server.pid

.PHONY: all fmt check clippy test nextest ci check-specs help \
        proto-format proto-generate proto-build proto-lint proto-clean \
        generate certify-client-sanity field-probe docs docs-check all-connectors-doc \
        setup-connector-tests \
        start-grpc stop-grpc \
        test-prism test-ucs test-connector test-scenario cargo \
        validate-pre-push \
        ai \
        gen-tech-spec \
        new-connector \
        add-flow \
        add-payment-method \
        review-pr \
        test-grpc \
		test-ffi

# ── Standard build/lint targets ────────────────────────────────────────────────

## Run all checks: fmt → check → clippy → test
all: fmt check clippy test

## Run rustfmt on all crates
fmt:
	@echo "▶ rustfmt (nightly)…"
	cargo $(NIGHTLY) fmt --all

## Run cargo-hack check on each feature (no dev‑deps)
check:
	@echo "▶ cargo-hack check…"
	cargo hack check --each-feature --no-dev-deps

## Run cargo-hack clippy on each feature (no dev‑deps)
clippy:
	@echo "▶ cargo-hack clippy…"
	cargo hack clippy --each-feature --no-dev-deps $(CLIPPY_EXTRA)

## Run cargo-hack tests on each feature
test:
	@echo "▶ cargo-hack test…"
	cargo hack test --each-feature

## Run tests with nextest (faster test runner)
nextest:
	@echo "▶ cargo nextest…"
	cargo nextest run --config-file .nextest.toml

## Check connector specs coverage (flow→suite parity + testable suite report)
check-specs:
	@echo "▶ check_connector_specs…"
	cargo run --bin check_connector_specs

## CI-friendly invocation:
##    make ci
## or CI=true make all
ci:
	@echo "⚙️  Running in CI mode (warnings = errors)…"
	@$(MAKE) CI=true all

# ── Connector integration test setup ──────────────────────────────────────────

## One-time (idempotent) setup for connector integration tests.
## Installs npm deps, Playwright browsers, and deploys GPay pages to Netlify.
## Run this once before running any connector tests.
setup-connector-tests:
	@echo "▶ Setting up connector integration tests…"
	@bash scripts/setup-connector-tests.sh

# ── gRPC server lifecycle ──────────────────────────────────────────────────────

## Build and start the gRPC server in the background on GRPC_PORT (default 50051).
## The server PID is written to $(GRPC_PID_FILE) so stop-grpc can kill it.
## You rarely need to call this directly — test-prism / test-connector /
## test-scenario all manage the server lifecycle automatically.
start-grpc:
	@echo "▶ Building grpc-server…"
	@cargo build -p grpc-server --release 2>&1
	@echo "▶ Starting grpc-server on $(GRPC_HOST):$(GRPC_PORT)…"
	@CS__SERVER__HOST=$(GRPC_HOST) CS__SERVER__PORT=$(GRPC_PORT) CS__COMMON__ENVIRONMENT=development ./target/release/grpc-server & echo $$! > $(GRPC_PID_FILE)
	@echo "[grpc] waiting for server to be ready on port $(GRPC_PORT)…"
	@for i in $$(seq 1 40); do \
	  if nc -z 127.0.0.1 $(GRPC_PORT) 2>/dev/null; then \
	    echo "[grpc] server is ready (PID $$(cat $(GRPC_PID_FILE)))"; \
	    exit 0; \
	  fi; \
	  sleep 0.5; \
	done; \
	echo "[grpc] ERROR: server did not start within 20 s"; \
	cat $(GRPC_PID_FILE) | xargs kill 2>/dev/null || true; \
	rm -f $(GRPC_PID_FILE); \
	exit 1

## Stop the background gRPC server that was started by start-grpc.
stop-grpc:
	@if [ -f $(GRPC_PID_FILE) ]; then \
	  PID=$$(cat $(GRPC_PID_FILE)); \
	  echo "[grpc] stopping server (PID $$PID)…"; \
	  kill $$PID 2>/dev/null || true; \
	  rm -f $(GRPC_PID_FILE); \
	  echo "[grpc] stopped"; \
	else \
	  echo "[grpc] no PID file found — server may not be running"; \
	fi

# ── Connector integration test runners ────────────────────────────────────────

# Internal macro: load .env.connector-tests and export GPAY_HOSTED_URL if present.
define load_env
	@[ -f .env.connector-tests ] && export $$(grep -v '^#' .env.connector-tests | xargs) || true
endef

## UCS connector test runner. After running setup, use `test-prism` directly.
## Handles first-run setup, gRPC server lifecycle, and all flags automatically.
##
##   make test-prism
##
## For full flag support use test-prism directly (installed by setup):
##   test-prism --help
test-prism:
	@./scripts/run-tests

## Backwards-compatible alias for test-prism.
test-ucs: test-prism

## Run all integration tests for a specific connector (non-interactive).
## Automatically starts the gRPC server before the run and stops it after.
##
##   make test-connector connector=stripe
##   make test-connector connector=cybersource interface=sdk
test-connector:
	@if [ -z "$(connector)" ]; then \
	  echo "Error: connector is required."; \
	  echo "Usage: make test-connector connector=stripe"; \
	  exit 1; \
	fi
	@echo "▶ Running all suites for connector '$(connector)' (interface=$(interface))…"
	@if [ "$(interface)" = "grpc" ]; then $(MAKE) start-grpc; fi
	@EXIT_CODE=0; \
	 [ -f .env.connector-tests ] && export $$(grep -v '^#' .env.connector-tests | xargs) 2>/dev/null || true; \
	 cargo run -p integration-tests --bin test_ucs -- \
	   --connector $(connector) \
	   --endpoint localhost:50051 \
	   --interface $(interface) || EXIT_CODE=$$?; \
	 [ "$(interface)" = "grpc" ] && $(MAKE) stop-grpc || true; \
	 exit $$EXIT_CODE

## Run a specific scenario (non-interactive).
## Automatically starts the gRPC server before the run and stops it after.
##
##   make test-scenario connector=stripe suite=authorize scenario=no3ds_auto_capture_credit_card
##   make test-scenario connector=stripe suite=authorize scenario=no3ds_auto_capture_google_pay_encrypted
test-scenario:
	@if [ -z "$(connector)" ] || [ -z "$(suite)" ] || [ -z "$(scenario)" ]; then \
	  echo "Error: connector, suite, and scenario are all required."; \
	  echo "Usage: make test-scenario connector=stripe suite=authorize scenario=no3ds_auto_capture_credit_card"; \
	  exit 1; \
	fi
	@echo "▶ Running $(connector)/$(suite)/$(scenario) (interface=$(interface))…"
	@if [ "$(interface)" = "grpc" ]; then $(MAKE) start-grpc; fi
	@EXIT_CODE=0; \
	 [ -f .env.connector-tests ] && export $$(grep -v '^#' .env.connector-tests | xargs) 2>/dev/null || true; \
	 cargo run -p integration-tests --bin test_ucs -- \
	   --connector $(connector) \
	   --suite $(suite) \
	   --scenario $(scenario) \
	   --endpoint localhost:50051 \
	   --interface $(interface) || EXIT_CODE=$$?; \
	 [ "$(interface)" = "grpc" ] && $(MAKE) stop-grpc || true; \
	 exit $$EXIT_CODE

# ── Cargo with environment ─────────────────────────────────────────────────────

## Run cargo commands with environment variables auto-loaded from .env.connector-tests
## Usage: make cargo ARGS="run -p integration-tests --bin test_ucs -- --connector stripe"
## Usage: make cargo ARGS="test"
## Usage: make cargo ARGS="build"
cargo:
	@if [ -f .env.connector-tests ]; then \
	  export $$(grep -v '^#' .env.connector-tests | xargs) 2>/dev/null; \
	fi; \
	cargo $(ARGS)

# ── Proto targets ──────────────────────────────────────────────────────────────

# Format proto files
proto-format:
	@echo "Formatting proto files..."
	buf format -w

# Generate code from proto files (e.g., OpenAPI specs)
proto-generate:
	@echo "Generating code from proto files..."
	buf generate

# Validate proto files
# This can catch issues before generating code or compiling
proto-build:
	@echo "Building/validating proto files..."
	buf build

# Lint proto files
proto-lint:
	@echo "Linting proto files..."
	buf lint

# Clean generated files
proto-clean:
	@echo "Cleaning generated files..."
	rm -rf gen

# ── SDK / docs targets ─────────────────────────────────────────────────────────

## Generate SDK flow bindings from services.proto ∩ bindings/uniffi.rs
generate:
	@echo "▶ Generating SDK flows from services.proto…"
	@$(MAKE) -C sdk generate

## SDK Certification: Run HTTP client sanity suite across all supported languages
certify-client-sanity:
	@echo "Cleaning previous client sanity artifacts..."
	@rm -rf sdk/tests/client_sanity/artifacts || true
	@mkdir -p sdk/tests/client_sanity/artifacts
	@echo "Starting Client Sanity Certification..."
	@pkill -f "[/]echo_server\\.js" || true
	@pkill -f "[/]simple_proxy\\.js" || true
	@node sdk/tests/client_sanity/simple_proxy.js > /dev/null 2>&1 & sleep 2
	@echo "Generating golden captures from manifest..."
	@node sdk/tests/client_sanity/generate_golden.js
	@echo "[CERTIFICATION]: Running client sanity suite..."
	@node sdk/tests/client_sanity/run_client_certification.js rust python node kotlin
	@pkill -f "[/]echo_server\\.js"; pkill -f "[/]simple_proxy\\.js" || true

CONNECTORS   ?= stripe
GRPC_PROFILE ?= release-fast

## Run gRPC smoke tests for all SDKs (Rust + JS + Python) with a combined pass/fail summary
test-grpc:
	@$(MAKE) -C sdk test-grpc CONNECTORS=$(CONNECTORS) GRPC_PROFILE=$(GRPC_PROFILE)

## Run FFI smoke tests for all SDKs (Rust + JS + Python + Kotlin) with a combined pass/fail summary
test-ffi:
	@$(MAKE) -C sdk test CONNECTORS=$(CONNECTORS)

## Run FFI smoke tests in MOCK mode for all SDKs (no real HTTP, verifies req_transformer only)
## Runs all SDKs in parallel and prints a combined pass/fail table.
## Set VERBOSE=1 or V=1 to see detailed error messages
test-ffi-mock: generate-harnesses
	@python3 scripts/run_smoke_tests_parallel.py --connectors $(CONNECTORS) --mock $(if $(filter 1,$(VERBOSE) $(V)),--verbose)

## Generate harnesses for all connectors specified in CONNECTORS
## Used by test-ffi-mock to ensure harnesses are up to date
generate-harnesses:
	@echo "Generating harnesses for: $(CONNECTORS)"
	@for connector in $(shell echo $(CONNECTORS) | tr ',' ' '); do \
		python3 scripts/generators/code/generate_harnesses.py --connector $$connector; \
	done

## Run field-probe to generate connector flow data
field-probe:
	@echo "▶ Running field-probe to generate connector flow data…"
	-cargo run -p field-probe

## Run comprehensive pre-push validation (format, check, clippy, generate, docs)
validate-pre-push:
	@echo "▶ Running pre-push validation..."
	@./scripts/validation/pre-push.sh

## Run pre-push validation with tests (slower but more thorough)
validate-pre-push-full:
	@echo "▶ Running pre-push validation with tests..."
	@./scripts/validation/pre-push.sh --with-tests

## Fix formatting and run pre-push validation
validate-pre-push-fix:
	@echo "▶ Running pre-push validation with auto-fix..."
	@./scripts/validation/pre-push.sh --fix

## Generate connector docs (default: stripe only; use CONNECTORS=all for all connectors)
## Skips field-probe if data/field_probe already exists; use CONNECTORS=all to re-probe all connectors.
docs:
	@echo "▶ Generating connector docs…"
	@if [ "$(CONNECTORS)" = "all" ]; then \
		$(MAKE) field-probe; \
		python3 scripts/generators/docs/generate.py --all --probe-path data/field_probe; \
	else \
		if [ ! -d data/field_probe ] || [ -z "$$(ls -A data/field_probe 2>/dev/null)" ]; then \
			$(MAKE) field-probe; \
		fi; \
		python3 scripts/generators/docs/generate.py stripe --probe-path data/field_probe; \
	fi
	@echo "▶ Formatting Rust code (nightly)…"
	@cargo $(NIGHTLY) fmt --all

## Generate the all-connectors coverage document
all-connectors-doc: field-probe
	@echo "▶ Generating all-connectors coverage doc…"
	python3 scripts/generators/docs/generate.py --all-connectors-doc --probe-path data/field_probe

## Report annotation coverage for connector docs
docs-check:
	@echo "▶ Checking connector annotation coverage…"
	python3 scripts/generators/docs/generate.py --check

# ── Help ───────────────────────────────────────────────────────────────────────
# Shared shell function: detect AI editors, prompt if multiple, set up symlink
define AI_AGENT
	editors=""; \
	command -v claude >/dev/null 2>&1 && editors="$$editors claude"; \
	command -v opencode >/dev/null 2>&1 && editors="$$editors opencode"; \
	command -v cursor >/dev/null 2>&1 && editors="$$editors cursor"; \
	command -v windsurf >/dev/null 2>&1 && editors="$$editors windsurf"; \
	command -v codex >/dev/null 2>&1 && editors="$$editors codex"; \
	editors=$$(echo $$editors | xargs); \
	if [ -z "$$editors" ]; then \
		echo "Error: No AI editors found. Install one of: claude, opencode, cursor, windsurf, codex"; \
		exit 1; \
	fi; \
	count=$$(echo $$editors | wc -w | xargs); \
	if [ "$$count" -eq 1 ]; then \
		choice=$$editors; \
	else \
		echo "Multiple AI editors detected:"; \
		i=1; for e in $$editors; do echo "  $$i) $$e"; i=$$((i+1)); done; \
		printf "Choose editor [1-$$count]: "; read sel; \
		choice=$$(echo $$editors | cut -d' ' -f$$sel); \
	fi; \
	case $$choice in \
		claude)   mkdir -p .claude && ln -sfn ../.skills .claude/skills ;; \
		opencode) mkdir -p .opencode && ln -sfn ../.skills .opencode/skills ;; \
		cursor)   mkdir -p .cursor && ln -sfn ../.skills .cursor/rules ;; \
		windsurf) mkdir -p .windsurf && ln -sfn ../.skills .windsurf/rules ;; \
		codex)    mkdir -p .agents && ln -sfn ../.skills .agents/skills ;; \
	esac; \
	echo "Skills linked for $$choice"
endef

# Launch editor with a specific skill
# Usage: $(call LAUNCH_SKILL,skill-name)
# - claude: /skill-name slash command as positional arg
# - opencode: --prompt flag (skills auto-invoke, prompt hints the agent)
# - codex: plain prompt that loads the skill and asks for required inputs
# - cursor/windsurf: open project (skills auto-load as rules)
define LAUNCH_SKILL
	case $$choice in \
		claude)   exec claude "/$(1)" ;; \
		opencode) exec opencode --prompt "Use the $(1) skill" ;; \
		codex)    exec codex "Use the $(1) skill. Ask the user for any required inputs before starting." ;; \
		cursor)   echo "Skill '$(1)' available as a rule in Cursor"; exec cursor . ;; \
		windsurf) echo "Skill '$(1)' available as a rule in Windsurf"; exec windsurf . ;; \
	esac
endef

## Launch AI editor with skills
ai:
	@$(AI_AGENT); \
	case $$choice in \
		claude|opencode|codex) exec $$choice ;; \
		cursor|windsurf) exec $$choice . ;; \
	esac

## Generate a technical specification for a connector
gen-tech-spec:
	@$(AI_AGENT); \
	$(call LAUNCH_SKILL,generate-tech-spec)

## Implement a new connector from scratch
new-connector:
	@$(AI_AGENT); \
	$(call LAUNCH_SKILL,new-connector)

## Add payment flow(s) to an existing connector
add-flow:
	@$(AI_AGENT); \
	$(call LAUNCH_SKILL,add-connector-flow)

## Add payment method support to an existing connector
add-payment-method:
	@$(AI_AGENT); \
	$(call LAUNCH_SKILL,add-payment-method)

## Review a PR using the pr-reviewer skill
review-pr:
	@$(AI_AGENT); \
	$(call LAUNCH_SKILL,pr-reviewer)

## Show this help
help:
	@echo "Usage: make [TARGET] [VARIABLE=value ...]"
	@echo
	@echo "Main Targets:"
	@echo "  all      Run fmt, check, clippy, test"
	@echo "  fmt      Format all crates with rustfmt (nightly)"
	@echo "  check    Run cargo-hack check (no dev-deps)"
	@echo "  clippy   Run cargo-hack clippy (no dev-deps)"
	@echo "  test     Run cargo-hack test"
	@echo "  nextest      Run tests with nextest (faster test runner)"
	@echo "  check-specs  Check connector specs coverage (flow→suite parity)"
	@echo "  ci       Same as 'all' but with CI=true (treat warnings as errors)"
	@echo
	@echo "Connector Integration Test Targets:"
	@echo ""
	@echo "  setup-connector-tests"
	@echo "    One-time setup: npm install, Playwright browsers, Netlify deploy."
	@echo "    Safe to re-run (idempotent). Do this once before running tests."
	@echo ""
	@echo "  test-prism"
	@echo "    Run all connector tests. After setup, you can also just type: test-prism"
	@echo "    Starts the gRPC server automatically."
	@echo "    Example: make test-prism"
	@echo "    Alias:   make test-ucs (backwards compat)"
	@echo ""
	@echo "  test-connector connector=<name> [interface=grpc|sdk]"
	@echo "    Run all suites for one connector, non-interactively."
	@echo "    Starts + stops the gRPC server automatically."
	@echo "    Example: make test-connector connector=stripe"
	@echo "             make test-connector connector=cybersource interface=sdk"
	@echo ""
	@echo "  test-scenario connector=<name> suite=<suite> scenario=<scenario> [interface=grpc|sdk]"
	@echo "    Run a single scenario, non-interactively."
	@echo "    Starts + stops the gRPC server automatically."
	@echo "    Example: make test-scenario connector=stripe suite=authorize scenario=no3ds_auto_capture_credit_card"
	@echo "             make test-scenario connector=stripe suite=authorize scenario=no3ds_auto_capture_google_pay_encrypted"
	@echo ""
	@echo "  cargo ARGS=\"<cargo-args>\""
	@echo "    Run cargo commands with .env.connector-tests auto-loaded (GPAY_HOSTED_URL, etc)."
	@echo "    Use this when running cargo directly instead of via test-prism."
	@echo "    Example: make cargo ARGS=\"run -p integration-tests --bin test_ucs -- --connector stripe\""
	@echo "             make cargo ARGS=\"test\""
	@echo "             make cargo ARGS=\"build --release\""
	@echo ""
	@echo "  start-grpc [GRPC_PORT=50051]"
	@echo "    Build and start the gRPC server in the background."
	@echo ""
	@echo "  stop-grpc"
	@echo "    Stop the background gRPC server."
	@echo ""
	@echo "Credential resolution order (for connector tests):"
	@echo "  1. CONNECTOR_AUTH_FILE_PATH env var"
	@echo "  2. UCS_CREDS_PATH env var"
	@echo "  3. creds.json (repo default)"
	@echo ""
	@echo "Google Pay tests require GPAY_HOSTED_URL to be set."
	@echo "Run 'make setup-connector-tests' to configure it automatically via Netlify."
	@echo ""
	@echo "Validation Targets:"
	@echo "  validate-pre-push      Run comprehensive pre-push validation (format, check, clippy, generate, docs)"
	@echo "  validate-pre-push-full Run pre-push validation with tests (slower)"
	@echo "  validate-pre-push-fix  Run pre-push validation with auto-fix"
	@echo
	@echo "Proto Targets:"
	@echo "  proto-format     Format proto files"
	@echo "  proto-generate   Generate code from proto files"
	@echo "  proto-build      Build/validate proto files"
	@echo "  proto-lint       Lint proto files"
	@echo "  proto-clean      Clean generated proto files"
	@echo ""
	@echo "SDK Codegen Targets:"
	@echo "  generate         Generate SDK flow bindings (Python, JS, Kotlin) from services.proto"
	@echo ""
	@echo "Docs Targets:"
	@echo "  docs               Regenerate connector docs (default: stripe; CONNECTORS=all for all)"
	@echo "  all-connectors-doc Generate the all-connectors coverage document"
	@echo "  docs-check         Report which connectors are missing annotation files"
	@echo ""
	@echo "Certification Targets:"
	@echo "  certify-client-sanity  Run cross-language transport parity certification"
	@echo ""
	@echo "Other:"
	@echo
	@echo "AI Targets:"
	@echo "  ai                 Detect AI editor, set up skills symlink, and launch"
	@echo "  gen-tech-spec      Generate a technical specification for a connector"
	@echo "  new-connector      Implement a new connector from scratch"
	@echo "  add-flow           Add payment flow(s) to an existing connector"
	@echo "  add-payment-method Add payment method support to an existing connector"
	@echo "  review-pr          Review a PR using the pr-reviewer skill"
	@echo
	@echo "Other Targets:"
	@echo "  test-grpc              Run gRPC smoke tests for all SDKs (Rust + JS + Python)"
	@echo "    CONNECTORS=stripe    Connector(s) to test (comma-separated)"
	@echo "    GRPC_PROFILE=...     Cargo profile (default: release-fast)"
	@echo "  test-ucs               Run interactive UCS connector tests"
	@echo "  help                   Show this help message"
