# Makefile

# Use nightly for rustfmt
NIGHTLY := +nightly

# CI mode? (set CI=true to enforce warnings-as-errors)
CI ?= false
ifeq ($(CI),true)
	CLIPPY_EXTRA := -- -D warnings
endif

.PHONY: all fmt check clippy test nextest ci help proto-format proto-generate proto-build proto-lint proto-clean generate certify-client-sanity field-probe docs docs-check test-ucs validate-pre-push ai gen-tech-spec new-connector add-flow add-payment-method review-pr test-grpc

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

CONNECTORS   ?= stripe
GRPC_PROFILE ?= release-fast

## Run gRPC smoke tests for all SDKs (Rust + JS + Python) with a combined pass/fail summary
test-grpc:
	@$(MAKE) -C sdk test-grpc CONNECTORS=$(CONNECTORS) GRPC_PROFILE=$(GRPC_PROFILE)

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

## Generate connector docs from source code (all connectors)
docs: field-probe
	@echo "▶ Generating connector docs…"
	python3 scripts/generators/docs/generate.py --all --probe-path data/field_probe

## Generate the all-connectors coverage document
all-connectors-doc: field-probe
	@echo "▶ Generating all-connectors coverage doc…"
	python3 scripts/generators/docs/generate.py --all-connectors-doc --probe-path data/field_probe

## Report annotation coverage for connector docs
docs-check:
	@echo "▶ Checking connector annotation coverage…"
	python3 scripts/generators/docs/generate.py --check

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
