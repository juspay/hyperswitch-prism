# Makefile

# Use nightly for rustfmt
NIGHTLY := +nightly

# CI mode? (set CI=true to enforce warnings-as-errors)
CI ?= false
ifeq ($(CI),true)
	CLIPPY_EXTRA := -- -D warnings
endif

.PHONY: all fmt check clippy test nextest ci help proto-format proto-generate proto-build proto-lint proto-clean generate certify-client-sanity field-probe docs docs-check test-ucs

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

## CI-friendly invocation:
##    make ci
## or CI=true make all
ci:
	@echo "⚙️  Running in CI mode (warnings = errors)…"
	@$(MAKE) CI=true all


## Generate SDK flow bindings from services.proto ∩ bindings/uniffi.rs
generate:
	@echo "▶ Generating SDK flows from services.proto…"
	@$(MAKE) -C sdk generate

## Run interactive UCS connector test runner
test-ucs:
	@echo "▶ Starting interactive UCS connector tests…"
	cargo run -p ucs-connector-tests --bin test_ucs

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

## Run field-probe to generate connector flow data
field-probe:
	@echo "▶ Running field-probe to generate connector flow data…"
	cargo run -p field-probe

## Generate connector docs from source code (all connectors)
docs: field-probe
	@echo "▶ Generating connector docs…"
	python3 scripts/generate-connector-docs.py --all --probe data/field_probe

## Report annotation coverage for connector docs
docs-check:
	@echo "▶ Checking connector annotation coverage…"
	python3 scripts/generate-connector-docs.py --check

## Show this help
help:
	@echo "Usage: make [TARGET]"
	@echo
	@echo "Main Targets:"
	@echo "  all      Run fmt, check, clippy, test"
	@echo "  fmt      Format all crates with rustfmt (nightly)"
	@echo "  check    Run cargo-hack check (no dev-deps)"
	@echo "  clippy   Run cargo-hack clippy (no dev-deps)"
	@echo "  test     Run cargo-hack test"
	@echo "  nextest  Run tests with nextest (faster test runner)"
	@echo "  ci       Same as '''all''' but with CI=true (treat warnings as errors)"
	@echo
	@echo "Proto Targets:"
	@echo "  proto-format     Format proto files"
	@echo "  proto-generate   Generate code from proto files"
	@echo "  proto-build      Build/validate proto files"
	@echo "  proto-lint       Lint proto files"
	@echo "  proto-clean      Clean generated proto files"
	@echo
	@echo "SDK Codegen Targets:"
	@echo "  generate         Generate SDK flow bindings (Python, JS, Kotlin) from services.proto"
	@echo
	@echo "Docs Targets:"
	@echo "  docs         Regenerate all connector docs from source"
	@echo "  docs-check   Report which connectors are missing annotation files"
	@echo "Certification Targets:"
	@echo "  certify-client-sanity  Run cross-language transport parity certification"
	@echo
	@echo "Other Targets:"
	@echo "  test-ucs Run interactive UCS connector tests"
	@echo "  help     Show this help message"
