#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "$0")/../.." && pwd)"
scratch="$(mktemp -d)"
trap 'rm -rf "$scratch"' EXIT
mkdir -p "$scratch/src" "$scratch/domain-config"
cp "$root/scripts/validation/fixtures/money_boundary.rs" "$scratch/src/lib.rs"
cp "$root/Cargo.lock" "$scratch/Cargo.lock"

# This temporary crate uses the real types and the real L3 Clippy configuration.
cat > "$scratch/Cargo.toml" <<EOF
[package]
name = "money-boundary-probes"
version = "0.0.0"
edition = "2021"
publish = false

[workspace]

[dependencies]
common_utils = { package = "ucs_common_utils", path = "$root/crates/common/common_utils", features = ["superposition"] }
common_enums = { package = "ucs_common_enums", path = "$root/crates/common/common_enums" }
domain_types = { path = "$root/crates/types-traits/domain_types" }
grpc-api-types = { path = "$root/crates/types-traits/grpc-api-types" }

[features]
domain_boundary = []
connector_pass = []
boundary_expect = []
boundary_expect_scope = []
obsolete_boundary_expect = []
boundary_allow = []
boundary_in_tests = []
private_constructor = []
private_fields = []
boundary_direct = []
boundary_alias = []
boundary_pointer = []
boundary_type_alias = []
boundary_reexport = []
proto_trait = []
domain_mapper = []
domain_minor_unit = []
domain_minor_converter = []
inferred_domain_minor_unit = []
money_amount = []
EOF

export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$root/target}"

check() {
    local feature="$1" expected="$2" config="$root/crates/integrations/connector-integration"
    local target="--lib"
    if [[ "$feature" == domain_boundary ]]; then
        config="$scratch/domain-config"
    fi
    if [[ "$feature" == boundary_in_tests ]]; then
        target="--tests"
    fi

    local status=0
    CLIPPY_CONF_DIR="$config" cargo clippy --quiet --no-deps -j 1 "$target" \
        --manifest-path "$scratch/Cargo.toml" --features "$feature" \
        --message-format=json -- -D warnings > "$scratch/output.jsonl" 2> "$scratch/stderr" || status=$?

    if [[ "$expected" == pass && "$status" == 0 ]]; then
        printf 'PASS %s\n' "$feature"
        return
    fi
    if [[ "$expected" != pass && "$status" == 101 ]] && jq -es --arg expected "$expected" '
        [.[] | select(.reason == "compiler-message" and .message.level == "error") | .message.code.code] as $codes |
        ($codes | length > 0) and
        all($codes[]; . as $code | $code != null and ($expected | split(",") | index($code) != null))
    ' "$scratch/output.jsonl" > /dev/null; then
        local actual
        actual="$(jq -rs '[.[] | select(.reason == "compiler-message" and .message.level == "error") | .message.code.code] | unique | join(",")' "$scratch/output.jsonl")"
        printf 'PASS %s (rejected: %s)\n' "$feature" "$actual"
        return
    fi

    printf 'FAIL %s: expected %s, exit %s\n' "$feature" "$expected" "$status" >&2
    jq -r 'select(.reason == "compiler-message") | .message.rendered // empty' "$scratch/output.jsonl" >&2
    cat "$scratch/stderr" >&2
    exit 1
}

check domain_boundary pass
check connector_pass pass
check boundary_expect pass
check boundary_expect_scope clippy::disallowed_methods,clippy::disallowed_types
check obsolete_boundary_expect unfulfilled_lint_expectations
check boundary_allow E0453
check boundary_in_tests clippy::disallowed_methods,clippy::disallowed_types
check private_constructor E0624
check private_fields E0451
check boundary_direct clippy::disallowed_methods,clippy::disallowed_types
check boundary_alias clippy::disallowed_methods,clippy::disallowed_types
check boundary_pointer clippy::disallowed_methods,clippy::disallowed_types
check boundary_type_alias clippy::disallowed_methods,clippy::disallowed_types
check boundary_reexport clippy::disallowed_methods,clippy::disallowed_types
check proto_trait E0599
check domain_mapper E0603
check domain_minor_unit clippy::disallowed_types
check domain_minor_converter clippy::disallowed_types
check inferred_domain_minor_unit clippy::disallowed_methods
check money_amount clippy::disallowed_methods
