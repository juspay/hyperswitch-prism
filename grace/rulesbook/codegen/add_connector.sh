#!/bin/bash

# =============================================================================
# Hyperswitch Connector Generator v2.0
# =============================================================================
# A robust, maintainable script for generating connector boilerplate code
#
# Usage: ./add_connector_v2.sh <connector_name> <base_url> [options]
#
# Features:
# - Modular design for easy maintenance
# - Comprehensive error handling and validation
# - Self-documenting configuration
# - Future-proof architecture
# =============================================================================

set -euo pipefail  # Strict error handling

# =============================================================================
# CONFIGURATION SECTION
# =============================================================================
# All configurable values are centralized here for easy maintenance

# Script metadata
readonly SCRIPT_VERSION="2.0.0"
readonly SCRIPT_NAME="Hyperswitch Connector Generator"

# Paths configuration
readonly SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
readonly ROOT_DIR="$(cd "$SCRIPT_DIR/../../.." && pwd)"
readonly TEMPLATE_DIR="$SCRIPT_DIR/template-generation"
readonly CRATES_TRAITS="$ROOT_DIR/crates/types-traits"
readonly CRATES_INTEGRATIONS="$ROOT_DIR/crates/integrations"
readonly CRATES_INTERNAL="$ROOT_DIR/crates/internal"
readonly CONFIG_DIR="$ROOT_DIR/config"

# File paths
readonly CONNECTOR_TYPES_FILE="$CRATES_TRAITS/interfaces/src/connector_types.rs"
readonly DOMAIN_TYPES_FILE="$CRATES_TRAITS/domain_types/src/connector_types.rs"
readonly DOMAIN_TYPES_TYPES_FILE="$CRATES_TRAITS/domain_types/src/types.rs"
readonly INTEGRATION_TYPES_FILE="$CRATES_INTEGRATIONS/connector-integration/src/types.rs"
readonly DEFAULT_IMPL_FILE="$CRATES_INTEGRATIONS/connector-integration/src/default_implementations.rs"
readonly CONNECTORS_MODULE_FILE="$CRATES_INTEGRATIONS/connector-integration/src/connectors.rs"
readonly PROTO_FILE="$CRATES_TRAITS/grpc-api-types/proto/payment.proto"
readonly ROUTER_DATA_FILE="$CRATES_TRAITS/domain_types/src/router_data.rs"
readonly CONFIG_FILE="$CONFIG_DIR/development.toml"
readonly SANDBOX_CONFIG_FILE="$CONFIG_DIR/sandbox.toml"
readonly PRODUCTION_CONFIG_FILE="$CONFIG_DIR/production.toml"
readonly SUPERPOSITION_CONFIG_FILE="$CONFIG_DIR/superposition.toml"
readonly FIELD_PROBE_FILE="$CRATES_INTERNAL/field-probe/src/auth.rs"
readonly CONNECTOR_SPECS_ROOT="$CRATES_INTERNAL/integration-tests/src/connector_specs"

# payment.proto path relative to the repo root. Used to read the UPSTREAM copy of
# the file via `git show <ref>:<path>` when allocating proto numbers.
readonly PROTO_FILE_RELPATH="crates/types-traits/grpc-api-types/proto/payment.proto"

# Git ref the proto number allocators are computed against.
#
# The local working tree is NOT authoritative. A branch cut days ago sees a stale
# payment.proto and hands out an enum ordinal / oneof field number that another
# in-flight connector has already taken. That has happened three times:
# GOTYME_SANLAM vs JPM, JPM vs PayNearMe, TRAVELHUB vs PayNearMe.
# Override with the PROTO_BASE_REF environment variable if your fork tracks a
# different upstream branch.
readonly PROTO_BASE_REF="${PROTO_BASE_REF:-origin/main}"

# Template files
readonly CONNECTOR_TEMPLATE="$TEMPLATE_DIR/connector.rs.template"
readonly TRANSFORMERS_TEMPLATE="$TEMPLATE_DIR/transformers.rs.template"

# =============================================================================
# DYNAMIC FLOW DETECTION
# =============================================================================
# This script automatically detects all available flows from connector_types.rs
# When new flows are added to the ConnectorServiceTrait, they will be automatically
# included in new connector templates without any manual configuration needed.

# Global array to store detected flows
AVAILABLE_FLOWS=()

# =============================================================================
# FLOW DETECTION FUNCTIONS
# =============================================================================

detect_flows_from_connector_service_trait() {
    log_step "Auto-detecting flows from ConnectorServiceTrait"

    local connector_types_file="$CONNECTOR_TYPES_FILE"
    if [[ ! -f "$connector_types_file" ]]; then
        fatal_error "Cannot find connector_types.rs at: $connector_types_file"
    fi

    # Extract all trait names from ConnectorServiceTrait definition
    # This looks for lines like "+ PaymentAuthorizeV2<T>" or "+ PaymentSyncV2"
    local detected_flows
    detected_flows=$(awk '
        /pub trait ConnectorServiceTrait/ { in_trait = 1 }
        in_trait { print }
        in_trait && /^[[:space:]]*\{/ { exit }
    ' "$connector_types_file" | \
                    grep -E "^[[:space:]]*\+[[:space:]]*[A-Z][A-Za-z0-9]*" | \
                    sed -E 's/^[[:space:]]*\+[[:space:]]*([A-Z][A-Za-z0-9]*).*/\1/' | \
                    grep -v "ConnectorCommon" | \
                    sort -u)

    if [[ -z "$detected_flows" ]]; then
        fatal_error "No flows detected from ConnectorServiceTrait"
    fi

    # Convert to array
    while IFS= read -r flow; do
        if [[ -n "$flow" ]]; then
            AVAILABLE_FLOWS+=("$flow")
        fi
    done <<< "$detected_flows"

    log_success "Detected ${#AVAILABLE_FLOWS[@]} flows from ConnectorServiceTrait"
    log_debug "Detected flows: ${AVAILABLE_FLOWS[*]}"
}

# Function to get basic description for any flow
get_flow_description() {
    case "$1" in
        *"Authorize"*) echo "Process payment authorization" ;;
        *"Sync"*) echo "Synchronize status" ;;
        *"Void"*) echo "Void/cancel operations" ;;
        *"Capture"*) echo "Capture authorized payments" ;;
        *"Refund"*) echo "Process refunds" ;;
        *"Mandate"*) echo "Setup recurring payment mandates" ;;
        *"Repeat"*) echo "Process recurring payments" ;;
        *"Order"*) echo "Create payment orders" ;;
        *"Token"*) echo "Handle tokenization" ;;
        *"Dispute"*) echo "Handle payment disputes" ;;
        *"Evidence"*) echo "Submit dispute evidence" ;;
        *"Webhook"*) echo "Handle incoming webhooks" ;;
        *"Validation"*) echo "Basic validation functionality" ;;
        *"Access"*) echo "Handle access tokens" ;;
        *"Session"*) echo "Handle session tokens" ;;
        *"Authenticate"*) echo "Handle authentication" ;;
        *) echo "Payment processing flow" ;;
    esac
}

# =============================================================================
# TRAIT-TO-FLOW MAPPING
# =============================================================================
# Maps a ConnectorServiceTrait sub-trait name (as detected from connector_types.rs)
# to the corresponding flow identifier used by
# crate::connectors::macros::macro_connector_flow_status_impls!.
#
# Returns empty string for a trait the generated connector must NOT list, and the
# caller skips it. Two separate reasons for that:
#   1. It is not a flow at all and has no `expand_flow_status_impl!` arm:
#      ConnectorCommon, ValidationTrait, IncomingWebhook, VerifyRedirectResponse,
#      VerifyWebhookSourceV2, RefreshPaymentMethodV2. These get plain `impl`
#      blocks (emitted above) or a blanket default impl.
#   2. It HAS an arm but already has a blanket default impl in
#      crates/integrations/connector-integration/src/default_implementations.rs,
#      so listing it here would produce a conflicting second impl:
#      RechargeV2, CreatePaymentMethodV2, GetPaymentMethodV2,
#      PaymentMethodEligibilityV2.
#
# Every OTHER arm of `expand_flow_status_impl!` in
# crates/integrations/connector-integration/src/connectors/macros.rs must appear
# here - a supertrait of ConnectorServiceTrait with neither a mapping nor a
# default impl makes the scaffolded connector fail to compile.
get_flow_name_for_trait() {
    case "$1" in
        PaymentAuthorizeV2)               echo "Authorize" ;;
        PaymentSyncV2)                    echo "PSync" ;;
        PaymentVoidV2)                    echo "Void" ;;
        PaymentCapture)                   echo "Capture" ;;
        PaymentVoidPostCaptureV2)         echo "VoidPC" ;;
        PaymentIncrementalAuthorization)  echo "IncrementalAuthorization" ;;
        PaymentOrderCreate)               echo "CreateOrder" ;;
        CreateConnectorCustomer)          echo "CreateConnectorCustomer" ;;
        GetConnectorCustomer)             echo "GetConnectorCustomer" ;;
        MandateRevokeV2)                  echo "MandateRevoke" ;;
        ClientAuthentication)             echo "ClientAuthenticationToken" ;;
        ServerAuthentication)             echo "ServerAuthenticationToken" ;;
        ServerSessionAuthentication)      echo "ServerSessionAuthenticationToken" ;;
        SetupMandateV2)                   echo "SetupMandate" ;;
        RepeatPaymentV2)                  echo "RepeatPayment" ;;
        PaymentTokenV2)                   echo "PaymentMethodToken" ;;
        PaymentPreAuthenticateV2)         echo "PreAuthenticate" ;;
        PaymentAuthenticateV2)            echo "Authenticate" ;;
        PaymentPostAuthenticateV2)        echo "PostAuthenticate" ;;
        RefundV2)                         echo "Refund" ;;
        RefundSyncV2)                     echo "RSync" ;;
        RefundVoidPostRefundV2)           echo "VoidPostRefund" ;;
        AcceptDispute)                    echo "Accept" ;;
        SubmitEvidenceV2)                 echo "SubmitEvidence" ;;
        DisputeDefend)                    echo "DefendDispute" ;;
        *)                                echo "" ;;
    esac
}

# =============================================================================
# FLOW-TO-SUITE MAPPING (connector_specs/<connector>/specs.json)
# =============================================================================
# Mirrors `fn flow_to_suites(flow: &str) -> Option<&'static [&'static str]>` in
# crates/internal/integration-tests/src/bin/check_connector_specs.rs.
#
# CI runs that binary (`cargo run --all-features --bin check_connector_specs`,
# job "Compilation Check") and it calls std::process::exit(1) when a flow a
# connector declares has no matching suite in its specs.json. This table must
# therefore stay identical to the Rust one — re-derive it from that source file,
# never from memory.
#
# The Rust function returns a slice; today every arm holds exactly one suite, so
# this echoes a single suite name, or "" when the flow maps to no suite.
flow_to_suite() {
    case "$1" in
        # Core payment flows
        Authorize)                        echo "PaymentService/Authorize" ;;
        PSync)                            echo "PaymentService/Get" ;;
        Capture)                          echo "PaymentService/Capture" ;;
        Void)                             echo "PaymentService/Void" ;;
        Refund)                           echo "PaymentService/Refund" ;;
        RSync)                            echo "RefundService/Get" ;;
        # Recurring/mandate flows
        SetupMandate)                     echo "PaymentService/SetupRecurring" ;;
        RepeatPayment)                    echo "RecurringPaymentService/Charge" ;;
        MandateRevoke)                    echo "RecurringPaymentService/Revoke" ;;
        # Customer/token flows
        CreateConnectorCustomer)          echo "CustomerService/Create" ;;
        GetConnectorCustomer)             echo "CustomerService/Get" ;;
        PaymentMethodToken)               echo "PaymentMethodService/Tokenize" ;;
        PaymentMethodEligibility)         echo "PaymentMethodService/Eligibility" ;;
        # Authentication flows
        ServerAuthenticationToken)        echo "MerchantAuthenticationService/CreateServerAuthenticationToken" ;;
        ClientAuthenticationToken)        echo "MerchantAuthenticationService/CreateClientAuthenticationToken" ;;
        ServerSessionAuthenticationToken) echo "MerchantAuthenticationService/CreateServerSessionAuthenticationToken" ;;
        PreAuthenticate)                  echo "PaymentMethodAuthenticationService/PreAuthenticate" ;;
        Authenticate)                     echo "PaymentMethodAuthenticationService/Authenticate" ;;
        PostAuthenticate)                 echo "PaymentMethodAuthenticationService/PostAuthenticate" ;;
        # Advanced flows
        CreateOrder)                      echo "PaymentService/CreateOrder" ;;
        IncrementalAuthorization)         echo "PaymentService/IncrementalAuthorization" ;;
        *)                                echo "" ;;
    esac
}

# Mirrors `const OUT_OF_SCOPE_FLOWS` in check_connector_specs.rs.
#
# READ THIS BEFORE ACTING ON THE RESULT. "Out of scope" here means EXEMPT FROM
# THE SPECS CHECK, not "do not build". It is a statement about
# connector_specs/<name>/specs.json, never about whether the flow should be
# implemented. `check_connector_specs.rs` says so itself, immediately above the
# second half of the list:
#
#     // Implemented today with no suite to map to. Each is a coverage gap, not a
#     // decision that it should never be covered: add a suite, then move the flow
#     // into flow_to_suites above.
#
# So: `is_out_of_scope_flow VoidPC` returning 0 means "VoidPC needs no entry in
# supported_suites yet". It does NOT mean "skip VoidPC". VoidPC is implemented
# by 14 connectors and certified by none - that gap is the bug this list
# documents, not a decision to preserve. Implement the flow when the connector's
# API supports it; the suite catches up afterwards.
#
# This function only REPORTS membership. A flow reaches the Rust list "only by
# decision" - do not add a flow there to make CI pass.
#
# Returns 0 when the flow is exempt from the specs check, 1 otherwise.
is_out_of_scope_flow() {
    case "$1" in
        # Disputes — no suites yet.
        Accept|DefendDispute|SubmitEvidence) return 0 ;;
        # Payouts — out of scope for the payment suites.
        PayoutCreate|PayoutGet|PayoutStage|PayoutTransfer|PayoutVoid) return 0 ;;
        PayoutEnrollDisburseAccount|PayoutCreateRecipient|PayoutCreateLink) return 0 ;;
        # Implemented today with no suite to map to. Coverage gaps, NOT decisions -
        # build these when the connector supports them; the suite catches up.
        VoidPC|VerifyWebhookSource|VoidPostRefund|Recharge) return 0 ;;
        CreatePaymentMethod|GetPaymentMethod|RefreshPaymentMethod) return 0 ;;
        PreRiskCheck|PostRiskCheck) return 0 ;;
        FrmPaymentOutcome|FrmRefundProcessed|FrmChargebackReceived) return 0 ;;
        *) return 1 ;;
    esac
}

# Default `supported_suites` seed for a freshly scaffolded connector: the six
# core flows the new-connector skill implements. Same set as `CORE_FLOWS` in
# .skills/new-connector/references/subagent-prompts.md, which lists them as
# [Authorize, PSync, Capture, Refund, RSync, Void] - order is not significant
# here, the set is. Override with --flows.
readonly DEFAULT_SPEC_FLOWS="Authorize,PSync,Capture,Void,Refund,RSync"

# =============================================================================
# CONNECTOR KIND PROFILES  (--kind)
# =============================================================================
# `connectors/` is not the only connector directory. Four SIBLING directories
# live next to it under crates/integrations/connector-integration/src/ and each
# one has its own enum, its own module file, its own `ConnectorDataProvider`,
# its own `patch_*_connector_urls` fn and its own service trait:
#
#   src/connectors/               ConnectorEnum            ConnectorData
#   src/payout_connectors/        PayoutConnectorEnum      PayoutConnectorData
#   src/surcharge_connectors/     SurchargeConnectorEnum   SurchargeConnectorData
#   src/authenticator_connectors/ AuthenticatorConnectorEnum AuthenticatorConnectorData
#
# FRM is the odd one out: it has NO directory of its own. `connectors/kount.rs`
# is a full payment connector (it is in `ConnectorEnum`, in
# `default_implementations.rs`, in field-probe and in connector_specs/) that
# ALSO implements `FrmServiceTrait` and appears in `FrmConnectorEnum`. So
# `--kind frm` scaffolds a payment connector and then prints the FRM-only
# registration as a checklist - see show_kind_checklist().
#
# Verified against HEAD with:
#   ls crates/integrations/connector-integration/src/
#   sed -n '/pub enum PayoutConnectorEnum/,/^}/p' \
#     crates/types-traits/domain_types/src/connector_types.rs
#   grep -n 'impl ConnectorDataProvider' \
#     crates/integrations/connector-integration/src/types.rs

# Populated by resolve_kind_profile(); read by every update_* function.
KIND_DIR=""                     # directory name under connector-integration/src/
KIND_MODULE_FILE=""             # <KIND_DIR>.rs - where `pub mod <name>;` goes
KIND_ENUM=""                    # the enum a new connector of this kind joins
KIND_VARIANT=""                 # ConnectorVariant arm: Payment/Payout/Surcharge/Frm/Authenticator
KIND_PROVIDER=""                # the ConnectorDataProvider struct in integration types.rs
KIND_PATCH_FN=""                # domain_types/src/types.rs URL-patching fn
KIND_SERVICE_TRAIT=""           # the aggregate trait the connector must satisfy
KIND_TYPE_SUFFIX=""             # payouts append "Payouts" to the struct name
KIND_TYPE_NAME=""               # NAME_PASCAL + KIND_TYPE_SUFFIX (set in validate_inputs)
KIND_GENERIC=true               # false => non-generic struct, no <T>, no macros
KIND_IN_CONNECTOR_ENUM=false    # also gets a ConnectorEnum variant + gRPC mapping
KIND_NEEDS_SPECS=false          # gets connector_specs/<name>/specs.json
KIND_NEEDS_SUPERPOSITION=false  # gets a config/superposition.toml entry
KIND_NEEDS_FIELD_PROBE=false    # gets a field-probe dummy_auth arm
KIND_NEEDS_DEFAULT_IMPLS=false  # gets registered in default_implementations.rs
KIND_HAS_CHECKLIST=false        # print a manual-completion checklist at the end

# Resolve CONNECTOR_KIND into the KIND_* profile above.
#
# Every flag below was checked against HEAD rather than assumed. The two that
# most often get guessed wrong:
#
#   * connector_specs/ - `check_connector_specs.rs` reads exactly one directory,
#     `root.join("crates/integrations/connector-integration/src/connectors")`,
#     and FAILS when a connector_specs/<name>/ directory has no matching .rs
#     file in it. Creating a specs dir for a payout/surcharge/authenticator
#     connector therefore BREAKS CI rather than satisfying it.
#     Verify: grep -n 'let connectors_src' \
#       crates/internal/integration-tests/src/bin/check_connector_specs.rs
#
#   * superposition.toml - payout, surcharge and FRM connectors have no entry
#     (gotyme_sanlam, santander, deutschebank, interpayments, kount are all
#     absent); `plaid` does. Verify:
#       grep -c 'gotyme_sanlam\|interpayments\|kount' config/superposition.toml
#       grep -c 'connector = "plaid"' config/superposition.toml
resolve_kind_profile() {
    local src_dir="$CRATES_INTEGRATIONS/connector-integration/src"

    case "$CONNECTOR_KIND" in
        payment)
            KIND_DIR="connectors"
            KIND_ENUM="ConnectorEnum"
            KIND_VARIANT="Payment"
            KIND_PROVIDER="ConnectorData"
            KIND_PATCH_FN="patch_connector_urls"
            KIND_SERVICE_TRAIT="ConnectorServiceTrait"
            KIND_TYPE_SUFFIX=""
            KIND_GENERIC=true
            KIND_IN_CONNECTOR_ENUM=true
            KIND_NEEDS_SPECS=true
            KIND_NEEDS_SUPERPOSITION=true
            KIND_NEEDS_FIELD_PROBE=true
            KIND_NEEDS_DEFAULT_IMPLS=true
            KIND_HAS_CHECKLIST=false
            ;;
        payout)
            KIND_DIR="payout_connectors"
            KIND_ENUM="PayoutConnectorEnum"
            KIND_VARIANT="Payout"
            KIND_PROVIDER="PayoutConnectorData"
            KIND_PATCH_FN="patch_payout_connector_urls"
            KIND_SERVICE_TRAIT="PayoutServiceTrait"
            # payout_connectors.rs re-exports `<Pascal>Payouts`, never `<Pascal>`:
            #   grep -n 'pub use self' crates/integrations/connector-integration/src/payout_connectors.rs
            KIND_TYPE_SUFFIX="Payouts"
            KIND_GENERIC=true
            KIND_IN_CONNECTOR_ENUM=false
            KIND_NEEDS_SPECS=false
            KIND_NEEDS_SUPERPOSITION=false
            KIND_NEEDS_FIELD_PROBE=false
            KIND_NEEDS_DEFAULT_IMPLS=false
            KIND_HAS_CHECKLIST=true
            ;;
        surcharge)
            KIND_DIR="surcharge_connectors"
            KIND_ENUM="SurchargeConnectorEnum"
            KIND_VARIANT="Surcharge"
            KIND_PROVIDER="SurchargeConnectorData"
            KIND_PATCH_FN="patch_surcharge_connector_urls"
            KIND_SERVICE_TRAIT="SurchargeServiceTrait"
            KIND_TYPE_SUFFIX=""
            # The one connector in this category, surcharge_connectors/interpayments.rs,
            # is NON-GENERIC and uses NO connector macros. See create_connector_files().
            KIND_GENERIC=false
            KIND_IN_CONNECTOR_ENUM=false
            KIND_NEEDS_SPECS=false
            KIND_NEEDS_SUPERPOSITION=false
            KIND_NEEDS_FIELD_PROBE=false
            KIND_NEEDS_DEFAULT_IMPLS=false
            KIND_HAS_CHECKLIST=true
            ;;
        authenticator)
            KIND_DIR="authenticator_connectors"
            KIND_ENUM="AuthenticatorConnectorEnum"
            KIND_VARIANT="Authenticator"
            KIND_PROVIDER="AuthenticatorConnectorData"
            KIND_PATCH_FN="patch_authenticator_connector_urls"
            KIND_SERVICE_TRAIT="AuthenticatorServiceTrait"
            KIND_TYPE_SUFFIX=""
            KIND_GENERIC=true
            KIND_IN_CONNECTOR_ENUM=false
            KIND_NEEDS_SPECS=false
            KIND_NEEDS_SUPERPOSITION=true
            KIND_NEEDS_FIELD_PROBE=false
            KIND_NEEDS_DEFAULT_IMPLS=false
            KIND_HAS_CHECKLIST=true
            ;;
        frm)
            # FRM has no directory of its own: connectors/kount.rs is a payment
            # connector that also implements FrmServiceTrait. Scaffold the
            # payment side in full, then hand the operator the FRM-only steps.
            KIND_DIR="connectors"
            KIND_ENUM="ConnectorEnum"
            KIND_VARIANT="Payment"
            KIND_PROVIDER="ConnectorData"
            KIND_PATCH_FN="patch_connector_urls"
            KIND_SERVICE_TRAIT="ConnectorServiceTrait"
            KIND_TYPE_SUFFIX=""
            KIND_GENERIC=true
            KIND_IN_CONNECTOR_ENUM=true
            KIND_NEEDS_SPECS=true
            KIND_NEEDS_SUPERPOSITION=false
            KIND_NEEDS_FIELD_PROBE=true
            KIND_NEEDS_DEFAULT_IMPLS=true
            KIND_HAS_CHECKLIST=true
            ;;
        *)
            fatal_error "Unknown --kind '$CONNECTOR_KIND' (expected: payment, payout, surcharge, frm, authenticator)"
            ;;
    esac

    KIND_MODULE_FILE="$src_dir/${KIND_DIR}.rs"

    log_debug "Kind profile: kind=$CONNECTOR_KIND dir=$KIND_DIR enum=$KIND_ENUM provider=$KIND_PROVIDER"
}

# =============================================================================

readonly COLOR_RED='\033[0;31m'
readonly COLOR_GREEN='\033[0;32m'
readonly COLOR_YELLOW='\033[1;33m'
readonly COLOR_BLUE='\033[0;34m'
readonly COLOR_PURPLE='\033[0;35m'
readonly COLOR_CYAN='\033[0;36m'
readonly COLOR_RESET='\033[0m'

# =============================================================================
# GLOBAL VARIABLES
# =============================================================================

# User inputs
CONNECTOR_NAME=""
BASE_URL=""
PRODUCTION_URL=""   # Optional; defaults to BASE_URL. Used for the superposition production override.
# Which connector category to scaffold. Selects the target directory, the module
# file, the enum, the ConnectorDataProvider and the URL-patching fn - see
# resolve_kind_profile(). Default "payment" reproduces the pre---kind behaviour
# exactly.
CONNECTOR_KIND="payment"
FORCE_MODE=false
YES_MODE=false

# Auto-detected flows (populated by detect_flows_from_connector_service_trait)
SELECTED_FLOWS=()

# Generated values
NAME_SNAKE=""
NAME_PASCAL=""
NAME_UPPER=""
ENUM_ORDINAL=""
BACKUP_DIR=""

# payment.proto as it exists at PROTO_BASE_REF. Empty when that ref cannot be
# read (no remote, no fetch, shallow clone). Populated by
# load_proto_base_snapshot(); consumed by the two proto number allocators.
PROTO_BASE_SNAPSHOT=""

# Flow identifiers seeded into connector_specs/<name>/specs.json (--flows).
SPEC_FLOWS=""

# True when generate_connector_specs() created the connector_specs/<name>
# directory, so emergency_rollback() knows it is safe to remove.
SPECS_DIR_CREATED=false

# =============================================================================
# UTILITY FUNCTIONS
# =============================================================================

# Logging functions with consistent formatting
log_info() {
    echo -e "${COLOR_BLUE}ℹ️  INFO: $1${COLOR_RESET}"
}

log_success() {
    echo -e "${COLOR_GREEN}✅ SUCCESS: $1${COLOR_RESET}"
}

log_warning() {
    echo -e "${COLOR_YELLOW}⚠️  WARNING: $1${COLOR_RESET}"
}

log_error() {
    echo -e "${COLOR_RED}❌ ERROR: $1${COLOR_RESET}"
}

log_step() {
    echo -e "${COLOR_PURPLE}🔧 STEP: $1${COLOR_RESET}"
}

log_debug() {
    if [[ "${DEBUG:-false}" == "true" ]]; then
        echo -e "${COLOR_CYAN}🐛 DEBUG: $1${COLOR_RESET}"
    fi
}

# Error handling with context
fatal_error() {
    log_error "$1"
    log_error "Script execution terminated."
    exit 1
}

# Validation helpers
validate_file_exists() {
    local file="$1"
    local description="$2"

    if [[ ! -f "$file" ]]; then
        fatal_error "$description not found at: $file"
    fi
    log_debug "Validated file exists: $file"
}

validate_directory_exists() {
    local dir="$1"
    local description="$2"

    if [[ ! -d "$dir" ]]; then
        fatal_error "$description not found at: $dir"
    fi
    log_debug "Validated directory exists: $dir"
}

# String manipulation utilities
to_snake_case() {
    echo "$1" | sed 's/\([A-Z]\)/_\1/g' | sed 's/^_//' | tr '[:upper:]' '[:lower:]'
}

to_pascal_case() {
    # Convert snake_case to PascalCase
    echo "$1" | awk -F'_' '{for(i=1;i<=NF;i++) $i=toupper(substr($i,1,1)) tolower(substr($i,2))} 1' OFS=''
}

to_upper_case() {
    echo "$1" | tr '[:lower:]' '[:upper:]'
}

# =============================================================================
# HELP AND USAGE FUNCTIONS
# =============================================================================

show_version() {
    echo "$SCRIPT_NAME v$SCRIPT_VERSION"
}

show_help() {
    cat << EOF
$SCRIPT_NAME v$SCRIPT_VERSION

USAGE:
    $0 <connector_name> <base_url> [OPTIONS]

ARGUMENTS:
    connector_name    Name of the connector (snake_case, e.g., 'my_connector')
    base_url         Base URL for the connector API (sandbox / default)

OPTIONS:
    --production-url URL  Production base URL for the superposition production override
                          (defaults to base_url when omitted)
    --flows LIST          Comma-separated flow identifiers seeded into
                          connector_specs/<name>/specs.json
                          (default: $DEFAULT_SPEC_FLOWS)
    --kind KIND           Connector category. One of:
                            payment       (default) src/connectors/, ConnectorEnum,
                                          ConnectorData, specs.json, superposition,
                                          field-probe, default_implementations
                            payout        src/payout_connectors/, PayoutConnectorEnum,
                                          PayoutConnectorData. NO specs.json, NO
                                          superposition entry, NO field-probe arm,
                                          NO default_implementations registration.
                                          Struct is named <Pascal>Payouts.
                            surcharge     src/surcharge_connectors/,
                                          SurchargeConnectorEnum. NON-GENERIC struct,
                                          NO connector macros (see interpayments.rs).
                            authenticator src/authenticator_connectors/,
                                          AuthenticatorConnectorEnum. Bank-account
                                          linking / identity - NOT 3DS.
                            frm           connectors/ + FrmConnectorEnum. Scaffolds the
                                          PAYMENT side (kount.rs is a payment connector
                                          too) and prints the FRM-only steps.
    --list-flows     Show auto-detected flows from codebase
    --force          Ignore git status and force creation
    -y, --yes        Skip confirmation prompts
    --debug          Enable debug logging
    -h, --help       Show this help message
    -v, --version    Show version information

EXAMPLES:
    # Create connector (automatically detects all flows)
    $0 stripe https://api.stripe.com/v1

    # Force creation with auto-confirmation
    $0 example https://api.example.com --force -y

    # Provide a distinct production URL for the superposition override
    $0 example https://sandbox.example.com --production-url https://api.example.com

    # Seed specs.json with a different set of suites
    $0 example https://api.example.com --flows Authorize,PSync,Refund,RSync

    # Scaffold a payout connector (src/payout_connectors/example.rs, ExamplePayouts)
    $0 example https://api.example.com --kind payout

    # Scaffold a surcharge connector (non-generic, macro-free)
    $0 example https://api.example.com --kind surcharge

    # List auto-detected flows
    $0 --list-flows

FEATURES:
    • Auto-detects all flows from ConnectorServiceTrait
    • Future-proof: automatically includes new flows when added to codebase
    • Creates empty implementations for all detected flows
    • No manual flow configuration required
    • Writes connector_specs/<name>/specs.json (required by CI's
      check_connector_specs); merges into an existing file, never overwrites
    • Allocates payment.proto numbers against $PROTO_BASE_REF, not the local tree
    • --kind routes every write to that category's real registration sites

NOTE ON "OUT OF SCOPE" FLOWS:
    is_out_of_scope_flow() mirrors OUT_OF_SCOPE_FLOWS in check_connector_specs.rs.
    It means EXEMPT FROM THE SPECS CHECK - it is a statement about specs.json,
    never an instruction to skip the flow. check_connector_specs.rs itself says
    of the second half of that list: "Each is a coverage gap, not a decision that
    it should never be covered." Implement the flow if the connector's API
    supports it; the test suite catches up afterwards.

WORKFLOW:
    1. Auto-detects flows from connector_types.rs
    2. Validates environment and inputs
    3. Generates connector boilerplate with all flows
    4. Updates integration files
    5. Generates connector_specs/<name>/specs.json (payment / frm kinds only)
    6. Validates compilation
    7. Provides next steps guidance

For more information, visit: https://github.com/juspay/hyperswitch-prism
EOF
}

show_available_flows() {
    echo "Auto-Detected Flows from ConnectorServiceTrait:"
    echo "==============================================="
    echo

    # Auto-detect flows first
    detect_flows_from_connector_service_trait

    local flow
    for flow in "${AVAILABLE_FLOWS[@]}"; do
        local description=$(get_flow_description "$flow")
        printf "  %-25s %s\n" "$flow" "$description"
    done

    echo
    echo "NOTE: All flows are automatically included when creating a connector."
    echo "No manual selection is required - the script is future-proof!"
}

# =============================================================================
# ARGUMENT PARSING
# =============================================================================

parse_arguments() {
    log_debug "Parsing arguments: $*"

    # Handle special cases first
    if [[ $# -eq 0 ]]; then
        show_help
        exit 0
    fi

    if [[ $# -eq 1 ]]; then
        case "$1" in
            --list-flows)
                show_available_flows
                exit 0
                ;;
            -h|--help)
                show_help
                exit 0
                ;;
            -v|--version)
                show_version
                exit 0
                ;;
            *)
                log_error "Missing required arguments."
                show_help
                exit 1
                ;;
        esac
    fi

    # Parse required arguments
    if [[ $# -lt 2 ]]; then
        log_error "Missing required arguments: connector_name and base_url"
        show_help
        exit 1
    fi

    CONNECTOR_NAME="$1"
    BASE_URL="$2"
    shift 2

    # Parse optional arguments
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --production-url)
                if [[ $# -lt 2 ]]; then
                    fatal_error "--production-url requires a URL argument"
                fi
                PRODUCTION_URL="$2"
                shift 2
                ;;
            --flows)
                if [[ $# -lt 2 ]]; then
                    fatal_error "--flows requires a comma-separated flow list"
                fi
                SPEC_FLOWS="$2"
                shift 2
                ;;
            --kind)
                if [[ $# -lt 2 ]]; then
                    fatal_error "--kind requires a value (payment, payout, surcharge, frm, authenticator)"
                fi
                CONNECTOR_KIND="$2"
                shift 2
                ;;
            --force)
                FORCE_MODE=true
                shift
                ;;
            -y|--yes)
                YES_MODE=true
                shift
                ;;
            --debug)
                DEBUG=true
                shift
                ;;
            --list-flows)
                show_available_flows
                exit 0
                ;;
            -h|--help)
                show_help
                exit 0
                ;;
            -v|--version)
                show_version
                exit 0
                ;;
            *)
                fatal_error "Unknown option: $1"
                ;;
        esac
    done

    # Default the production URL to the sandbox base URL when not explicitly provided
    # (mirrors the existing behaviour of writing BASE_URL into production.toml).
    if [[ -z "$PRODUCTION_URL" ]]; then
        PRODUCTION_URL="$BASE_URL"
    fi

    # Default the specs.json seed to the six core flows.
    if [[ -z "$SPEC_FLOWS" ]]; then
        SPEC_FLOWS="$DEFAULT_SPEC_FLOWS"
    fi

    # Resolve --kind into the KIND_* profile before anything reads KIND_DIR.
    resolve_kind_profile

    log_debug "Arguments parsed successfully"
}

# =============================================================================
# VALIDATION FUNCTIONS
# =============================================================================

validate_environment() {
    log_step "Validating environment"

    validate_directory_exists "$TEMPLATE_DIR" "Template directory"
    validate_directory_exists "$CRATES_TRAITS" "types-traits crate directory"
    validate_directory_exists "$CRATES_INTEGRATIONS" "integrations crate directory"
    validate_directory_exists "$CRATES_INTERNAL" "internal crate directory"

    # Check required template files
    validate_file_exists "$CONNECTOR_TEMPLATE" "Connector template"
    validate_file_exists "$TRANSFORMERS_TEMPLATE" "Transformers template"

    # Check target files that will be modified
    validate_file_exists "$CONNECTOR_TYPES_FILE" "Connector types file"
    validate_file_exists "$DOMAIN_TYPES_FILE" "Domain types file"
    validate_file_exists "$INTEGRATION_TYPES_FILE" "Integration types file"
    validate_file_exists "$DEFAULT_IMPL_FILE" "Default implementations file"
    validate_file_exists "$CONNECTORS_MODULE_FILE" "Connectors module file"
    validate_file_exists "$KIND_MODULE_FILE" "Module file for --kind $CONNECTOR_KIND"
    validate_directory_exists "$CRATES_INTEGRATIONS/connector-integration/src/$KIND_DIR" \
        "Connector directory for --kind $CONNECTOR_KIND"
    validate_file_exists "$PROTO_FILE" "Protocol buffer file"
    validate_file_exists "$FIELD_PROBE_FILE" "Field probe file"

    # Check git status unless forced
    if [[ "$FORCE_MODE" == "false" ]] && command -v git >/dev/null 2>&1; then
        if [[ -n "$(git status --porcelain 2>/dev/null)" ]]; then
            fatal_error "Git working directory is not clean. Use --force to proceed anyway."
        fi
    fi

    log_success "Environment validation passed"
}

validate_inputs() {
    log_step "Validating inputs"

    # Validate connector name
    if [[ ! "$CONNECTOR_NAME" =~ ^[a-z][a-z0-9_]*$ ]]; then
        fatal_error "Connector name must start with a letter and contain only lowercase letters, numbers, and underscores"
    fi

    # Validate base URL
    if [[ ! "$BASE_URL" =~ ^https?://.+ ]]; then
        fatal_error "Base URL must be a valid HTTP/HTTPS URL"
    fi

    # Generate name variants
    NAME_SNAKE="$CONNECTOR_NAME"
    NAME_PASCAL=$(to_pascal_case "$CONNECTOR_NAME")
    NAME_UPPER=$(to_upper_case "$CONNECTOR_NAME")

    # The Rust struct name. Payout connectors are re-exported as `<Pascal>Payouts`
    # (payout_connectors.rs: `pub use self::trustly::TrustlyPayouts;`); every other
    # kind uses the bare Pascal name.
    KIND_TYPE_NAME="${NAME_PASCAL}${KIND_TYPE_SUFFIX}"

    # Auto-detect flows from codebase
    detect_flows_from_connector_service_trait

    # Always use all detected flows (no manual selection)
    SELECTED_FLOWS=("${AVAILABLE_FLOWS[@]}")

    log_success "Input validation passed"
    log_info "Configuration: $NAME_SNAKE → $NAME_PASCAL"
    log_info "Base URL: $BASE_URL"
    log_info "Kind: $CONNECTOR_KIND (src/$KIND_DIR/$NAME_SNAKE.rs, struct $KIND_TYPE_NAME)"
    log_info "Auto-detected ${#SELECTED_FLOWS[@]} flows: ${SELECTED_FLOWS[*]}"
}

check_naming_conflicts() {
    log_step "Checking for naming conflicts"

    # Check if connector files already exist (in THIS kind's directory)
    local connector_file="$CRATES_INTEGRATIONS/connector-integration/src/$KIND_DIR/$NAME_SNAKE.rs"
    local connector_dir="$CRATES_INTEGRATIONS/connector-integration/src/$KIND_DIR/$NAME_SNAKE"

    if [[ -f "$connector_file" ]] || [[ -d "$connector_dir" ]]; then
        if [[ "$FORCE_MODE" == "false" ]]; then
            fatal_error "Connector '$NAME_SNAKE' already exists. Use --force to override."
        else
            log_warning "Connector files exist but will be overwritten due to --force mode"
        fi
    fi

    # Check protobuf enum (skip if --force mode)
    if [[ "$FORCE_MODE" == "false" ]] && grep -q "$NAME_UPPER =" "$PROTO_FILE" 2>/dev/null; then
        fatal_error "Connector '$NAME_UPPER' already exists in protobuf enum"
    elif grep -q "$NAME_UPPER =" "$PROTO_FILE" 2>/dev/null; then
        log_warning "Connector '$NAME_UPPER' already in protobuf enum, will skip protobuf update"
    fi

    # Check the kind's own enum (skip if --force mode)
    if [[ "$FORCE_MODE" == "false" ]] && grep -q "^[[:space:]]*$NAME_PASCAL,\$" "$DOMAIN_TYPES_FILE" 2>/dev/null; then
        log_warning "'$NAME_PASCAL' already appears as an enum variant in connector_types.rs"
    fi

    # Check domain types (skip if --force mode)
    if [[ "$FORCE_MODE" == "false" ]] && grep -q "$NAME_PASCAL" "$DOMAIN_TYPES_FILE" 2>/dev/null; then
        fatal_error "Connector '$NAME_PASCAL' already exists in domain types"
    elif grep -q "$NAME_PASCAL" "$DOMAIN_TYPES_FILE" 2>/dev/null; then
        log_warning "Connector '$NAME_PASCAL' already in domain types, will skip domain types update"
    fi

    log_success "Conflict check completed"
}

# =============================================================================
# CORE GENERATION FUNCTIONS
# =============================================================================

# Load payment.proto as it exists at PROTO_BASE_REF into PROTO_BASE_SNAPSHOT.
#
# Strictly read-only: `git show` only. No fetch, no checkout, no index writes.
# On any failure the snapshot is left empty and the allocators fall back to the
# local working tree, with a warning.
load_proto_base_snapshot() {
    log_step "Reading payment.proto from $PROTO_BASE_REF for proto number allocation"

    PROTO_BASE_SNAPSHOT=""

    if ! command -v git >/dev/null 2>&1; then
        log_warning "git not found - falling back to the LOCAL payment.proto for number allocation"
        return 0
    fi

    if ! git -C "$ROOT_DIR" rev-parse --verify --quiet "$PROTO_BASE_REF" >/dev/null 2>&1; then
        log_warning "Ref '$PROTO_BASE_REF' not found - falling back to the LOCAL payment.proto"
        log_warning "  Run 'git fetch origin main' first, or set PROTO_BASE_REF to a ref you have."
        return 0
    fi

    local snapshot
    if ! snapshot=$(git -C "$ROOT_DIR" show "$PROTO_BASE_REF:$PROTO_FILE_RELPATH" 2>/dev/null); then
        log_warning "Could not read $PROTO_FILE_RELPATH at $PROTO_BASE_REF - falling back to the LOCAL file"
        return 0
    fi
    PROTO_BASE_SNAPSHOT="$snapshot"

    local base_desc
    base_desc=$(git -C "$ROOT_DIR" log -1 --format='%h %cd' --date=short "$PROTO_BASE_REF" 2>/dev/null || echo "unknown")
    log_info "Proto number base: $PROTO_BASE_REF ($base_desc)"
    log_warning "That ref is only as fresh as your last fetch. Run 'git fetch origin main' before trusting it."
}

# Highest `= N;` inside `enum Connector { ... }` of the proto content on stdin.
# Echoes nothing when the block is absent or holds no numbers.
max_connector_ordinal() {
    sed -n '/^enum Connector {/,/^}/p' \
        | { grep -o '= [0-9]\+;' || true; } \
        | { grep -o '[0-9]\+' || true; } \
        | sort -n \
        | tail -1
}

get_next_enum_ordinal() {
    log_step "Determining next Connector enum ordinal"

    local local_max="" base_max=""

    if [[ -f "$PROTO_FILE" ]]; then
        local_max=$(max_connector_ordinal < "$PROTO_FILE")
    fi

    if [[ -n "$PROTO_BASE_SNAPSHOT" ]]; then
        base_max=$(printf '%s\n' "$PROTO_BASE_SNAPSHOT" | max_connector_ordinal)
    fi

    # Allocate against the highest number seen in EITHER place.
    #   - The base ref alone is not enough: the local tree may already carry an
    #     unmerged connector holding a higher number.
    #   - The local tree alone is not enough: that is exactly the stale-branch
    #     bug this replaces. A branch cut before an upstream connector landed
    #     re-issues a number main has already taken.
    local max_ordinal=0
    if [[ -n "$local_max" ]] && (( local_max > max_ordinal )); then
        max_ordinal=$local_max
    fi
    if [[ -n "$base_max" ]] && (( base_max > max_ordinal )); then
        max_ordinal=$base_max
    fi

    if (( max_ordinal == 0 )); then
        ENUM_ORDINAL=100
    else
        ENUM_ORDINAL=$((max_ordinal + 1))
    fi

    log_info "Highest Connector ordinal - local tree: ${local_max:-none}, $PROTO_BASE_REF: ${base_max:-unavailable}"
    log_info "Allocating Connector ordinal: $ENUM_ORDINAL"
    warn_proto_number_race "Connector enum ordinal" "$ENUM_ORDINAL" \
        "git show origin/main:$PROTO_FILE_RELPATH | sed -n '/^enum Connector {/,/^}/p' | grep -oE '= [0-9]+;' | grep -oE '[0-9]+' | sort -n | tail -1"

    log_debug "Next enum ordinal: $ENUM_ORDINAL"
}

# Tell the operator, in the loudest terms this script has, that a proto number is
# a claim on a SHARED namespace and only holds until someone else merges first.
warn_proto_number_race() {
    local what="$1"
    local number="$2"
    # Namespace-specific re-verification pipeline. `enum Connector` ordinals and
    # `oneof connector_config` field numbers are SEPARATE namespaces - checking
    # one tells you nothing about the other, so each call site passes its own.
    local verify_cmd="$3"

    log_warning "PROTO NUMBER RACE: this $what ($number) is reserved on YOUR branch only."
    log_warning "  Three connectors have already collided this way:"
    log_warning "    GOTYME_SANLAM vs JPM, JPM vs PayNearMe, TRAVELHUB vs PayNearMe."
    log_warning "  IMMEDIATELY BEFORE opening the PR, re-verify with:"
    log_warning "    git fetch origin main"
    log_warning "    $verify_cmd"
    log_warning "  If upstream now holds a number >= $number, renumber before you push."
}

create_backup() {
    log_step "Creating backup"

    BACKUP_DIR="$ROOT_DIR/.connector_backup_$(date +%s)"
    mkdir -p "$BACKUP_DIR"

    local files_to_backup=(
        "$PROTO_FILE"
        "$DOMAIN_TYPES_FILE"
        "$DOMAIN_TYPES_TYPES_FILE"
        "$CONNECTORS_MODULE_FILE"
        "$KIND_MODULE_FILE"
        "$INTEGRATION_TYPES_FILE"
        "$DEFAULT_IMPL_FILE"
        "$ROUTER_DATA_FILE"
        "$FIELD_PROBE_FILE"
        "$CONFIG_FILE"
        "$SANDBOX_CONFIG_FILE"
        "$PRODUCTION_CONFIG_FILE"
        "$SUPERPOSITION_CONFIG_FILE"
    )

    local file
    for file in "${files_to_backup[@]}"; do
        if [[ -f "$file" ]]; then
            # Create unique backup names for files with same basename
            if [[ "$file" == "$DOMAIN_TYPES_TYPES_FILE" ]]; then
                cp "$file" "$BACKUP_DIR/domain_types_types.rs"
                log_debug "Backed up: domain_types/types.rs"
            elif [[ "$file" == "$INTEGRATION_TYPES_FILE" ]]; then
                cp "$file" "$BACKUP_DIR/integration_types.rs"
                log_debug "Backed up: connector-integration/types.rs"
            elif [[ "$file" == "$DEFAULT_IMPL_FILE" ]]; then
                cp "$file" "$BACKUP_DIR/default_implementations.rs"
                log_debug "Backed up: connector-integration/default_implementations.rs"
            elif [[ "$file" == "$ROUTER_DATA_FILE" ]]; then
                cp "$file" "$BACKUP_DIR/router_data.rs"
                log_debug "Backed up: domain_types/router_data.rs"
            elif [[ "$file" == "$FIELD_PROBE_FILE" ]]; then
                cp "$file" "$BACKUP_DIR/field_probe_auth.rs"
                log_debug "Backed up: field-probe/auth.rs"
            elif [[ "$file" == "$KIND_MODULE_FILE" ]]; then
                # payout_connectors.rs / surcharge_connectors.rs /
                # authenticator_connectors.rs. For --kind payment|frm this path
                # equals CONNECTORS_MODULE_FILE and was already copied above, so
                # `cp` here is a harmless no-op overwrite of the same bytes.
                cp "$file" "$BACKUP_DIR/$(basename "$file")"
                log_debug "Backed up: $(basename "$file")"
            else
                cp "$file" "$BACKUP_DIR/$(basename "$file")"
                log_debug "Backed up: $(basename "$file")"
            fi
        fi
    done

    # An existing connector_specs/<name>/specs.json is merged into later, so it
    # needs a backup too. Stored under a unique name: its basename would be
    # ambiguous next to the other backups.
    local existing_specs="$CONNECTOR_SPECS_ROOT/$NAME_SNAKE/specs.json"
    if [[ -f "$existing_specs" ]]; then
        cp "$existing_specs" "$BACKUP_DIR/connector_specs_specs.json"
        log_debug "Backed up: connector_specs/$NAME_SNAKE/specs.json"
    fi

    log_success "Backup created at: $BACKUP_DIR"
}

# Expand a template.
#
# {{CONNECTOR_NAME_PASCAL}} does double duty in connector.rs.template: it names
# the CONNECTOR STRUCT (`pub struct X<T>`, `for X<T>`) and it prefixes the two
# TRANSFORMER TYPES (`XAuthType`, `XErrorResponse`). Those two names diverge for
# any kind with a struct-name suffix - a payout connector's struct is
# `TrustlyPayouts` while its transformers are `TrustlyAuthType` /
# `TrustlyErrorResponse` (payout_connectors/trustly.rs). So when
# KIND_TYPE_SUFFIX is non-empty, resolve the AuthType/ErrorResponse occurrences
# FIRST (to the bare Pascal name), then everything left over to the suffixed
# struct name. Order matters: the specific substitution has to run before the
# general one.
substitute_template_variables() {
    local input_file="$1"
    local output_file="$2"

    log_debug "Substituting variables in template: $(basename "$input_file")"

    if [[ -n "$KIND_TYPE_SUFFIX" ]]; then
        sed -e "s/{{CONNECTOR_NAME_PASCAL}}\(AuthType\|ErrorResponse\)/$NAME_PASCAL\1/g" \
            -e "s/{{CONNECTOR_NAME_PASCAL}}/$KIND_TYPE_NAME/g" \
            -e "s/{{CONNECTOR_NAME_SNAKE}}/$NAME_SNAKE/g" \
            -e "s/{{CONNECTOR_NAME_UPPER}}/$NAME_UPPER/g" \
            -e "s|{{BASE_URL}}|$BASE_URL|g" \
            "$input_file" > "$output_file"
    else
        sed -e "s/{{CONNECTOR_NAME_PASCAL}}/$NAME_PASCAL/g" \
            -e "s/{{CONNECTOR_NAME_SNAKE}}/$NAME_SNAKE/g" \
            -e "s/{{CONNECTOR_NAME_UPPER}}/$NAME_UPPER/g" \
            -e "s|{{BASE_URL}}|$BASE_URL|g" \
            "$input_file" > "$output_file"
    fi
}

create_connector_files() {
    log_step "Creating connector files (--kind $CONNECTOR_KIND)"

    local connectors_dir="$CRATES_INTEGRATIONS/connector-integration/src/$KIND_DIR"
    local connector_subdir="$connectors_dir/$NAME_SNAKE"

    mkdir -p "$connector_subdir"

    case "$CONNECTOR_KIND" in
        surcharge)
            # NOT the shared template: surcharge connectors are non-generic and
            # macro-free. See write_surcharge_connector_file().
            write_surcharge_connector_file "$connectors_dir/$NAME_SNAKE.rs"
            write_minimal_transformers "$connector_subdir/transformers.rs"
            ;;
        payout|authenticator)
            # Same generic struct + ConnectorCommon shell as a payment connector,
            # but the payment-shaped Authorize transformers in
            # transformers.rs.template do not belong here: PayoutFlowData /
            # MerchantAuthenticationFlowData are the resource_common_data for
            # these kinds, not PaymentFlowData. Emit only the two types the
            # ConnectorCommon shell actually needs.
            substitute_template_variables "$CONNECTOR_TEMPLATE" "$connectors_dir/$NAME_SNAKE.rs"
            write_minimal_transformers "$connector_subdir/transformers.rs"
            generate_dynamic_implementations "$connectors_dir/$NAME_SNAKE.rs"
            ;;
        *)
            substitute_template_variables "$CONNECTOR_TEMPLATE" "$connectors_dir/$NAME_SNAKE.rs"
            substitute_template_variables "$TRANSFORMERS_TEMPLATE" "$connector_subdir/transformers.rs"
            generate_dynamic_implementations "$connectors_dir/$NAME_SNAKE.rs"
            ;;
    esac

    log_success "Created connector files in src/$KIND_DIR/"
}

# Minimal transformers.rs for the non-payment kinds: the AuthType and the error
# body, which are exactly what the ConnectorCommon shell in
# connector.rs.template references. Shape copied verbatim from the head of
# template-generation/transformers.rs.template so the two stay consistent.
write_minimal_transformers() {
    local out="$1"

    cat > "$out" <<EOF
use common_utils::consts::{NO_ERROR_CODE, NO_ERROR_MESSAGE};
use domain_types::{errors, router_data::ConnectorSpecificConfig};
use hyperswitch_masking::Secret;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct ${NAME_PASCAL}AuthType {
    pub api_key: Secret<String>,
}

// Auth is read from \`ConnectorSpecificConfig\`, NOT from a \`connector_auth_type\`
// field on RouterDataV2 - that field was deleted from RouterDataV2. The variant
// matched below is the one add_connector.sh appends to
// \`domain_types::router_data::ConnectorSpecificConfig\`.
impl TryFrom<&ConnectorSpecificConfig> for ${NAME_PASCAL}AuthType {
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::${NAME_PASCAL} { api_key, .. } => Ok(Self {
                api_key: api_key.to_owned(),
            }),
            _ => Err(error_stack::report!(
                errors::IntegrationError::FailedToObtainAuthType {
                    context: errors::IntegrationErrorContext::default()
                }
            )),
        }
    }
}

// Every field is \`Option\` on purpose: a vendor error body that omits \`code\` or
// \`message\` must still deserialize. The NO_ERROR_CODE / NO_ERROR_MESSAGE
// sentinels are applied at the call site in \`build_error_response\` - never
// \`.unwrap_or_default()\`, which turns a missing code into an empty string
// indistinguishable from a real empty code.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ${NAME_PASCAL}ErrorResponse {
    pub code: Option<String>,
    pub message: Option<String>,
}

// Silence the unused-import warning until build_error_response's call site is
// customised; both sentinels are referenced from the connector file.
#[allow(dead_code)]
const _SENTINELS: (&str, &str) = (NO_ERROR_CODE, NO_ERROR_MESSAGE);
EOF

    log_debug "Wrote minimal transformers.rs for --kind $CONNECTOR_KIND"
}

# Emit a surcharge connector.
#
# WHY THIS DOES NOT USE THE SHARED TEMPLATE OR ANY MACRO
# -----------------------------------------------------
# GRACE's most-repeated codegen rule - "declare flows through
# macros::create_all_prerequisites! and macros::macro_connector_implementation!,
# never hand-roll ConnectorIntegrationV2" - does not hold in this directory.
# The only surcharge connector on HEAD,
# crates/integrations/connector-integration/src/surcharge_connectors/interpayments.rs,
# is:
#
#   * NON-GENERIC. `pub struct InterPayments;` - a unit struct, no
#     `<T: PaymentMethodDataTypes>` parameter and no PhantomData. Every impl is
#     `impl ... for InterPayments`, never `for InterPayments<T>`.
#   * MACRO-FREE. It contains no create_all_prerequisites!, no
#     macro_connector_implementation! and no macro_connector_flow_status_impls!.
#     Its three flows are raw `impl ConnectorIntegrationV2<...> for InterPayments`
#     blocks. The one macro it does use is
#     `common_macros::create_amount_converter_wrapper!`, which is not a flow macro.
#
# That is not an oversight to be "fixed": every connector macro in
# connectors/macros.rs is written as `impl<$g: $($b)*> ... for $c<$g>` and so
# requires a generic connector. Handing a unit struct to
# macro_connector_flow_status_impls! does not compile.
#
# Verify before changing any of this:
#   grep -n 'pub struct InterPayments' src/surcharge_connectors/interpayments.rs
#   grep -c 'macro_connector_' src/surcharge_connectors/interpayments.rs   # -> 0
#   sed -n '/pub trait SurchargeServiceTrait/,/^}/p' \
#     crates/types-traits/interfaces/src/connector_types.rs
#
# SurchargeServiceTrait = ConnectorCommon + ValidationTrait + SurchargeCalculateV2
# + SurchargePaymentSucceededV2 + SurchargeRefundSucceededV2, and each of those
# three flow traits binds ConnectorIntegrationV2<Flow, SurchargeFlowData, Req, Resp>.
# The three stubs below satisfy them; replace each `get_url` with the real
# endpoint (and add get_headers / get_request_body / handle_response) as you
# implement the flow, copying interpayments.rs.
write_surcharge_connector_file() {
    local out="$1"

    cat > "$out" <<EOF
pub mod transformers;

use common_enums::CurrencyUnit;
use common_utils::{
    consts::{NO_ERROR_CODE, NO_ERROR_MESSAGE},
    errors::CustomResult,
    events,
    ext_traits::ByteSliceExt,
};
use domain_types::{
    connector_flow::{SurchargeCalculate, SurchargePaymentSucceeded, SurchargeRefundSucceeded},
    errors::{ConnectorError, IntegrationError, IntegrationErrorContext},
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    surcharge::surcharge_types::{
        SurchargeCalculateRequest, SurchargeCalculateResponse, SurchargeFlowData,
        SurchargePaymentSucceededRequest, SurchargePaymentSucceededResponse,
        SurchargeRefundSucceededRequest, SurchargeRefundSucceededResponse,
    },
    types::Connectors,
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, Mask, Maskable};
use interfaces::{
    api::ConnectorCommon,
    connector_integration_v2::ConnectorIntegrationV2,
    connector_types::{
        SurchargeCalculateV2, SurchargePaymentSucceededV2, SurchargeRefundSucceededV2,
        SurchargeServiceTrait, ValidationTrait,
    },
};
use transformers as ${NAME_SNAKE};

use crate::{common_macros, connectors::macros, with_error_response_body};

pub(crate) mod headers {
    pub(crate) const AUTHORIZATION: &str = "Authorization";
}

// NON-GENERIC ON PURPOSE. Surcharge connectors are unit structs - see
// surcharge_connectors/interpayments.rs. Do NOT add \`<T: PaymentMethodDataTypes>\`
// and do NOT reach for crate::connectors::macros here: every one of those macros
// expands to \`impl<T: ...> ... for \$connector<T>\` and will not apply to this type.
pub struct ${NAME_PASCAL};

impl ${NAME_PASCAL} {
    pub const fn new() -> &'static Self {
        &Self
    }
}

common_macros::create_amount_converter_wrapper!(connector_name: ${NAME_PASCAL}, amount_type: FloatMajorUnit);

impl ConnectorCommon for ${NAME_PASCAL} {
    fn id(&self) -> &'static str {
        "${NAME_SNAKE}"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Base
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        &connectors.${NAME_SNAKE}.base_url
    }

    fn get_auth_header(
        &self,
        auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        let auth = ${NAME_SNAKE}::${NAME_PASCAL}AuthType::try_from(auth_type).change_context(
            IntegrationError::FailedToObtainAuthType {
                context: IntegrationErrorContext::default(),
            },
        )?;

        // \`.into_masked()\` (the \`Mask\` trait), NOT \`.into()\`: \`impl From<T> for
        // Maskable<T>\` builds \`Maskable::Normal\`, which logs the token in clear.
        Ok(vec![(
            headers::AUTHORIZATION.to_string(),
            format!("Bearer {}", auth.api_key.expose()).into_masked(),
        )])
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        let response: ${NAME_SNAKE}::${NAME_PASCAL}ErrorResponse = res
            .response
            .parse_struct("${NAME_PASCAL}ErrorResponse")
            .change_context(crate::utils::response_deserialization_fail(
                res.status_code,
                "${NAME_SNAKE}: error body did not match the documented error shape",
            ))?;

        with_error_response_body!(event_builder, response);

        Ok(ErrorResponse {
            status_code: res.status_code,
            code: response
                .code
                .clone()
                .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
            message: response
                .message
                .clone()
                .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
            reason: response.message.clone(),
            typed_connector_response: macros::serialize_typed_connector_payload(
                &response,
                "typed_connector_response",
            ),
            ..Default::default()
        })
    }
}

// ===== MARKER TRAITS =====
// SurchargeServiceTrait requires ConnectorCommon + ValidationTrait + the three
// flow traits below. Verify with:
//   sed -n '/pub trait SurchargeServiceTrait/,/^}/p' \\
//     crates/types-traits/interfaces/src/connector_types.rs
impl ValidationTrait for ${NAME_PASCAL} {}
impl SurchargeServiceTrait for ${NAME_PASCAL} {}
impl SurchargeCalculateV2 for ${NAME_PASCAL} {}
impl SurchargePaymentSucceededV2 for ${NAME_PASCAL} {}
impl SurchargeRefundSucceededV2 for ${NAME_PASCAL} {}

// ===== FLOW STUBS =====
// Hand-written because no macro applies to a non-generic connector (see the
// comment on write_surcharge_connector_file in add_connector.sh). Each block
// mirrors what \`flow_status_emit!(status: not_implemented)\` emits for the
// generic kinds: only \`get_url\`, returning connector_flow_not_implemented.
// Replace one at a time with the real implementation, copying the corresponding
// block in surcharge_connectors/interpayments.rs.

impl
    ConnectorIntegrationV2<
        SurchargeCalculate,
        SurchargeFlowData,
        SurchargeCalculateRequest,
        SurchargeCalculateResponse,
    > for ${NAME_PASCAL}
{
    fn get_url(
        &self,
        _req: &RouterDataV2<
            SurchargeCalculate,
            SurchargeFlowData,
            SurchargeCalculateRequest,
            SurchargeCalculateResponse,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Err(IntegrationError::connector_flow_not_implemented(
            ConnectorCommon::id(self),
            "surcharge_calculate",
            IntegrationErrorContext::default(),
        )
        .into())
    }
}

impl
    ConnectorIntegrationV2<
        SurchargePaymentSucceeded,
        SurchargeFlowData,
        SurchargePaymentSucceededRequest,
        SurchargePaymentSucceededResponse,
    > for ${NAME_PASCAL}
{
    fn get_url(
        &self,
        _req: &RouterDataV2<
            SurchargePaymentSucceeded,
            SurchargeFlowData,
            SurchargePaymentSucceededRequest,
            SurchargePaymentSucceededResponse,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Err(IntegrationError::connector_flow_not_implemented(
            ConnectorCommon::id(self),
            "surcharge_payment_succeeded",
            IntegrationErrorContext::default(),
        )
        .into())
    }
}

impl
    ConnectorIntegrationV2<
        SurchargeRefundSucceeded,
        SurchargeFlowData,
        SurchargeRefundSucceededRequest,
        SurchargeRefundSucceededResponse,
    > for ${NAME_PASCAL}
{
    fn get_url(
        &self,
        _req: &RouterDataV2<
            SurchargeRefundSucceeded,
            SurchargeFlowData,
            SurchargeRefundSucceededRequest,
            SurchargeRefundSucceededResponse,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Err(IntegrationError::connector_flow_not_implemented(
            ConnectorCommon::id(self),
            "surcharge_refund_succeeded",
            IntegrationErrorContext::default(),
        )
        .into())
    }
}
EOF

    log_debug "Wrote non-generic, macro-free surcharge connector file"
}

# Append the payout trait block.
#
# PayoutServiceTrait is NOT a relative of ConnectorServiceTrait. Its full
# supertrait list is ConnectorCommon + ServerAuthentication + the nine Payout*
# flow traits - it requires NO ValidationTrait, NO IncomingWebhook, NO
# VerifyRedirectResponse and NO SourceVerification, so none of those are emitted
# here. Verify with:
#   sed -n '/pub trait PayoutServiceTrait/,/^{/p' \
#     crates/types-traits/interfaces/src/connector_types.rs
generate_payout_implementations() {
    local connector_file="$1"

    log_step "Generating payout trait implementations"

    cat >> "$connector_file" <<EOF

// =============================================================================
// DYNAMICALLY GENERATED IMPLEMENTATIONS (--kind payout)
// =============================================================================
// Auto-generated by add_connector.sh. Exemplars to copy as you implement:
//   payout_connectors/gotyme_sanlam.rs  - two real flows + stubs for the rest
//   payout_connectors/trustly.rs        - explicit payout_flows list
//   payout_connectors/deutschebank.rs   - real PayoutEligibility
//
// NOT payout_connectors/itaubank.rs's ancestor connectors/itaubank.rs: that file
// has had no payout code since payouts moved into their own directory.
// =============================================================================

// ===== PAYOUT SERVICE TRAIT =====
// PayoutServiceTrait = ConnectorCommon + ServerAuthentication + nine Payout*
// flows. It does NOT require ValidationTrait / IncomingWebhook /
// VerifyRedirectResponse, which is why this file has none of them.
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PayoutServiceTrait for ${KIND_TYPE_NAME}<T>
{
}

// ===== PAYOUT FLOW STUBS =====
// The no-explicit-list arm of macro_connector_payout_implementation! expands to
// ALL NINE payout flows: PayoutCreate, PayoutTransfer, PayoutGet, PayoutVoid,
// PayoutStage, PayoutCreateLink, PayoutCreateRecipient,
// PayoutEnrollDisburseAccount, PayoutEligibility.
//
// PayoutEligibility IS covered. It gained an arm in expand_payout_implementation!
// in commit 3df5eb702 (GoTyme payout connector); the comment in
// payout_connectors/trustly.rs that says it has no arm predates that and is
// stale. Do NOT hand-write a PayoutEligibility stub on top of this macro call -
// that is two ConnectorIntegrationV2<PayoutEligibility, ...> impls for one type,
// which is E0119.
//   Verify: grep -n 'flow: PayoutEligibility' crates/integrations/connector-integration/src/connectors/macros.rs
//
// To implement a flow for real, pass an explicit payout_flows: [...] list here
// naming only the flows to keep stubbed (see gotyme_sanlam.rs), then add a
// macros::macro_connector_implementation! block for the one you implemented.
crate::connectors::macros::macro_connector_payout_implementation!(
    connector: ${KIND_TYPE_NAME},
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);

// ===== ServerAuthentication =====
// The tenth supertrait of PayoutServiceTrait. It has no arm in
// expand_payout_implementation!, but it does have one in expand_flow_status_impl!,
// which emits both the ServerAuthentication marker impl and the stub
// ConnectorIntegrationV2<ServerAuthenticationToken, MerchantAuthenticationFlowData,
// ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData>.
// Note the resource_common_data: MerchantAuthenticationFlowData, NOT
// PaymentFlowData and NOT PayoutFlowData.
crate::connectors::macros::macro_connector_flow_status_impls!(
    connector: ${KIND_TYPE_NAME},
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [ServerAuthenticationToken],
);
EOF

    log_success "Generated payout implementations (PayoutServiceTrait + 9 payout stubs + ServerAuthenticationToken)"
}

# Append the authenticator trait block.
#
# AuthenticatorServiceTrait<T> = ConnectorCommon + ValidationTrait +
# ClientAuthentication + PaymentTokenV2<T> + GetPaymentMethodV2. Verify with:
#   sed -n '/pub trait AuthenticatorServiceTrait/,/^{/p' \
#     crates/types-traits/interfaces/src/connector_types.rs
#
# THIS IS NOT 3DS. authenticator_connectors/ holds bank-account linking and
# identity connectors (plaid). Standalone 3DS is the
# PreAuthenticate/Authenticate/PostAuthenticate trio on a PAYMENT connector, and
# external 3DS providers (Netcetera aside) never reach UCS at all.
generate_authenticator_implementations() {
    local connector_file="$1"

    log_step "Generating authenticator trait implementations"

    cat >> "$connector_file" <<EOF

// =============================================================================
// DYNAMICALLY GENERATED IMPLEMENTATIONS (--kind authenticator)
// =============================================================================
// Exemplar: authenticator_connectors/plaid.rs.
//
// This directory is ACCOUNT LINKING / IDENTITY, not 3D Secure. If you are
// scaffolding a 3DS connector you want --kind payment plus the
// PreAuthenticate / Authenticate / PostAuthenticate trio (connectors/netcetera.rs
// is the authentication-only payment connector), and you must also override
// ValidationTrait::next_authentication_step - without that override the composite
// authorize loop in crates/internal/composite-service/src/payments.rs never
// dispatches the 3DS legs. connectors/barclaycard.rs is the canonical override.
// =============================================================================

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for ${NAME_PASCAL}<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::AuthenticatorServiceTrait<T> for ${NAME_PASCAL}<T>
{
}

// The three flow traits AuthenticatorServiceTrait requires. Each identifier
// below has an arm in expand_flow_status_impl!, so the macro emits the marker
// impl (ClientAuthentication / PaymentTokenV2<T> / GetPaymentMethodV2) AND the
// stub ConnectorIntegrationV2 impl.
//
// ClientAuthenticationToken's resource_common_data is
// MerchantAuthenticationFlowData, not PaymentFlowData - it carries no payment
// fields. Verify:
//   grep -n 'flow: ClientAuthenticationToken' -A 8 crates/integrations/connector-integration/src/connectors/macros.rs
crate::connectors::macros::macro_connector_flow_status_impls!(
    connector: ${NAME_PASCAL},
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [
        ClientAuthenticationToken,
        PaymentMethodToken,
        GetPaymentMethod
    ],
);
EOF

    log_success "Generated authenticator implementations (AuthenticatorServiceTrait + 3 flow stubs)"
}

# Generate dynamic implementation code for all flows and append to connector file.
#
# The new connector starts with every flow stubbed out via
# `macros::macro_connector_flow_status_impls!` (status: `not_implemented`).
# That macro emits BOTH the marker-trait impl (e.g. `PaymentAuthorizeV2<T>`)
# AND a stub `ConnectorIntegrationV2` impl whose `get_url` returns
# `IntegrationError::connector_flow_not_implemented(...)`. Per-flow `impl`
# blocks are no longer hand-rolled here.
#
# Non-flow base traits (ConnectorServiceTrait, ValidationTrait, IncomingWebhook,
# VerifyRedirectResponse, SourceVerification) are emitted as plain `impl` blocks
# because they are not arms of `expand_flow_status_impl!`. BodyDecoding, the
# other VerifyRedirectResponse supertrait, comes from connector.rs.template.
generate_dynamic_implementations() {
    local connector_file="$1"

    # Non-payment kinds satisfy a DIFFERENT aggregate trait and therefore need a
    # different impl block. Dispatch before emitting anything payment-shaped.
    case "$CONNECTOR_KIND" in
        payout)
            generate_payout_implementations "$connector_file"
            return 0
            ;;
        authenticator)
            generate_authenticator_implementations "$connector_file"
            return 0
            ;;
    esac

    log_step "Generating macro-based dynamic implementations"

    local temp_file="${connector_file}.dynamic"

    cat > "$temp_file" <<EOF

// =============================================================================
// DYNAMICALLY GENERATED IMPLEMENTATIONS
// =============================================================================
// Auto-generated by add_connector.sh using the macro-based pattern. All flow
// traits are stubbed via \`macros::macro_connector_flow_status_impls!\` with
// \`not_implemented\` status, which emits both the marker-trait impl and a stub
// \`ConnectorIntegrationV2\` impl per flow.
//
// To implement a flow:
//   1. Remove that flow's name from the \`not_implemented\` list below.
//   2. Add a manual marker-trait impl, e.g.
//        impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
//            connector_types::PaymentAuthorizeV2<T> for ${NAME_PASCAL}<T> {}
//   3. Add a \`macros::macro_connector_implementation!(...)\` block with the
//      flow's request/response types, HTTP method, and \`get_url\`/\`get_headers\`.
//      For a flow with NO outbound HTTP call (everything decided locally), use
//      \`macros::macro_connector_local_flow_implementation!\` instead - its keys
//      are connector / flow_name / resource_common_data / flow_request /
//      flow_response / handle_response / generic_type / [bounds], and it wires
//      \`CallConnectorAction::HandleResponseWithoutBuildRequest\` for you.
//
// See crates/integrations/connector-integration/src/connectors/xendit.rs for a
// reference implementation that follows this pattern.
// =============================================================================

// ===== CONNECTOR SERVICE TRAIT IMPLEMENTATION =====
// Aggregate trait - composes all other connector traits.
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for ${NAME_PASCAL}<T>
{
}

// ===== BASE (NON-FLOW) TRAIT IMPLEMENTATIONS =====
// These are simple marker traits that are NOT flows and therefore have no arm
// in expand_flow_status_impl!. They must be impl'd manually.
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for ${NAME_PASCAL}<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for ${NAME_PASCAL}<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for ${NAME_PASCAL}<T>
{
}

// ===== SOURCE VERIFICATION IMPLEMENTATION =====
// \`VerifyRedirectResponse: SourceVerification + BodyDecoding\`
// (crates/types-traits/interfaces/src/connector_types.rs), so both supertraits
// must be impl'd or the VerifyRedirectResponse impl above does not compile.
// SourceVerification is emitted here; BodyDecoding already comes from
// template-generation/connector.rs.template - do NOT emit a second one here,
// two impls of the same trait for the same type do not compile.
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    interfaces::verification::SourceVerification for ${NAME_PASCAL}<T>
{
}

// ===== PAYOUT TRAIT IMPLEMENTATIONS =====
// Emits payout marker-trait impls and default no-op ConnectorIntegrationV2
// impls for all PayoutXxxV2 flows.
crate::connectors::macros::macro_connector_payout_implementation!(
    connector: ${NAME_PASCAL},
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);

// ===== FLOW STATUS IMPLEMENTATIONS =====
// Emits marker-trait impls AND stub ConnectorIntegrationV2 impls for every
// flow listed. Each stub's get_url returns
// IntegrationError::connector_flow_not_implemented(...).
crate::connectors::macros::macro_connector_flow_status_impls!(
    connector: ${NAME_PASCAL},
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [
EOF

    # Build the comma-separated list of flow identifiers inside not_implemented.
    # Walk detected ConnectorServiceTrait sub-traits and map each to its flow
    # name. Skip traits with no flow mapping (non-flows + payout).
    local first_flow=true
    local flow flow_name
    for flow in "${AVAILABLE_FLOWS[@]}"; do
        flow_name=$(get_flow_name_for_trait "$flow")
        if [[ -z "$flow_name" ]]; then
            log_debug "Skipping non-flow trait: $flow"
            continue
        fi
        if [[ "$first_flow" == "true" ]]; then
            printf "        %s" "$flow_name" >> "$temp_file"
            first_flow=false
        else
            printf ",\n        %s" "$flow_name" >> "$temp_file"
        fi
    done

    cat >> "$temp_file" <<EOF

    ],
);
EOF

    cat "$temp_file" >> "$connector_file"
    rm -f "$temp_file"

    log_success "Generated macro-based implementations (flow_status_impls + payout + base traits)"
}


# =============================================================================
# FILE UPDATE FUNCTIONS
# =============================================================================

update_protobuf() {
    log_step "Updating protobuf definitions"

    # Check if already exists
    if grep -q "$NAME_UPPER =" "$PROTO_FILE" 2>/dev/null; then
        log_warning "Skipping protobuf update - $NAME_UPPER already exists"
        return 0
    fi

    python3 - "$NAME_UPPER" "$ENUM_ORDINAL" "$PROTO_FILE" <<'PYEOF'
import sys

name = sys.argv[1]
ordinal = sys.argv[2]
path = sys.argv[3]
content = open(path).read()
start = content.index("enum Connector {")
end = content.index("\n}", start)
entry = f"  {name} = {ordinal};\n"
content = content[:end] + "\n" + entry + content[end + 1:]
open(path, "w").write(content)
PYEOF

    log_success "Updated protobuf with $NAME_UPPER = $ENUM_ORDINAL"
}

# Register the connector in domain_types/src/connector_types.rs.
#
# Which edits apply depends on --kind:
#
#   payment / frm  -> ConnectorEnum variant
#                   + grpc_api_types::payments::Connector -> ConnectorEnum mapping
#                   + ConnectorVariant arm  Ok(Self::Payment(ConnectorEnum::X))
#   payout         -> PayoutConnectorEnum variant
#                   + ForeignTryFrom<AuthType> for PayoutConnectorEnum arm
#                   + ConnectorVariant arm  Ok(Self::Payout(PayoutConnectorEnum::X))
#   surcharge      -> same shape against SurchargeConnectorEnum
#   authenticator  -> same shape against AuthenticatorConnectorEnum
#
# A payout/surcharge/authenticator connector gets NO ConnectorEnum variant:
# gotyme_sanlam, santander, deutschebank, interpayments and plaid are all absent
# from `pub enum ConnectorEnum`. Verify:
#   sed -n '/pub enum ConnectorEnum {/,/^}/p' \
#     crates/types-traits/domain_types/src/connector_types.rs | grep -c Plaid   # -> 0
#
# The proto Connector -> ConnectorEnum match already ends in `_ => Err(...)`, so
# skipping that mapping for the non-payment kinds still compiles.
update_domain_types() {
    log_step "Updating domain types ($KIND_ENUM, --kind $CONNECTOR_KIND)"

    # Check if already exists in the target enum
    if grep -q "^[[:space:]]*$NAME_PASCAL,$" "$DOMAIN_TYPES_FILE" 2>/dev/null; then
        log_warning "Skipping domain types update - $NAME_PASCAL already exists"
        return 0
    fi

    python3 - "$NAME_PASCAL" "$DOMAIN_TYPES_FILE" "$KIND_ENUM" "$KIND_VARIANT" "$KIND_IN_CONNECTOR_ENUM" <<'DOMAINEOF'
import sys

name = sys.argv[1]
path = sys.argv[2]
kind_enum = sys.argv[3]
kind_variant = sys.argv[4]
in_connector_enum = sys.argv[5] == "true"
content = open(path).read()


def find_matching_brace(text: str, open_idx: int) -> int:
    depth = 0
    for idx in range(open_idx, len(text)):
        ch = text[idx]
        if ch == "{":
            depth += 1
        elif ch == "}":
            depth -= 1
            if depth == 0:
                return idx
    raise SystemExit("matching brace not found")


# 1) The enum this kind of connector joins.
enum_start = content.index("pub enum %s {" % kind_enum)
enum_open = content.index("{", enum_start)
enum_close = find_matching_brace(content, enum_open)
content = content[:enum_close] + "    %s,\n" % name + content[enum_close:]

if in_connector_enum:
    # 2a) Payment kinds only: proto Connector -> ConnectorEnum.
    grpc_anchor = "            grpc_api_types::payments::Connector::Unspecified =>"
    grpc_entry = "            grpc_api_types::payments::Connector::%s => Ok(Self::%s),\n" % (name, name)
    if grpc_anchor not in content:
        raise SystemExit("Connector enum gRPC mapping anchor not found")
    content = content.replace(grpc_anchor, grpc_entry + grpc_anchor, 1)
else:
    # 2b) Non-payment kinds: ForeignTryFrom<AuthType> for <KindEnum>. That match
    # ends in `_ => Err(error_stack::Report::new(`; insert above the fallback.
    impl_start = content.index("impl ForeignTryFrom<AuthType> for %s {" % kind_enum)
    impl_close = find_matching_brace(content, content.index("{", impl_start))
    fallback = content.index("            _ => Err(error_stack::Report::new(", impl_start)
    if fallback > impl_close:
        raise SystemExit("ForeignTryFrom<AuthType> for %s fallback arm not found" % kind_enum)
    entry = "            AuthType::%s(_) => Ok(Self::%s),\n" % (name, name)
    content = content[:fallback] + entry + content[fallback:]

# 3) ConnectorVariant: every kind gets an arm, wrapped in its own variant.
auth_anchor = "            AuthType::Imerchantsolutions(_) => Ok(Self::Payment(ConnectorEnum::Imerchantsolutions)),"
auth_entry = "            AuthType::%s(_) => Ok(Self::%s(%s::%s)),\n" % (name, kind_variant, kind_enum, name)
if auth_anchor not in content:
    raise SystemExit("AuthType to ConnectorVariant mapping anchor not found")
content = content.replace(auth_anchor, auth_entry + auth_anchor, 1)

open(path, "w").write(content)
DOMAINEOF

    log_success "Updated domain types: $KIND_ENUM::$NAME_PASCAL + ConnectorVariant::$KIND_VARIANT arm"
}

update_domain_types_file() {
    log_step "Updating domain types types.rs file"

    if grep -q "^[[:space:]]*pub $NAME_SNAKE: ConnectorParams," "$DOMAIN_TYPES_TYPES_FILE" 2>/dev/null; then
        log_warning "Skipping types.rs update - $NAME_SNAKE already exists"
        return 0
    fi

    python3 - "$NAME_SNAKE" "$DOMAIN_TYPES_TYPES_FILE" <<'PYEOF'
import sys

name = sys.argv[1]
path = sys.argv[2]
content = open(path).read()
start = content.index("pub struct Connectors {")
end = content.index("\n}", start)
entry = f"    pub {name}: ConnectorParams,\n"
content = content[:end] + "\n" + entry + content[end + 1:]
open(path, "w").write(content)
PYEOF

    log_success "Added $NAME_SNAKE to Connectors struct in types.rs"
}

# Add the URL-patching match arm in domain_types/src/types.rs.
#
# There is one patch fn PER KIND, and each non-payment one is an EXHAUSTIVE match
# over its enum with no `_` fallback - so adding a variant without adding an arm
# here is a hard compile error, not a silent gap:
#
#   patch_connector_urls               ConnectorEnum              (has a `_ => {` fallback)
#   patch_payout_connector_urls        PayoutConnectorEnum        (exhaustive)
#   patch_surcharge_connector_urls     SurchargeConnectorEnum     (exhaustive)
#   patch_frm_connector_urls           FrmConnectorEnum           (exhaustive)
#   patch_authenticator_connector_urls AuthenticatorConnectorEnum (exhaustive)
#
# Verify: grep -n 'pub fn patch_.*_connector_urls' \
#           crates/types-traits/domain_types/src/types.rs
update_domain_types_apply() {
    log_step "Updating $KIND_PATCH_FN match arm in types.rs (dynamic URL patching)"

    if grep -q "$KIND_ENUM::$NAME_PASCAL =>" "$DOMAIN_TYPES_TYPES_FILE" 2>/dev/null; then
        log_warning "Skipping $KIND_PATCH_FN update - $NAME_PASCAL already exists"
        return 0
    fi

    python3 - "$NAME_PASCAL" "$NAME_SNAKE" "$DOMAIN_TYPES_TYPES_FILE" "$KIND_ENUM" "$KIND_PATCH_FN" <<'APPLYEOF'
import sys

pascal = sys.argv[1]
snake = sys.argv[2]
path = sys.argv[3]
kind_enum = sys.argv[4]
patch_fn = sys.argv[5]
content = open(path).read()

if patch_fn == "patch_connector_urls":
    # Payment: this match HAS a `_ => {` fallback that raises
    # "is not supported for dynamic URL patching from superposition".
    # Anchor on that unique string and insert just above the fallback.
    marker = "is not supported for dynamic URL patching from superposition"
    mi = content.index(marker)

    fb = content.rindex("_ => {", 0, mi)
    line_start = content.rindex("\n", 0, fb) + 1
    indent = content[line_start:fb]  # leading whitespace of the `_ => {` line
    arm = (
        "%sConnectorEnum::%s => {\n"
        "%s    patched.%s.apply(params_patch);\n"
        "%s}\n"
    ) % (indent, pascal, indent, snake, indent)
    content = content[:line_start] + arm + content[line_start:]

    # Keep the human-readable "Supported connectors:" hint in sync.
    key = "Supported connectors: "
    ki = content.index(key)
    end = content.index('"', ki)  # closing quote of the string literal
    content = content[:end] + ", " + snake + content[end:]
else:
    # Every other kind: an exhaustive `match connector { ... }` with no fallback.
    # Insert the new arm immediately before the match's closing brace.
    fn_start = content.index("pub fn %s(" % patch_fn)
    match_kw = content.index("match connector {", fn_start)
    brace_open = content.index("{", match_kw + len("match connector"))

    depth = 0
    match_close = -1
    for idx in range(brace_open, len(content)):
        ch = content[idx]
        if ch == "{":
            depth += 1
        elif ch == "}":
            depth -= 1
            if depth == 0:
                match_close = idx
                break
    if match_close == -1:
        raise SystemExit("%s match closing brace not found" % patch_fn)

    line_start = content.rindex("\n", 0, match_close) + 1
    indent = content[line_start:match_close] + "    "
    arm = "%s%s::%s => patched.%s.apply(params_patch),\n" % (indent, kind_enum, pascal, snake)
    content = content[:line_start] + arm + content[line_start:]

open(path, "w").write(content)
APPLYEOF

    log_success "Added $KIND_ENUM::$NAME_PASCAL arm to $KIND_PATCH_FN in types.rs"
}

update_router_data() {
    log_step "Updating router_data.rs (ConnectorSpecificAuth + match arm)"

    # Check if already exists
    if grep -q "ConnectorEnum::$NAME_PASCAL =>" "$ROUTER_DATA_FILE" 2>/dev/null; then
        log_warning "Skipping router_data update - $NAME_PASCAL already exists"
        return 0
    fi

    python3 - "$NAME_PASCAL" "$NAME_SNAKE" "$ROUTER_DATA_FILE" "$KIND_ENUM" "$KIND_VARIANT" <<'PYEOF'
import sys

name = sys.argv[1]
auth_var = sys.argv[2]
path = sys.argv[3]
kind_enum = sys.argv[4]
kind_variant = sys.argv[5]
content = open(path).read()

def find_matching_brace(text: str, open_idx: int) -> int:
    depth = 0
    for idx in range(open_idx, len(text)):
        ch = text[idx]
        if ch == "{":
            depth += 1
        elif ch == "}":
            depth -= 1
            if depth == 0:
                return idx
    raise SystemExit("matching brace not found")

enum_start = content.index("pub enum ConnectorSpecificConfig {")
enum_open = content.index("{", enum_start)
enum_close = find_matching_brace(content, enum_open)
enum_entry = (
    f"    {name} {{\n"
    f"        api_key: Secret<String>,\n"
    f"        base_url: Option<String>,\n"
    f"    }},\n"
)
content = content[:enum_close] + enum_entry + content[enum_close:]

macro_anchor = "            Imerchantsolutions { api_key },"
macro_entry = f"            {name} {{ api_key }},\n"
if content.count(macro_anchor) < 2:
    raise SystemExit("base_url_override/connector_key macro anchor not found twice")
content = content.replace(macro_anchor, macro_entry + macro_anchor, 2)

auth_type_anchor = "            AuthType::Imerchantsolutions(imerchantsolutions) => Ok(Self::Imerchantsolutions {"
auth_type_entry = (
    f"            AuthType::{name}({auth_var}) => Ok(Self::{name} {{\n"
    f"                api_key: {auth_var}.api_key.ok_or_else(err)?,\n"
    f"                base_url: {auth_var}.base_url,\n"
    f"            }}),\n"
)
if auth_type_anchor not in content:
    raise SystemExit("ConnectorSpecificConfig gRPC AuthType anchor not found")
content = content.replace(auth_type_anchor, auth_type_entry + auth_type_anchor, 1)

# The legacy ConnectorAuthType -> ConnectorSpecificConfig conversion is split by
# ConnectorVariant. Each non-payment arm is an EXHAUSTIVE `match connector_enum`
# over its own enum, so a new variant needs an arm here or the crate stops
# compiling. Verify the block layout with:
#   grep -n 'ConnectorVariant::\\(Payment\\|Payout\\|Surcharge\\|Frm\\|Authenticator\\)(' \\
#     crates/types-traits/domain_types/src/router_data.rs
if kind_variant == "Payment":
    connector_anchor = "            ConnectorEnum::PinelabsOnline => match auth {"
    connector_entry = (
        f"            ConnectorEnum::{name} => match auth {{\n"
        f"                ConnectorAuthType::HeaderKey {{ api_key }} => Ok(Self::{name} {{\n"
        f"                    api_key: api_key.clone(),\n"
        f"                    base_url: None,\n"
        f"                }}),\n"
        f"                _ => Err(err().into()),\n"
        f"            }},\n"
    )
    if connector_anchor not in content:
        raise SystemExit("ConnectorEnum auth conversion anchor not found")
    content = content.replace(connector_anchor, connector_entry + connector_anchor, 1)
else:
    variant_marker = f"ConnectorVariant::{kind_variant}(connector_enum)"
    vi = content.index(variant_marker)
    match_kw = content.index("match connector_enum {", vi)
    brace_open = content.index("{", match_kw + len("match connector_enum"))
    depth = 0
    match_close = -1
    for idx in range(brace_open, len(content)):
        ch = content[idx]
        if ch == "{":
            depth += 1
        elif ch == "}":
            depth -= 1
            if depth == 0:
                match_close = idx
                break
    if match_close == -1:
        raise SystemExit(f"ConnectorVariant::{kind_variant} match closing brace not found")
    line_start = content.rindex("\n", 0, match_close) + 1
    indent = content[line_start:match_close]
    arm_indent = indent + "    "
    # `fn foreign_try_from` opens with a NARROW local import:
    #   use connector_types::{ConnectorEnum, ConnectorVariant, PayoutConnectorEnum,
    #                         SurchargeConnectorEnum};
    # AuthenticatorConnectorEnum and FrmConnectorEnum are NOT in it, which is why
    # the existing Plaid and Kount arms in this file are written
    # `connector_types::AuthenticatorConnectorEnum::Plaid`. Qualifying every kind
    # is valid regardless (`use crate::{connector_types, ...}` is module-level), so
    # do that rather than tracking which four names the local `use` happens to list.
    #   Verify: sed -n '/for ConnectorSpecificConfig/,/let err = /p' \
    #             crates/types-traits/domain_types/src/router_data.rs
    connector_entry = (
        f"{arm_indent}connector_types::{kind_enum}::{name} => match auth {{\n"
        f"{arm_indent}    ConnectorAuthType::HeaderKey {{ api_key }} => Ok(Self::{name} {{\n"
        f"{arm_indent}        api_key: api_key.clone(),\n"
        f"{arm_indent}        base_url: None,\n"
        f"{arm_indent}    }}),\n"
        f"{arm_indent}    _ => Err(err().into()),\n"
        f"{arm_indent}}},\n"
    )
    content = content[:line_start] + connector_entry + content[line_start:]

open(path, "w").write(content)
PYEOF

    log_success "Updated router_data.rs with $NAME_PASCAL auth variant and match arm"
}

update_protobuf_auth() {
    log_step "Updating protobuf auth definitions"

    # Check if config message already exists
    if grep -q "${NAME_PASCAL}Config" "$PROTO_FILE" 2>/dev/null; then
        log_warning "Skipping protobuf auth update - ${NAME_PASCAL}Config already exists"
        return 0
    fi

    # Write the base-ref snapshot to a temp file so the allocator below can see
    # field numbers that exist upstream but not yet in the local working tree.
    local base_snapshot_file=""
    if [[ -n "$PROTO_BASE_SNAPSHOT" ]]; then
        base_snapshot_file=$(mktemp)
        printf '%s\n' "$PROTO_BASE_SNAPSHOT" > "$base_snapshot_file"
    fi

    local next_field_num
    next_field_num=$(python3 - "$NAME_PASCAL" "$NAME_SNAKE" "$NAME_UPPER" "$ENUM_ORDINAL" "$PROTO_FILE" "$base_snapshot_file" <<'PYEOF'
import re
import sys

name_pascal = sys.argv[1]
name_snake = sys.argv[2]
name_upper = sys.argv[3]
ordinal = sys.argv[4]
path = sys.argv[5]
base_path = sys.argv[6]
content = open(path).read()

message = (
    f"message {name_pascal}Config {{\n"
    f"  SecretString api_key = 1;\n"
    f"  optional string base_url = 50;\n"
    f"}}\n\n"
)
comment = "// ConnectorSpecificConfig message"
if comment not in content:
    raise SystemExit("ConnectorSpecificConfig comment anchor not found")
content = content.replace(comment, message + comment, 1)



def config_field_numbers(text):
    return [int(num) for num in re.findall(r"Config\s+[a-z0-9_]+\s+=\s+(\d+);", text)]


# Allocate against the UNION of the local tree and the base ref. Reading only
# the local tree is how two parallel branches end up claiming the same oneof
# field number; reading only the base ref would ignore an unmerged connector
# that is already present locally.
field_numbers = config_field_numbers(content)
if base_path:
    field_numbers += config_field_numbers(open(base_path).read())
next_field_num = max(field_numbers, default=0) + 1

oneof_close = content.index("\n  }\n}", content.index("message ConnectorSpecificConfig {"))
entry = f"\n    // {name_upper} = {ordinal}\n    {name_pascal}Config {name_snake} = {next_field_num};"
content = content[:oneof_close] + entry + content[oneof_close:]
open(path, "w").write(content)
print(next_field_num)
PYEOF
)

    if [[ -n "$base_snapshot_file" ]]; then
        rm -f "$base_snapshot_file"
    fi

    log_success "Updated protobuf with ${NAME_PASCAL}Config message and oneof entry"
    log_info "Allocated ConnectorSpecificConfig oneof field number: $next_field_num"
    warn_proto_number_race "ConnectorSpecificConfig oneof field number" "$next_field_num" \
        "git show origin/main:$PROTO_FILE_RELPATH | grep -oE 'Config[[:space:]]+[a-z0-9_]+[[:space:]]+=[[:space:]]+[0-9]+;' | grep -oE '[0-9]+;' | tr -d ';' | sort -n | tail -1"
}

update_router_data_grpc_auth() {
    log_step "Updating router_data.rs gRPC AuthType mapping"

    # Check if already exists
    if grep -q "AuthType::$NAME_PASCAL(" "$ROUTER_DATA_FILE" 2>/dev/null; then
        log_warning "Skipping gRPC auth mapping - $NAME_PASCAL already exists"
        return 0
    fi

    python3 - "$NAME_PASCAL" "$NAME_SNAKE" "$ROUTER_DATA_FILE" <<'PYEOF'
import sys

name = sys.argv[1]
var = sys.argv[2]
path = sys.argv[3]
content = open(path).read()
anchor = "            AuthType::Imerchantsolutions(imerchantsolutions) => Ok(Self::Imerchantsolutions {"
entry = (
    f"            AuthType::{name}({var}) => Ok(Self::{name} {{\n"
    f"                api_key: {var}.api_key.ok_or_else(err)?,\n"
    f"                base_url: {var}.base_url,\n"
    f"            }}),\n"
)
if anchor not in content:
    raise SystemExit("ConnectorSpecificConfig gRPC AuthType anchor not found")
content = content.replace(anchor, entry + anchor, 1)
open(path, "w").write(content)
PYEOF

    log_success "Updated router_data.rs with gRPC AuthType::$NAME_PASCAL mapping"
}

# Declare + re-export the new module in this kind's module file.
#
#   payment / frm  -> src/connectors.rs
#   payout         -> src/payout_connectors.rs      (re-exports <Pascal>Payouts)
#   surcharge      -> src/surcharge_connectors.rs
#   authenticator  -> src/authenticator_connectors.rs
#
# Verify the naming convention per directory with:
#   grep -n 'pub use self' \
#     crates/integrations/connector-integration/src/payout_connectors.rs
update_connectors_module() {
    log_step "Updating module file $(basename "$KIND_MODULE_FILE")"

    if grep -q "^pub mod $NAME_SNAKE;" "$KIND_MODULE_FILE" 2>/dev/null; then
        log_warning "Skipping module update - $NAME_SNAKE already declared in $(basename "$KIND_MODULE_FILE")"
        return 0
    fi

    # Add module declaration and use statement
    cat >> "$KIND_MODULE_FILE" << EOF

pub mod $NAME_SNAKE;
pub use self::${NAME_SNAKE}::${KIND_TYPE_NAME};
EOF

    log_success "Registered $NAME_SNAKE ($KIND_TYPE_NAME) in $(basename "$KIND_MODULE_FILE")"
}

# Add the dispatch arm in the kind's ConnectorDataProvider.
#
# connector-integration/src/types.rs holds five of them, one per kind:
#
#   impl<T> ConnectorData<T>          fn convert_connector -> BoxedConnector<T>
#   impl SurchargeConnectorData       fn convert_connector -> BoxedSurchargeConnector
#   impl FrmConnectorData             fn convert_connector -> BoxedFrmConnector
#   impl PayoutConnectorData          fn convert_connector -> BoxedPayoutConnector
#   impl AuthenticatorConnectorData   fn convert_connector -> BoxedAuthenticatorConnector
#
# The four non-payment boxes are NOT generic in T - `BoxedPayoutConnector` is
# `Box<&'static (dyn PayoutServiceTrait + Sync)>` with no type parameter - so a
# generic connector must be monomorphised at the call site with
# `::<domain_types::payment_method_data::DefaultPCIHolder>`, exactly as
# TrustlyPayouts / GotymeSanlamPayouts / Plaid / Kount already are. A
# non-generic connector (surcharge) takes no turbofish at all.
#
# Verify: grep -n 'pub type Boxed' crates/types-traits/interfaces/src/connector_types.rs
#         grep -n 'Box::new(payout_connectors::' \
#           crates/integrations/connector-integration/src/types.rs
update_integration_types() {
    log_step "Updating $KIND_PROVIDER::convert_connector in integration types.rs"

    if grep -q "$KIND_ENUM::$NAME_PASCAL =>" "$INTEGRATION_TYPES_FILE" 2>/dev/null; then
        log_warning "Skipping integration types update - $NAME_PASCAL already exists"
        return 0
    fi

    python3 - "$NAME_PASCAL" "$INTEGRATION_TYPES_FILE" "$KIND_ENUM" "$KIND_PROVIDER" "$KIND_DIR" "$KIND_TYPE_NAME" "$KIND_GENERIC" <<'INTTYPESEOF'
import sys

name = sys.argv[1]
path = sys.argv[2]
kind_enum = sys.argv[3]
kind_provider = sys.argv[4]
kind_dir = sys.argv[5]
type_name = sys.argv[6]
kind_generic = sys.argv[7] == "true"
content = open(path).read()

if kind_provider == "ConnectorData":
    # Payment: locate the FIRST `fn convert_connector` (the one on
    # ConnectorData) so the anchor survives new impls added below it.
    fn_start = content.find("fn convert_connector")
    if fn_start == -1:
        raise SystemExit("fn convert_connector not found")
    boxed = "Box::new(connectors::%s::<T>::new())" % type_name
else:
    # Other kinds: scope the search to that provider's own impl block.
    impl_start = content.find("impl %s {" % kind_provider)
    if impl_start == -1:
        raise SystemExit("impl %s not found" % kind_provider)
    fn_start = content.find("fn convert_connector", impl_start)
    if fn_start == -1:
        raise SystemExit("fn convert_connector not found in impl %s" % kind_provider)
    if kind_generic:
        # BoxedPayoutConnector / BoxedAuthenticatorConnector carry no type
        # parameter, so the connector must be monomorphised here.
        boxed = (
            "Box::new(%s::%s::<domain_types::payment_method_data::DefaultPCIHolder>::new())"
            % (kind_dir, type_name)
        )
    else:
        boxed = "Box::new(%s::%s::new())" % (kind_dir, type_name)

match_kw = content.find("match connector_name {", fn_start)
if match_kw == -1:
    raise SystemExit("match connector_name not found in convert_connector")
brace_open = content.index("{", match_kw + len("match connector_name"))

depth = 0
match_close = -1
for idx in range(brace_open, len(content)):
    ch = content[idx]
    if ch == "{":
        depth += 1
    elif ch == "}":
        depth -= 1
        if depth == 0:
            match_close = idx
            break
if match_close == -1:
    raise SystemExit("convert_connector match closing brace not found")

entry = "            %s::%s => %s,\n" % (kind_enum, name, boxed)
line_start = content.rfind("\n", 0, match_close) + 1
content = content[:line_start] + entry + content[line_start:]
open(path, "w").write(content)
INTTYPESEOF

    log_success "Updated $KIND_PROVIDER with $KIND_ENUM::$NAME_PASCAL -> $KIND_TYPE_NAME"
}

# Register the connector in EVERY `default_impl_*!(...)` invocation list in
# default_implementations.rs.
#
# Why all of them, not just one: each `default_impl_<x>_single!` emits both a
# marker-trait impl (e.g. `RechargeV2`) and a blanket `ConnectorIntegrationV2`
# impl for that flow. `ConnectorServiceTrait` requires every one of those
# marker traits as a supertrait, so a connector missing from ANY of these lists
# fails to satisfy `ConnectorServiceTrait` and the build dies with E0277 at the
# `connector_integration_types` dispatch table - far away from the list that was
# actually missed.
#
# The invocation list is ENUMERATED from the file at run time, never hardcoded:
# a new `default_impl_*!` block added upstream must be picked up automatically,
# which is exactly the failure mode this replaces.
#
# CRITICAL: a list is SKIPPED when the connector implements that flow itself via
# `macro_connector_implementation!(... flow_name: <Flow> ...)`. Registering in
# both places produces two `ConnectorIntegrationV2<Flow, ...>` impls for the
# same type - E0119, conflicting implementations.
update_default_implementations() {
    # default_implementations.rs registers PAYMENT connectors only: every
    # `default_impl_*!` list is a list of ConnectorEnum-backed types in
    # connectors/. Payout, surcharge and authenticator connectors are absent
    # (GotymeSanlam, InterPayments and Plaid produce zero hits) because their
    # service traits do not require the flows those macros blanket-impl.
    #   Verify: grep -c 'Plaid\|InterPayments' \
    #     crates/integrations/connector-integration/src/default_implementations.rs
    if [[ "$KIND_NEEDS_DEFAULT_IMPLS" != "true" ]]; then
        log_info "Skipping default_implementations.rs - not used by --kind $CONNECTOR_KIND"
        return 0
    fi

    log_step "Updating default_implementations.rs"

    local connector_file="$CRATES_INTEGRATIONS/connector-integration/src/connectors/$NAME_SNAKE.rs"

    python3 - "$NAME_PASCAL" "$DEFAULT_IMPL_FILE" "$connector_file" <<'PYEOF'
import re
import sys

name = sys.argv[1]
path = sys.argv[2]
connector_file = sys.argv[3]

content = open(path).read()

try:
    connector_src = open(connector_file).read()
except OSError:
    connector_src = ""

# Flows this connector implements for real. Registering such a flow in a
# default_impl list too would emit a second ConnectorIntegrationV2 impl -> E0119.
implemented_flows = set(re.findall(r"flow_name\s*:\s*([A-Za-z0-9_]+)", connector_src))

# --- Map each `default_impl_<x>` family to the flow its `_single` macro emits.
# Read it out of the macro body rather than keeping a table here, so a renamed
# or newly added flow is picked up without editing this script.
flow_of = {}
for m in re.finditer(r"macro_rules!\s+(default_impl_[a-z0-9_]+?)_single\s*\{", content):
    base = m.group(1)
    tail = content[m.end() : m.end() + 4000]
    fm = re.search(r"ConnectorIntegrationV2<\s*([A-Za-z0-9_]+)\s*,", tail)
    if fm:
        flow_of[base] = fm.group(1)

# --- Enumerate every real invocation. Column-0 anchored on purpose: the
# `macro_rules!` definitions and the `///` doc-comment examples above them are
# both indented or prefixed, so neither is matched here.
invocations = [
    (m.group(1), m.start())
    for m in re.finditer(r"(?m)^(default_impl_[a-z0-9_]+)!\(", content)
]
if not invocations:
    raise SystemExit("no default_impl_*! invocations found - file layout changed")

print(f"  found {len(invocations)} default_impl_*! invocation list(s)")


def matching_paren(text, open_idx):
    depth = 0
    for idx in range(open_idx, len(text)):
        ch = text[idx]
        if ch == "(":
            depth += 1
        elif ch == ")":
            depth -= 1
            if depth == 0:
                return idx
    raise SystemExit("unterminated macro invocation")


def matching_bracket(text, open_idx):
    depth = 0
    for idx in range(open_idx, len(text)):
        ch = text[idx]
        if ch == "[":
            depth += 1
        elif ch == "]":
            depth -= 1
            if depth == 0:
                return idx
    raise SystemExit("unterminated bucket")


registered, skipped, already = [], [], []

# Walk in REVERSE file order so an earlier insertion never shifts a later offset.
for macro, start in sorted(invocations, key=lambda pair: pair[1], reverse=True):
    open_paren = content.index("(", start)
    close_paren = matching_paren(content, open_paren)
    region = content[open_paren : close_paren + 1]

    flow = flow_of.get(macro)
    if flow and flow in implemented_flows:
        skipped.append(
            f"{macro} (connector implements {flow} itself - it must therefore also\n"
            f"          hand-write the marker-trait impl that {macro}_single emits,\n"
            f"          e.g. `impl<T: ...> connector_types::VerifyWebhookSourceV2 for X<T> {{}}`;\n"
            f"          see paypal.rs, which does both. Without it: E0277.)"
        )
        continue

    if re.search(r"(?<![A-Za-z0-9_])" + re.escape(name) + r"(?![A-Za-z0-9_])", region):
        already.append(macro)
        continue

    # Two invocation shapes exist:
    #   bucketed: not_supported: [...] / not_implemented: [...]
    #   flat:     a bare comma-separated ident list
    bucket_rel = region.find("not_implemented:")
    if bucket_rel == -1:
        bucket_rel = region.find("not_supported:")

    if bucket_rel != -1:
        # New connectors land in whichever bucket was found (not_implemented
        # preferred). An audit can later move one into not_supported.
        bucket_abs = open_paren + bucket_rel
        bucket_open = content.index("[", bucket_abs)
        bucket_close = matching_bracket(content, bucket_open)
        before = content[:bucket_close].rstrip()
        suffix = content[bucket_close:]
        separator = "" if before.endswith(",") or before.endswith("[") else ","
        content = before + f"{separator}\n        {name},\n    " + suffix
    else:
        content = (
            content[: open_paren + 1] + f"\n    {name}," + content[open_paren + 1 :]
        )

    registered.append(macro)

open(path, "w").write(content)

for macro in sorted(registered):
    print(f"  registered in {macro}!")
for macro in sorted(already):
    print(f"  already present in {macro}!")
for note in sorted(skipped):
    print(f"  SKIPPED {note}")
PYEOF

    verify_default_implementations
}

# Post-check: every `default_impl_*!(...)` list must now either contain the
# connector or be a list the connector legitimately owns. Fails loudly rather
# than deferring the problem to a 20-minute cargo build.
verify_default_implementations() {
    if [[ "$KIND_NEEDS_DEFAULT_IMPLS" != "true" ]]; then
        return 0
    fi

    log_step "Verifying default_implementations.rs registration"

    local connector_file="$CRATES_INTEGRATIONS/connector-integration/src/connectors/$NAME_SNAKE.rs"
    local missing=""

    missing=$(python3 - "$NAME_PASCAL" "$DEFAULT_IMPL_FILE" "$connector_file" <<'PYEOF'
import re
import sys

name = sys.argv[1]
content = open(sys.argv[2]).read()
try:
    connector_src = open(sys.argv[3]).read()
except OSError:
    connector_src = ""

implemented_flows = set(re.findall(r"flow_name\s*:\s*([A-Za-z0-9_]+)", connector_src))

flow_of = {}
for m in re.finditer(r"macro_rules!\s+(default_impl_[a-z0-9_]+?)_single\s*\{", content):
    base = m.group(1)
    tail = content[m.end() : m.end() + 4000]
    fm = re.search(r"ConnectorIntegrationV2<\s*([A-Za-z0-9_]+)\s*,", tail)
    if fm:
        flow_of[base] = fm.group(1)


def matching_paren(text, open_idx):
    depth = 0
    for idx in range(open_idx, len(text)):
        if text[idx] == "(":
            depth += 1
        elif text[idx] == ")":
            depth -= 1
            if depth == 0:
                return idx
    return len(text) - 1


for m in re.finditer(r"(?m)^(default_impl_[a-z0-9_]+)!\(", content):
    macro = m.group(1)
    open_paren = content.index("(", m.start())
    region = content[open_paren : matching_paren(content, open_paren) + 1]
    flow = flow_of.get(macro)
    if flow and flow in implemented_flows:
        continue
    if not re.search(r"(?<![A-Za-z0-9_])" + re.escape(name) + r"(?![A-Za-z0-9_])", region):
        print(macro)
PYEOF
)

    if [[ -n "$missing" ]]; then
        log_error "$NAME_PASCAL is MISSING from these default_impl_*! lists:"
        while IFS= read -r macro; do
            [[ -n "$macro" ]] && log_error "    $macro!"
        done <<< "$missing"
        log_error "  Each missing list is an E0277 at the connector_integration_types"
        log_error "  dispatch table: ConnectorServiceTrait requires every marker trait"
        log_error "  those macros emit. Add $NAME_PASCAL to each list by hand, or fix"
        log_error "  the anchor in update_default_implementations()."
        fatal_error "default_implementations.rs registration incomplete"
    fi

    log_success "Registered $NAME_PASCAL in every default_impl_*! list that needs it"
}

update_config_file() {
    local config_file="$1"
    local config_name="$2"

    if [[ -f "$config_file" ]]; then
        if grep -q "^$NAME_SNAKE\\.base_url[[:space:]]*=" "$config_file"; then
            log_warning "Skipping $config_name update - $NAME_SNAKE already exists"
            return 0
        fi

        # Check if [connectors] section exists
        if grep -q "^\[connectors\]" "$config_file"; then
            python3 - "$NAME_SNAKE" "$BASE_URL" "$config_file" <<'PYEOF'
import sys

name = sys.argv[1]
base_url = sys.argv[2]
path = sys.argv[3]
lines = open(path).read().splitlines(keepends=True)
for idx, line in enumerate(lines):
    if line.strip() == "[connectors]":
        lines.insert(idx + 1, f'{name}.base_url = "{base_url}"\n')
        break
else:
    raise SystemExit("[connectors] section not found")
open(path, "w").write("".join(lines))
PYEOF
            log_success "Updated $config_name in [connectors] section"
        else
            # Create [connectors] section at the end
            echo "" >> "$config_file"
            echo "[connectors]" >> "$config_file"
            echo "# $NAME_PASCAL connector configuration" >> "$config_file"
            echo "$NAME_SNAKE.base_url = \"$BASE_URL\"" >> "$config_file"
            log_success "Created [connectors] section in $config_name and added configuration"
        fi
    else
        log_warning "$config_name not found, skipping config update"
    fi
}

update_config() {
    log_step "Updating configuration files"

    # Update all environment config files
    update_config_file "$CONFIG_FILE" "development.toml"
    update_config_file "$SANDBOX_CONFIG_FILE" "sandbox.toml"
    update_config_file "$PRODUCTION_CONFIG_FILE" "production.toml"

    log_success "All configuration files updated"
}

update_superposition_config() {
    # Payout, surcharge and FRM connectors have NO superposition entry on HEAD -
    # gotyme_sanlam, santander, deutschebank, interpayments and kount are all
    # absent from config/superposition.toml. `plaid` (authenticator) does have
    # one, so authenticator keeps this step.
    #   Verify: grep -c 'gotyme_sanlam' config/superposition.toml     # -> 0
    #           grep -c 'connector = "plaid"' config/superposition.toml
    if [[ "$KIND_NEEDS_SUPERPOSITION" != "true" ]]; then
        log_info "Skipping superposition.toml - no entry is used by --kind $CONNECTOR_KIND"
        return 0
    fi

    log_step "Updating superposition.toml (connector dimension + base-URL overrides)"

    if [[ ! -f "$SUPERPOSITION_CONFIG_FILE" ]]; then
        log_warning "superposition.toml not found, skipping superposition update"
        return 0
    fi

    python3 - "$NAME_SNAKE" "$NAME_PASCAL" "$BASE_URL" "$PRODUCTION_URL" "$SUPERPOSITION_CONFIG_FILE" <<'PYEOF'
import sys

snake = sys.argv[1]
pascal = sys.argv[2]
sandbox_url = sys.argv[3]
prod_url = sys.argv[4]
path = sys.argv[5]
content = open(path).read()

# 1) Add the connector to the `connector` dimension enum (idempotent).
lines = content.splitlines(keepends=True)
for i, line in enumerate(lines):
    if line.lstrip().startswith("connector = {") and "enum = [" in line:
        if f'"{snake}"' not in line:
            idx = line.rindex("]")  # closing bracket of the enum array
            lines[i] = line[:idx].rstrip() + f', "{snake}"' + line[idx:]
        break
content = "".join(lines)

# 2) Append sandbox (default) + production overrides at EOF (idempotent).
ctx = f'_context_ = {{ connector = "{snake}" }}'
if ctx not in content:
    prefix = "" if content.endswith("\n") else "\n"
    content += (
        f"{prefix}\n"
        f"# {pascal}\n"
        f"[[overrides]]\n"
        f'_context_ = {{ connector = "{snake}" }}\n'
        f'connector_base_url = "{sandbox_url}"\n'
        f"\n"
        f"# {pascal} Production\n"
        f"[[overrides]]\n"
        f'_context_ = {{ connector = "{snake}", environment = "production" }}\n'
        f'connector_base_url = "{prod_url}"\n'
    )

open(path, "w").write(content)
PYEOF

    log_success "Registered $NAME_SNAKE in superposition.toml (enum + sandbox/production overrides)"
}

update_field_probe() {
    # field-probe's `dummy_auth` matches ConnectorEnum EXHAUSTIVELY (no `_` arm),
    # so it needs an arm for every payment connector and for none of the others -
    # payout/surcharge/authenticator connectors are not in ConnectorEnum at all.
    #   Verify: grep -n 'fn dummy_auth' crates/internal/field-probe/src/auth.rs
    if [[ "$KIND_NEEDS_FIELD_PROBE" != "true" ]]; then
        log_info "Skipping field-probe auth.rs - --kind $CONNECTOR_KIND is not in ConnectorEnum"
        return 0
    fi

    log_step "Updating field-probe auth.rs (ConnectorEnum match arm)"

    # Check if already exists
    if grep -q "ConnectorEnum::$NAME_PASCAL =>" "$FIELD_PROBE_FILE" 2>/dev/null; then
        log_warning "Skipping field-probe update - $NAME_PASCAL already exists"
        return 0
    fi

    python3 - "$NAME_PASCAL" "$FIELD_PROBE_FILE" <<'PYEOF'
import re
import sys

name = sys.argv[1]
path = sys.argv[2]
content = open(path).read()
arm = (
    f"        ConnectorEnum::{name} => ConnectorSpecificConfig::{name} {{\n"
    f"            api_key: k(),\n"
    f"            base_url: None,\n"
    f"        }},\n"
)
content, count = re.subn(r"(\n    \}\n\}\s*)$", "\n" + arm + r"\1", content, count=1)
if count != 1:
    raise SystemExit("dummy_auth match closing anchor not found")
open(path, "w").write(content)
PYEOF

    log_success "Updated field-probe auth.rs with $NAME_PASCAL match arm"
}

# =============================================================================
# CERTIFICATION MANIFEST
# =============================================================================

# Generate crates/internal/integration-tests/src/connector_specs/<name>/specs.json.
#
# Why this is not optional: CI runs
#   cargo run --all-features --bin check_connector_specs
# inside the "Compilation Check" job, and that binary exits 1 when a file under
# connectors/ has no matching connector_specs/<name>/ directory (Phase 1,
# connector list parity). Scaffolding a connector without this file therefore
# produces a branch that cannot pass CI, and the certification sweep never runs
# for the connector at all.
#
# .github/scripts/verify-new-connectors.sh additionally rejects a NEW connector
# whose specs.json is missing or whose supported_suites is empty.
#
# IDEMPOTENT AND NON-DESTRUCTIVE. An existing specs.json is MERGED into (union
# of supported_suites, every other key - supported_payment_methods,
# unsupported_scenarios, sync_poll_until_terminal_seconds, request_id_* -
# preserved verbatim), never overwritten. Re-running with nothing new to add
# leaves the file byte-identical.
generate_connector_specs() {
    # check_connector_specs.rs reads exactly ONE directory:
    #   let connectors_src = root.join("crates/integrations/connector-integration/src/connectors");
    # and it FAILS a connector_specs/<name>/ directory that has no matching .rs
    # file in it ("have a connector_specs/ directory but NO integration .rs
    # file"). Writing specs.json for a payout/surcharge/authenticator connector
    # therefore BREAKS CI rather than satisfying it. Only kinds that live in
    # connectors/ (payment, frm) get a manifest.
    #   Verify: grep -n 'let connectors_src' \
    #     crates/internal/integration-tests/src/bin/check_connector_specs.rs
    if [[ "$KIND_NEEDS_SPECS" != "true" ]]; then
        log_info "Skipping connector_specs/ - check_connector_specs only scans src/connectors/,"
        log_info "  and a specs dir with no matching connectors/*.rs file FAILS that check."
        return 0
    fi

    log_step "Generating connector_specs/$NAME_SNAKE/specs.json"

    if [[ ! -d "$CONNECTOR_SPECS_ROOT" ]]; then
        log_warning "connector_specs root not found at $CONNECTOR_SPECS_ROOT - skipping specs.json"
        log_warning "  CI's check_connector_specs will fail until this file exists."
        return 0
    fi

    local specs_dir="$CONNECTOR_SPECS_ROOT/$NAME_SNAKE"
    local specs_file="$specs_dir/specs.json"

    # Split --flows on commas (spaces tolerated) and map each flow to its suite
    # using the same table the Rust checker uses.
    local -a requested_flows=()
    IFS=',' read -r -a requested_flows <<< "${SPEC_FLOWS// /}"

    local -a suites=() unknown_flows=() skipped_flows=()
    local flow suite
    for flow in "${requested_flows[@]}"; do
        if [[ -z "$flow" ]]; then
            continue
        fi
        suite=$(flow_to_suite "$flow")
        if [[ -n "$suite" ]]; then
            # De-duplicate: two flows never share a suite today, but --flows is
            # operator input.
            if [[ " ${suites[*]-} " != *" $suite "* ]]; then
                suites+=("$suite")
            fi
        elif is_out_of_scope_flow "$flow"; then
            skipped_flows+=("$flow")
        else
            unknown_flows+=("$flow")
        fi
    done

    if [[ ${#unknown_flows[@]} -gt 0 ]]; then
        log_error "Unrecognised flow(s) in --flows: ${unknown_flows[*]}"
        log_error "  A flow that is in neither flow_to_suites nor OUT_OF_SCOPE_FLOWS fails"
        log_error "  check_connector_specs as \"unknown\". Fix the spelling, or triage the flow"
        log_error "  in crates/internal/integration-tests/src/bin/check_connector_specs.rs first."
        log_error "  --flows takes check_connector_specs flow names (Authorize, PSync, Capture,"
        log_error "  Void, Refund, RSync, SetupMandate, RepeatPayment, ...), NOT the trait names"
        log_error "  '$0 --list-flows' prints - those are a different vocabulary."
        fatal_error "Refusing to write a specs.json that CI would reject"
    fi

    if [[ ${#skipped_flows[@]} -gt 0 ]]; then
        log_warning "Flow(s) with no integration-test suite (OUT_OF_SCOPE_FLOWS in"
        log_warning "  check_connector_specs.rs), omitted from specs.json: ${skipped_flows[*]}"
    fi

    if [[ ${#suites[@]} -eq 0 ]]; then
        log_warning "No requested flow maps to a suite - supported_suites would be empty."
        log_warning "  verify-new-connectors.sh rejects a NEW connector with empty supported_suites."
    fi

    if [[ ! -d "$specs_dir" ]]; then
        mkdir -p "$specs_dir"
        SPECS_DIR_CREATED=true
        log_debug "Created specs directory: $specs_dir"
    fi

    local result
    result=$(python3 - "$NAME_SNAKE" "$specs_file" "${suites[@]}" <<'PYEOF'
import json
import os
import sys

connector = sys.argv[1]
path = sys.argv[2]
new_suites = sys.argv[3:]

if os.path.exists(path):
    # Merge. A hand-tuned specs.json carries fields this script knows nothing
    # about (unsupported_scenarios, supported_payment_methods, ...), and its
    # supported_suites may have been trimmed deliberately after a live run.
    # Only ever ADD suites; never remove, reorder, or drop a key.
    with open(path) as fh:
        specs = json.load(fh)

    merged = list(specs.get("supported_suites", []))
    added = [s for s in new_suites if s not in merged]
    if not added:
        print("UNCHANGED")
        raise SystemExit(0)

    merged.extend(added)
    specs["supported_suites"] = merged
    specs.setdefault("connector", connector)
    print("MERGED " + " ".join(added))
else:
    specs = {"connector": connector, "supported_suites": list(new_suites)}
    print("CREATED")

with open(path, "w") as fh:
    json.dump(specs, fh, indent=2)
    fh.write("\n")
PYEOF
)

    case "$result" in
        UNCHANGED)
            log_success "specs.json already covers every requested suite - left untouched"
            ;;
        CREATED)
            log_success "Created $specs_file (${#suites[@]} suite(s))"
            ;;
        MERGED*)
            log_success "Merged into the existing $specs_file - added:${result#MERGED}"
            ;;
        *)
            log_warning "Unexpected specs.json result: $result"
            ;;
    esac

    if [[ ${#suites[@]} -gt 0 ]]; then
        log_info "supported_suites: ${suites[*]}"
    fi

    # EventService/HandleEvent carries a second, separate requirement (Phase 2b in
    # check_connector_specs). No flow maps to that suite, so it can only get into
    # the file by hand - which is exactly why this reads the RESULTING file rather
    # than the suites this run derived.
    if grep -q '"EventService/HandleEvent"' "$specs_file" 2>/dev/null; then
        log_warning "specs.json declares EventService/HandleEvent, so check_connector_specs"
        log_warning "  also requires $specs_dir/webhook_payload.json."
        log_warning "  This script does NOT fabricate that fixture - add a real captured payload."
    fi

    # specs.json is a claim about what works, not a scaffold artifact.
    log_warning "specs.json is a CERTIFICATION CLAIM, not boilerplate. Before pushing:"
    log_warning "  - trim it to the suites the connector ACTUALLY implements;"
    log_warning "    check_connector_specs reads the flows straight out of $NAME_SNAKE.rs, and"
    log_warning "    .github/scripts/verify-new-connectors.sh runs every scenario of every"
    log_warning "    declared suite against the live sandbox."
    log_warning "  - run 'cargo run --bin check_connector_specs' and expect 'All checks passed'."

    # alpha_connectors.json is deliberately NOT touched. See the note below.
    log_info "NOT modified by this script: $CONNECTOR_SPECS_ROOT/alpha_connectors.json"
    log_info "  Add \"$NAME_SNAKE\": { \"reason\": \"...\" } under its top-level \"connectors\""
    log_info "  object BY HAND when there are no CI"
    log_info "  sandbox credentials - verify-new-connectors.sh exits 1 on a bare {} entry,"
    log_info "  it requires a non-empty reason, and it posts a public"
    log_info "  'merging without live sandbox proof' comment on the PR."
    log_info "  Left alone on purpose: CI reads REMOVAL of a name from that file as a"
    log_info "  promotion and pulls the connector into the certification sweep, so editing"
    log_info "  it has consequences well beyond scaffolding."
}

# =============================================================================
# VALIDATION AND CLEANUP
# =============================================================================

format_code() {
    log_step "Formatting code"

    if command -v cargo >/dev/null 2>&1; then
        if (cd "$ROOT_DIR" && cargo +nightly fmt --all >/dev/null 2>&1); then
            log_success "Code formatted with nightly rustfmt"
        elif (cd "$ROOT_DIR" && cargo fmt --all >/dev/null 2>&1); then
            log_success "Code formatted with stable rustfmt"
        else
            log_warning "Code formatting failed"
        fi
    else
        log_warning "Cargo not found, skipping code formatting"
    fi
}

validate_compilation() {
    log_step "Validating compilation"

    if command -v cargo >/dev/null 2>&1; then
        log_info "Running cargo check..."

        if (cd "$ROOT_DIR" && cargo check --package connector-integration 2>&1); then
            log_success "Compilation validation passed"
            return 0
        else
            log_error "Compilation validation failed"
            return 1
        fi
    else
        log_warning "Cargo not found, skipping compilation validation"
        return 0
    fi
}

cleanup_backup() {
    if [[ -n "$BACKUP_DIR" ]] && [[ -d "$BACKUP_DIR" ]]; then
        rm -rf "$BACKUP_DIR"
        log_debug "Cleaned up backup directory"
    fi
}

emergency_rollback() {
    log_step "Performing emergency rollback"

    if [[ -n "$BACKUP_DIR" ]] && [[ -d "$BACKUP_DIR" ]]; then
        # Remove created files (from THIS kind's directory)
        rm -f "$CRATES_INTEGRATIONS/connector-integration/src/$KIND_DIR/$NAME_SNAKE.rs"
        rm -rf "$CRATES_INTEGRATIONS/connector-integration/src/$KIND_DIR/$NAME_SNAKE"

        # Only remove the specs directory if THIS run created it; a pre-existing
        # hand-tuned one is restored from backup below instead.
        if [[ "$SPECS_DIR_CREATED" == "true" ]]; then
            rm -rf "$CONNECTOR_SPECS_ROOT/$NAME_SNAKE"
        fi

        # Restore backed up files
        local backup_file
        for backup_file in "$BACKUP_DIR"/*; do
            if [[ -f "$backup_file" ]]; then
                local filename
                filename=$(basename "$backup_file")
                case "$filename" in
                    "payment.proto")
                        cp "$backup_file" "$PROTO_FILE"
                        ;;
                    "connector_types.rs")
                        cp "$backup_file" "$DOMAIN_TYPES_FILE"
                        ;;
                    "domain_types_types.rs")
                        cp "$backup_file" "$DOMAIN_TYPES_TYPES_FILE"
                        ;;
                    "integration_types.rs")
                        cp "$backup_file" "$INTEGRATION_TYPES_FILE"
                        ;;
                    "default_implementations.rs")
                        cp "$backup_file" "$DEFAULT_IMPL_FILE"
                        ;;
                    "router_data.rs")
                        cp "$backup_file" "$ROUTER_DATA_FILE"
                        ;;
                    "field_probe_auth.rs")
                        cp "$backup_file" "$FIELD_PROBE_FILE"
                        ;;
                    "connectors.rs")
                        cp "$backup_file" "$CONNECTORS_MODULE_FILE"
                        ;;
                    "payout_connectors.rs" | "surcharge_connectors.rs" | "authenticator_connectors.rs")
                        cp "$backup_file" "$CRATES_INTEGRATIONS/connector-integration/src/$filename"
                        ;;
                    "development.toml")
                        cp "$backup_file" "$CONFIG_FILE"
                        ;;
                    "sandbox.toml")
                        cp "$backup_file" "$SANDBOX_CONFIG_FILE"
                        ;;
                    "production.toml")
                        cp "$backup_file" "$PRODUCTION_CONFIG_FILE"
                        ;;
                    "superposition.toml")
                        cp "$backup_file" "$SUPERPOSITION_CONFIG_FILE"
                        ;;
                    "connector_specs_specs.json")
                        mkdir -p "$CONNECTOR_SPECS_ROOT/$NAME_SNAKE"
                        cp "$backup_file" "$CONNECTOR_SPECS_ROOT/$NAME_SNAKE/specs.json"
                        ;;
                esac
            fi
        done

        rm -rf "$BACKUP_DIR"
        log_success "Emergency rollback completed"
    else
        log_warning "No backup found for rollback"
    fi
}

# =============================================================================
# USER INTERACTION
# =============================================================================

show_implementation_plan() {
    if [[ "$YES_MODE" == "true" ]]; then
        return 0
    fi

    echo
    log_step "Implementation Plan"
    echo "====================="
    echo
    echo "📁 Files to create:"
    echo "   ├── crates/integrations/connector-integration/src/$KIND_DIR/$NAME_SNAKE.rs"
    echo "   ├── crates/integrations/connector-integration/src/$KIND_DIR/$NAME_SNAKE/transformers.rs"
    if [[ "$KIND_NEEDS_SPECS" == "true" ]]; then
        echo "   └── crates/internal/integration-tests/src/connector_specs/$NAME_SNAKE/specs.json"
    else
        echo "   └── (no connector_specs/ manifest - check_connector_specs only scans src/connectors/)"
    fi
    echo
    echo "📝 Files to modify:"
    echo "   ├── crates/types-traits/grpc-api-types/proto/payment.proto"
    echo "   ├── crates/types-traits/domain_types/src/connector_types.rs   ($KIND_ENUM, ConnectorVariant::$KIND_VARIANT)"
    echo "   ├── crates/types-traits/domain_types/src/types.rs             (Connectors struct, $KIND_PATCH_FN)"
    echo "   ├── crates/types-traits/domain_types/src/router_data.rs       (ConnectorSpecificConfig)"
    echo "   ├── crates/integrations/connector-integration/src/${KIND_DIR}.rs"
    echo "   ├── crates/integrations/connector-integration/src/types.rs    ($KIND_PROVIDER)"
    [[ "$KIND_NEEDS_DEFAULT_IMPLS" == "true" ]] && echo "   ├── crates/integrations/connector-integration/src/default_implementations.rs"
    [[ "$KIND_NEEDS_FIELD_PROBE" == "true" ]] && echo "   ├── crates/internal/field-probe/src/auth.rs"
    [[ "$KIND_NEEDS_SUPERPOSITION" == "true" ]] && echo "   ├── config/superposition.toml"
    echo "   └── config/development.toml, sandbox.toml, production.toml"
    echo
    echo "🎯 Configuration:"
    echo "   ├── Kind: $CONNECTOR_KIND"
    echo "   ├── Connector: $KIND_TYPE_NAME (in src/$KIND_DIR/)"
    echo "   ├── Service trait: $KIND_SERVICE_TRAIT"
    echo "   ├── Enum: $KIND_ENUM::$NAME_PASCAL"
    echo "   ├── Enum ordinal: $ENUM_ORDINAL (allocated against $PROTO_BASE_REF)"
    echo "   ├── Base URL: $BASE_URL"
    if [[ "$KIND_NEEDS_SPECS" == "true" ]]; then
        echo "   ├── Flows: ${SELECTED_FLOWS[*]}"
        echo "   └── specs.json flows: $SPEC_FLOWS"
    else
        echo "   └── Flows: stubbed by this kind's own macro set (no specs.json)"
    fi
    echo

    read -p "❓ Proceed with implementation? [y/N]: " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        log_error "Implementation cancelled by user"
        exit 1
    fi
}

# Manual completion checklist for the non-payment kinds.
#
# Deliberately a PRINTED checklist rather than more automation: every item below
# is a judgement call about the connector's real auth shape or flow set, and a
# half-correct edit to an exhaustive match is worse than no edit. Each item names
# the file and the SYMBOL to anchor on - no line numbers, they rot.
show_kind_checklist() {
    [[ "$KIND_HAS_CHECKLIST" != "true" ]] && return 0

    echo
    log_step "MANUAL COMPLETION CHECKLIST (--kind $CONNECTOR_KIND)"
    echo "==================================================="
    echo
    echo "The generated auth shape is a PLACEHOLDER for every kind:"
    echo "  ConnectorSpecificConfig::$NAME_PASCAL { api_key, base_url } built from"
    echo "  ConnectorAuthType::HeaderKey. If the vendor needs BodyKey / SignatureKey"
    echo "  or extra fields, change all four places together:"
    echo "    • domain_types/src/router_data.rs  enum ConnectorSpecificConfig"
    echo "      + both macro lists (extract_base_url!, connector_key!)"
    echo "      + the AuthType::$NAME_PASCAL arm"
    echo "      + the $KIND_ENUM::$NAME_PASCAL arm"
    echo "    • grpc-api-types/proto/payment.proto  message ${NAME_PASCAL}Config"
    echo "    • src/$KIND_DIR/$NAME_SNAKE/transformers.rs  ${NAME_PASCAL}AuthType"
    echo "    • src/$KIND_DIR/$NAME_SNAKE.rs  ConnectorCommon::get_auth_header"
    echo

    case "$CONNECTOR_KIND" in
        payout)
            echo "PAYOUT-SPECIFIC:"
            echo "  1. Implement real flows. Replace the bare"
            echo "     macro_connector_payout_implementation!(connector: $KIND_TYPE_NAME, ...) call"
            echo "     with an explicit payout_flows: [...] list naming ONLY the flows that stay"
            echo "     stubbed, then add macros::create_all_prerequisites! and"
            echo "     macros::macro_connector_implementation! blocks for the real ones."
            echo "     Exemplar: payout_connectors/gotyme_sanlam.rs (PayoutTransfer + PayoutGet real,"
            echo "     the other seven still listed in payout_flows)."
            echo
            echo "  2. Do NOT hand-write a PayoutEligibility stub. expand_payout_implementation!"
            echo "     gained a PayoutEligibility arm in commit 3df5eb702, and the default arm of"
            echo "     macro_connector_payout_implementation! already lists it. Two impls of"
            echo "     ConnectorIntegrationV2<PayoutEligibility, ...> for one type is E0119."
            echo "     (payout_connectors/trustly.rs still carries a comment saying the arm does"
            echo "     not exist - that comment predates 3df5eb702 and is stale.)"
            echo "     Verify: grep -n 'flow: PayoutEligibility' \\"
            echo "               crates/integrations/connector-integration/src/connectors/macros.rs"
            echo
            echo "  3. NOTHING to add in superposition.toml, connector_specs/, field-probe or"
            echo "     default_implementations.rs. That is not an omission:"
            echo "       • check_connector_specs.rs scans only src/connectors/, and IGNORE_SERVICES"
            echo "         already contains \"PayoutService\";"
            echo "       • PayoutServiceTrait requires none of the flows the default_impl_*!"
            echo "         macros blanket-impl (no ValidationTrait, no IncomingWebhook, no"
            echo "         VerifyRedirectResponse);"
            echo "       • dummy_auth in field-probe matches ConnectorEnum, which this connector"
            echo "         is not in."
            echo
            echo "  4. Only if this connector ALSO exists as a payment connector: add an arm to"
            echo "     impl TryFrom<ConnectorEnum> for PayoutConnectorEnum in"
            echo "     domain_types/src/connector_types.rs (that is how Trustly and Cybersource"
            echo "     are reachable from PayoutConnectorData::from_connector_variant)."
            echo
            echo "  5. Reference payout connectors are payout_connectors/{trustly,gotyme_sanlam}.rs."
            echo "     connectors/itaubank.rs has had NO payout code since payouts moved out of"
            echo "     connectors/ - do not copy it."
            ;;
        surcharge)
            echo "SURCHARGE-SPECIFIC:"
            echo "  1. The generated file is NON-GENERIC and MACRO-FREE by design. Do not add"
            echo "     <T: PaymentMethodDataTypes> and do not reach for crate::connectors::macros:"
            echo "     every connector macro expands to impl<T: ...> ... for \$connector<T> and"
            echo "     cannot apply to a unit struct. This is the one place GRACE's"
            echo "     \"always use the macros\" rule does not hold."
            echo "     Exemplar: surcharge_connectors/interpayments.rs (0 macro_connector_* uses)."
            echo
            echo "  2. Fill in the three ConnectorIntegrationV2 blocks - SurchargeCalculate,"
            echo "     SurchargePaymentSucceeded, SurchargeRefundSucceeded - all over"
            echo "     SurchargeFlowData. They currently return connector_flow_not_implemented."
            echo
            echo "  3. Nothing to add in connector_specs/, superposition.toml, field-probe or"
            echo "     default_implementations.rs."
            ;;
        authenticator)
            echo "AUTHENTICATOR-SPECIFIC:"
            echo "  1. THIS DIRECTORY IS NOT 3D SECURE. authenticator_connectors/ is bank-account"
            echo "     linking and identity (plaid). If you meant 3DS you want --kind payment plus"
            echo "     the PreAuthenticate / Authenticate / PostAuthenticate trio, whose"
            echo "     resource_common_data is PaymentFlowData, and you must also override"
            echo "     ValidationTrait::next_authentication_step or the composite authorize loop"
            echo "     in crates/internal/composite-service/src/payments.rs never dispatches them."
            echo
            echo "  2. Implement ClientAuthenticationToken for real. Its resource_common_data is"
            echo "     MerchantAuthenticationFlowData (NOT PaymentFlowData) and its response is"
            echo "     PaymentsResponseData - an asymmetric binding. Exemplar:"
            echo "     authenticator_connectors/plaid.rs."
            echo
            echo "  3. If the connector returns a connector-specific client token payload, add a"
            echo "     variant to enum ClientAuthenticationTokenData in"
            echo "     domain_types/src/connector_types.rs and the matching proto response"
            echo "     message, the way Plaid(Box<PlaidClientAuthenticationResponse>) does."
            ;;
        frm)
            echo "FRM-SPECIFIC:"
            echo "  FRM has no directory of its own. What ran above scaffolded the PAYMENT half"
            echo "  (connectors/$NAME_SNAKE.rs, ConnectorEnum, default_implementations, field-probe,"
            echo "  connector_specs) exactly as connectors/kount.rs has it. The FRM half is"
            echo "  hand-written because expand_flow_status_impl! has NO arms for FRM flows:"
            echo
            echo "  1. domain_types/src/connector_types.rs"
            echo "       • add $NAME_PASCAL to pub enum FrmConnectorEnum"
            echo "       • add AuthType::$NAME_PASCAL(_) => Ok(Self::$NAME_PASCAL) to"
            echo "         impl ForeignTryFrom<AuthType> for FrmConnectorEnum"
            echo "       • decide which ConnectorVariant this connector reports. The arm written"
            echo "         above says Ok(Self::Payment(ConnectorEnum::$NAME_PASCAL)); change it to"
            echo "         Ok(Self::Frm(FrmConnectorEnum::$NAME_PASCAL)) if it is FRM-first."
            echo "  2. domain_types/src/types.rs -> patch_frm_connector_urls (exhaustive match)"
            echo "  3. domain_types/src/router_data.rs -> the ConnectorVariant::Frm(connector_enum)"
            echo "     block (exhaustive match)"
            echo "  4. connector-integration/src/types.rs -> FrmConnectorData::convert_connector"
            echo "  5. connectors/$NAME_SNAKE.rs:"
            echo "       • impl connector_types::FrmServiceTrait for ${NAME_PASCAL}<DefaultPCIHolder>"
            echo "         NOT generic in T: FrmServiceTrait also requires"
            echo "         PaymentPreAuthenticateV2<DefaultPCIHolder>, and DefaultPCIHolder is the"
            echo "         only monomorphisation FrmConnectorData ever constructs."
            echo "       • hand-write the five FRM marker impls (PreRiskCheckV2, PostRiskCheckV2,"
            echo "         FrmPaymentOutcomeV2, FrmRefundProcessedV2, FrmChargebackReceivedV2)"
            echo "         and stub the flows with macros::frm_flow_not_implemented! - that macro"
            echo "         emits ONLY the ConnectorIntegrationV2 impl, never the marker trait."
            echo "     Exemplar for all of the above: connectors/kount.rs."
            ;;
    esac
    echo
}

show_next_steps() {
    echo
    log_success "Connector '$NAME_SNAKE' successfully created!"
    echo
    log_step "Next Steps"
    echo "============"
    echo
    echo "1️⃣  Implement Core Logic:"
    echo "   📁 Edit: crates/integrations/connector-integration/src/$KIND_DIR/$NAME_SNAKE/transformers.rs"
    echo "      • Update request/response structures for your API"
    echo "      • Implement proper field mappings"
    echo "      • Handle authentication requirements"
    echo
    echo "2️⃣  Customize Connector:"
    echo "   📁 Edit: crates/integrations/connector-integration/src/$KIND_DIR/$NAME_SNAKE.rs"
    echo "      • Update URL patterns and endpoints"
    echo "      • Implement error handling"
    echo "      • Add connector-specific logic"
    echo
    if [[ "$KIND_NEEDS_SPECS" == "true" ]]; then
        echo "3️⃣  Certification Manifest (MERGE-BLOCKING since 2026-08-31, commit 75079740f):"
        echo "   📁 Edit: crates/internal/integration-tests/src/connector_specs/$NAME_SNAKE/specs.json"
        echo "      • Trim supported_suites to what the connector actually implements"
        echo "      • Add an alpha_connectors.json entry with a non-empty \"reason\" by hand"
        echo "        if there are no CI sandbox credentials for this connector"
        echo
        echo "      .github/scripts/verify-new-connectors.sh fires for ANY connector whose spec"
        echo "      directory did not exist at the merge base, and HARD-FAILS on:"
        echo "        • CONNECTOR_AUTH_FILE_PATH unset"
        echo "        • no specs.json"
        echo "        • empty supported_suites"
        echo "        • an alpha_connectors.json entry with no reason"
        echo "        • off-alpha with no CI credentials"
        echo "        • any declared scenario failing"
        echo "      A declared suite you cannot run is a merge blocker, not a TODO."
        echo
        echo "      \"Out of scope\" in is_out_of_scope_flow() means EXEMPT FROM THIS CHECK, not"
        echo "      \"do not build\". check_connector_specs.rs: \"Each is a coverage gap, not a"
        echo "      decision that it should never be covered.\""
        echo
    else
        echo "3️⃣  Certification Manifest: NOT APPLICABLE to --kind $CONNECTOR_KIND."
        echo "      check_connector_specs.rs scans only src/connectors/, and a"
        echo "      connector_specs/<name>/ directory with no matching connectors/*.rs file"
        echo "      FAILS that check. Do not create one."
        echo
    fi
    echo "4️⃣  Re-verify the proto numbers before opening the PR:"
    echo "   📋 git fetch origin main"
    echo "   📋 git show $PROTO_BASE_REF:$PROTO_FILE_RELPATH | sed -n '/^enum Connector {/,/^}/p' | grep -oE '= [0-9]+;' | grep -oE '[0-9]+' | sort -n | tail -1"
    echo "      • If that is >= $ENUM_ORDINAL, someone merged first - renumber."
    echo
    echo "5️⃣  Validation Commands:"
    echo "   📋 Check compilation: cargo check --package connector-integration"
    echo "   📋 Run tests: cargo test --package connector-integration"
    echo "   📋 Build: cargo build --package connector-integration"
    if [[ "$KIND_NEEDS_SPECS" == "true" ]]; then
        echo "   📋 Certification manifest: cargo run --all-features --bin check_connector_specs"
        echo "   📋 New-connector gate:     bash .github/scripts/verify-new-connectors.sh"
    fi
    echo

    show_kind_checklist

    log_success "Connector '$KIND_TYPE_NAME' is ready for implementation!"
}

# =============================================================================
# MAIN EXECUTION FLOW
# =============================================================================

main() {
    # Print header
    echo "$SCRIPT_NAME v$SCRIPT_VERSION"
    echo "======================================="
    echo

    # Set up error handling
    trap 'emergency_rollback; exit 1' ERR

    # Core execution flow
    parse_arguments "$@"
    validate_environment
    validate_inputs
    check_naming_conflicts
    load_proto_base_snapshot
    get_next_enum_ordinal

    # Show implementation plan and get confirmation
    show_implementation_plan

    # Create backup for safety
    create_backup

    # Execute main operations
    create_connector_files
    update_protobuf
    update_protobuf_auth
    update_domain_types
    update_domain_types_file
    update_domain_types_apply
    update_router_data
    update_router_data_grpc_auth
    update_connectors_module
    update_integration_types
    update_default_implementations
    update_config
    update_superposition_config
    update_field_probe

    # Certification manifest (required by CI's check_connector_specs)
    generate_connector_specs

    # Validate and finalize
    format_code
    if ! validate_compilation; then
        emergency_rollback
        exit 1
    fi

    # Success cleanup and guidance
    cleanup_backup
    show_next_steps
}

# Execute main function with all arguments
main "$@"
