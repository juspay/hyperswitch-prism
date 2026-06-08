#!/usr/bin/env python3
"""
Coverage Summary Generator

Produces an aggregated, at-a-glance summary of connector capability coverage
across ALL flows and ALL payment methods, built entirely from the existing
field-probe output in data/field_probe/*.json (no re-probe).

Unlike docs-generated/all_connector.md (a full matrix), this emits *summary
statistics only*:
  1. Overall coverage (status distribution across every flow x method combo)
  2. Per-flow summary (connector counts by status, for all 32 flows)
  3. Per-payment-method summary (Authorize flow only -- the only PM-aware flow)
  4. Per-connector summary (#flows supported, #methods supported, status counts)

Usage:
    python3 scripts/generators/docs/coverage_summary.py
    python3 scripts/generators/docs/coverage_summary.py --probe-dir DIR --out FILE
"""

import sys
import json
import argparse
from pathlib import Path
from collections import Counter, defaultdict

# Constants mirrored verbatim from the sibling doc generator (generate.py) so the
# summary's flow/payment-method naming stays in sync with all_connector.md. They
# are copied rather than imported because generate.py uses Python 3.10+ syntax
# (`str | None`) and this script must run under the repo's Python 3.9 toolchain.

# Flows that have PM-specific probe results (vs flows with only a 'default' key).
_PM_AWARE_FLOWS = frozenset(["authorize"])

# Payment methods grouped by category, matching generate.py.
# Format: (category_name, [(pm_key, display_name), ...])
_PROBE_PM_BY_CATEGORY = [
    ("Card", [
        ("Card", "Card"),
        ("BancontactCard", "Bancontact"),
    ]),
    ("Wallet", [
        ("ApplePay", "Apple Pay"),
        ("ApplePayDecrypted", "Apple Pay Dec"),
        ("ApplePayThirdPartySdk", "Apple Pay SDK"),
        ("GooglePay", "Google Pay"),
        ("GooglePayDecrypted", "Google Pay Dec"),
        ("GooglePayThirdPartySdk", "Google Pay SDK"),
        ("PaypalSdk", "PayPal SDK"),
        ("AmazonPayRedirect", "Amazon Pay"),
        ("CashappQr", "Cash App"),
        ("PaypalRedirect", "PayPal"),
        ("WeChatPayQr", "WeChat Pay"),
        ("AliPayRedirect", "Alipay"),
        ("RevolutPay", "Revolut Pay"),
        ("Mifinity", "MiFinity"),
        ("Bluecode", "Bluecode"),
        ("Paze", "Paze"),
        ("SamsungPay", "Samsung Pay"),
        ("MbWay", "MB Way"),
        ("Satispay", "Satispay"),
        ("Wero", "Wero"),
        ("GoPay", "GoPay"),
        ("GCash", "GCash"),
        ("Momo", "Momo"),
        ("Dana", "Dana"),
        ("KakaoPay", "Kakao Pay"),
        ("TouchNGo", "Touch 'n Go"),
        ("Twint", "Twint"),
        ("Vipps", "Vipps"),
        ("Swish", "Swish"),
    ]),
    ("BNPL", [
        ("Affirm", "Affirm"),
        ("Afterpay", "Afterpay"),
        ("Klarna", "Klarna"),
    ]),
    ("UPI", [
        ("UpiCollect", "UPI Collect"),
        ("UpiIntent", "UPI Intent"),
        ("UpiQr", "UPI QR"),
    ]),
    ("Online Banking", [
        ("OnlineBankingThailand", "Thailand"),
        ("OnlineBankingCzechRepublic", "Czech"),
        ("OnlineBankingFinland", "Finland"),
        ("OnlineBankingFpx", "FPX"),
        ("OnlineBankingPoland", "Poland"),
        ("OnlineBankingSlovakia", "Slovakia"),
    ]),
    ("Open Banking", [
        ("OpenBankingUk", "UK"),
        ("OpenBankingPis", "PIS"),
        ("OpenBanking", "Generic"),
    ]),
    ("Bank Redirect", [
        ("LocalBankRedirect", "Local"),
        ("Ideal", "iDEAL"),
        ("Sofort", "Sofort"),
        ("Trustly", "Trustly"),
        ("Giropay", "Giropay"),
        ("Eps", "EPS"),
        ("Przelewy24", "Przelewy24"),
        ("Pse", "PSE"),
        ("Blik", "BLIK"),
        ("Interac", "Interac"),
        ("Bizum", "Bizum"),
        ("Eft", "EFT"),
        ("DuitNow", "DuitNow"),
    ]),
    ("Bank Transfer", [
        ("AchBankTransfer", "ACH"),
        ("SepaBankTransfer", "SEPA"),
        ("BacsBankTransfer", "BACS"),
        ("MultibancoBankTransfer", "Multibanco"),
        ("InstantBankTransfer", "Instant"),
        ("InstantBankTransferFinland", "Instant FI"),
        ("InstantBankTransferPoland", "Instant PL"),
        ("Pix", "Pix"),
        ("PermataBankTransfer", "Permata"),
        ("BcaBankTransfer", "BCA"),
        ("BniVaBankTransfer", "BNI VA"),
        ("BriVaBankTransfer", "BRI VA"),
        ("CimbVaBankTransfer", "CIMB VA"),
        ("DanamonVaBankTransfer", "Danamon VA"),
        ("MandiriVaBankTransfer", "Mandiri VA"),
        ("LocalBankTransfer", "Local"),
        ("IndonesianBankTransfer", "Indonesian"),
    ]),
    ("Bank Debit", [
        ("Ach", "ACH"),
        ("Sepa", "SEPA"),
        ("Bacs", "BACS"),
        ("Becs", "BECS"),
        ("SepaGuaranteedDebit", "SEPA Guaranteed"),
    ]),
    ("Alternative", [
        ("Crypto", "Crypto"),
        ("ClassicReward", "Reward"),
        ("Givex", "Givex"),
        ("PaySafeCard", "PaySafeCard"),
        ("EVoucher", "E-Voucher"),
        ("Boleto", "Boleto"),
        ("Efecty", "Efecty"),
        ("PagoEfectivo", "Pago Efectivo"),
        ("RedCompra", "Red Compra"),
        ("RedPagos", "Red Pagos"),
        ("Alfamart", "Alfamart"),
        ("Indomaret", "Indomaret"),
        ("Oxxo", "Oxxo"),
        ("SevenEleven", "7-Eleven"),
        ("Lawson", "Lawson"),
        ("MiniStop", "Mini Stop"),
        ("FamilyMart", "Family Mart"),
        ("Seicomart", "Seicomart"),
        ("PayEasy", "Pay Easy"),
    ]),
]

# Flattened pm_key -> display name, for detecting uncategorized methods.
_PROBE_PM_DISPLAY = {}
for _category, _pms in _PROBE_PM_BY_CATEGORY:
    _PROBE_PM_DISPLAY.update(dict(_pms))

# Repo root: .../hyperswitch-prism (this file lives at scripts/generators/docs/)
REPO_ROOT = Path(__file__).resolve().parents[3]
DEFAULT_PROBE_DIR = REPO_ROOT / "data" / "field_probe"
DEFAULT_OUT = REPO_ROOT / "docs-generated" / "coverage_summary.md"

# Reported status model. The raw probe data has four statuses
# (supported, not_implemented, not_supported, error) but the coverage reports
# collapse them via normalize_status():
#   - error          -> counted as not_implemented (an unfinished integration)
#   - not_supported  -> excluded from coverage entirely (genuinely out of scope)
# leaving two reported buckets, in display order.
STATUSES = ["supported", "not_implemented"]
STATUS_SYMBOL = {
    "supported": "✓",       # checkmark
    "not_implemented": "⚠",  # warning
}


def normalize_status(status):
    """Collapse a raw probe status to the reported model.

    Returns "supported" or "not_implemented", or None for statuses excluded
    from coverage (not_supported, and anything unrecognized).
    """
    if status in ("supported", "not_implemented"):
        return status
    # Legacy: probe output prior to the 3-status model emitted "error"; the probe
    # now folds those cases into "not_implemented" directly, but keep this so the
    # summary still works against older field_probe JSON.
    if status == "error":
        return "not_implemented"
    # not_supported (and any unknown value) is excluded from coverage.
    return None

# Curated flow_key -> "Service.Rpc" display names, matching the gRPC service
# definitions in proto/services.proto. Used so the summary reads the same as
# all_connector.md without depending on grpc_tools.protoc at runtime.
FLOW_DISPLAY = {
    "authorize": "PaymentService.Authorize",
    "get": "PaymentService.Get (PSync)",
    "capture": "PaymentService.Capture",
    "void": "PaymentService.Void",
    "reverse": "PaymentService.Reverse",
    "refund": "PaymentService.Refund",
    "create_order": "PaymentService.CreateOrder",
    "incremental_authorization": "PaymentService.IncrementalAuthorization",
    "verify_redirect": "PaymentService.VerifyRedirectResponse",
    "setup_recurring": "PaymentService.SetupRecurring",
    "proxy_authorize": "PaymentService.ProxyAuthorize",
    "proxy_setup_recurring": "PaymentService.ProxySetupRecurring",
    "token_authorize": "PaymentService.TokenAuthorize",
    "token_setup_recurring": "PaymentService.TokenSetupRecurring",
    "refund_get": "RefundService.Get (RSync)",
    "recurring_charge": "RecurringPaymentService.Charge",
    "recurring_revoke": "RecurringPaymentService.Revoke",
    "create_customer": "CustomerService.Create",
    "tokenize": "PaymentMethodService.Tokenize",
    "eligibility": "PaymentMethodService.Eligibility",
    "create_client_authentication_token": "MerchantAuthenticationService.CreateAccessToken",
    "create_server_authentication_token": "MerchantAuthenticationService.CreateSessionToken",
    "create_server_session_authentication_token": "MerchantAuthenticationService.CreateSdkSessionToken",
    "pre_authenticate": "PaymentMethodAuthenticationService.PreAuthenticate",
    "authenticate": "PaymentMethodAuthenticationService.Authenticate",
    "post_authenticate": "PaymentMethodAuthenticationService.PostAuthenticate",
    "dispute_submit_evidence": "DisputeService.SubmitEvidence",
    "dispute_get": "DisputeService.Get",
    "dispute_defend": "DisputeService.Defend",
    "dispute_accept": "DisputeService.Accept",
    "parse_event": "EventService.ParseEvent",
    "handle_event": "EventService.HandleEvent",
}


def flow_display(flow_key):
    """Friendly Service.Rpc name for a probe flow key (prettified fallback)."""
    return FLOW_DISPLAY.get(flow_key, flow_key.replace("_", " ").title())


def load_probe_files(probe_dir):
    """Load every connector probe JSON, skipping non-connector payload files."""
    connectors = {}
    for fp in sorted(probe_dir.glob("*.json")):
        try:
            data = json.loads(fp.read_text())
        except (json.JSONDecodeError, OSError):
            continue
        # A connector probe file has both a connector name and a flows map.
        name = data.get("connector")
        flows = data.get("flows")
        if not name or not isinstance(flows, dict):
            continue
        connectors[name] = flows
    return connectors


def entry_status(entry):
    """Extract a normalized status from a probe entry, or None if absent."""
    if isinstance(entry, dict):
        return entry.get("status")
    return None


def compute(connectors):
    """Compute all summary aggregates from the loaded probe data."""
    overall = Counter()

    # Per-flow: flow_key -> {status: set(connectors)}; track all flow keys seen.
    flow_status_conns = defaultdict(lambda: defaultdict(set))
    # Authorize: max # of methods a single connector supports.
    authorize_max_methods = 0

    # Per-payment-method (Authorize flow only): pm_key -> Counter(status)
    pm_status = defaultdict(Counter)

    # Per-connector: name -> {flows_supported, methods_supported, status Counter}
    per_connector = {}

    all_flow_keys = set()

    for name, flows in connectors.items():
        flows_supported = 0
        methods_supported = 0
        # API view: one connector-level status per flow.
        flow_counts = Counter()
        # PM view: status counts across Authorize methods for this connector.
        method_counts = Counter()

        for flow_key, methods in flows.items():
            all_flow_keys.add(flow_key)
            if not isinstance(methods, dict):
                continue

            flow_supported_here = False
            for pm_key, entry in methods.items():
                status = normalize_status(entry_status(entry))
                if status is None:
                    continue
                overall[status] += 1
                if status == "supported":
                    flow_supported_here = True

                # Authorize is the only PM-aware flow -> contributes to PM stats.
                if flow_key in _PM_AWARE_FLOWS:
                    pm_status[pm_key][status] += 1
                    method_counts[status] += 1
                    if status == "supported":
                        methods_supported += 1

            # A flow counts as "supported" by a connector if any entry is supported.
            if flow_supported_here:
                bucket = "supported"
                flows_supported += 1
            else:
                # A non-supported flow is "not_implemented" if any probed method
                # normalizes to that bucket (error folds in here). Flows whose
                # methods are all not_supported normalize away and are excluded.
                statuses_here = {
                    normalize_status(entry_status(e))
                    for e in methods.values()
                }
                statuses_here.discard(None)
                bucket = "not_implemented" if "not_implemented" in statuses_here else None
            if bucket:
                flow_status_conns[flow_key][bucket].add(name)
                flow_counts[bucket] += 1

        authorize_max_methods = max(authorize_max_methods, methods_supported)
        per_connector[name] = {
            "flows_supported": flows_supported,
            "methods_supported": methods_supported,
            "flow_counts": flow_counts,
            "method_counts": method_counts,
        }

    return {
        "overall": overall,
        "flow_status_conns": flow_status_conns,
        "all_flow_keys": all_flow_keys,
        "pm_status": pm_status,
        "per_connector": per_connector,
        "n_connectors": len(connectors),
        "authorize_max_methods": authorize_max_methods,
    }


def pct(n, total):
    return f"{(100.0 * n / total):.1f}%" if total else "0.0%"


def _header(lines, title, intro):
    lines.append(f"# {title}")
    lines.append("")
    lines.append("<!--")
    lines.append("This file is auto-generated. Do not edit by hand.")
    lines.append("Source: data/field_probe/")
    lines.append("Regenerate: python3 scripts/generators/docs/coverage_summary.py")
    lines.append("-->")
    lines.append("")
    lines.append(intro)
    lines.append("")
    lines.append(
        "**Legend:** "
        f"{STATUS_SYMBOL['supported']} Supported | "
        f"{STATUS_SYMBOL['not_implemented']} Not Implemented "
        "(includes errors / missing required fields). "
        "Not-supported method/flow combinations are excluded from coverage."
    )
    lines.append("")


def _status_table(lines, counts, total):
    lines.append("| Status | Count | Share |")
    lines.append("|---|---:|---:|")
    for st in STATUSES:
        label = f"{STATUS_SYMBOL[st]} {st.replace('_', ' ').title()}"
        lines.append(f"| {label} | {counts.get(st, 0)} | {pct(counts.get(st, 0), total)} |")
    lines.append(f"| **Total** | **{total}** | **100.0%** |")
    lines.append("")


def render_api(agg):
    """API/flow coverage: unit = connector x flow (one status per flow)."""
    n_conn = agg["n_connectors"]
    flow_keys = agg["all_flow_keys"]
    fsc = agg["flow_status_conns"]
    per_connector = agg["per_connector"]
    n_flows = len(flow_keys)

    # API overall = distribution of connector-flow buckets.
    api_overall = Counter()
    for buckets in fsc.values():
        for st, conns in buckets.items():
            api_overall[st] += len(conns)
    total = sum(api_overall.values())

    out = []
    _header(
        out,
        "API (Flow) Coverage Summary",
        "Coverage of the gRPC API flows across all connectors, derived from "
        "field-probe data. The unit is one connector x flow: each connector gets a "
        "single status per flow (*supported* if any probed payment method succeeded, "
        "otherwise *not implemented*). Not-supported flows are excluded. For "
        "payment-method coverage see [coverage_pm.md](coverage_pm.md); for the full "
        "matrix see [all_connector.md](all_connector.md).",
    )

    out.append("## 1. Overall API Coverage")
    out.append("")
    out.append(f"- **Connectors:** {n_conn}")
    out.append(f"- **Flows (APIs):** {n_flows}")
    out.append(f"- **Total connector x flow units:** {total}")
    out.append("")
    _status_table(out, api_overall, total)

    out.append("## 2. Per-Flow Coverage")
    out.append("")
    out.append(
        "Connector counts per flow, sorted by adoption. The Authorize row notes the "
        "max number of payment methods a single connector supports (its method-level "
        "detail lives in coverage_pm.md)."
    )
    out.append("")
    out.append("| Flow (API) | Supported | Not Impl | PM-aware |")
    out.append("|---|---:|---:|:---:|")

    def flow_sup(fk):
        return len(fsc.get(fk, {}).get("supported", ()))

    for fk in sorted(flow_keys, key=lambda k: (-flow_sup(k), flow_display(k))):
        b = fsc.get(fk, {})
        sup = len(b.get("supported", ()))
        ni = len(b.get("not_implemented", ()))
        name = flow_display(fk)
        pm_flag = ""
        if fk in _PM_AWARE_FLOWS:
            name += f" ({agg['authorize_max_methods']} methods max)"
            pm_flag = "yes"
        out.append(f"| {name} | {sup} | {ni} | {pm_flag} |")
    out.append("")

    out.append("## 3. Per-Connector API Coverage")
    out.append("")
    out.append(
        f"Number of the {n_flows} flows each connector supports, with the breakdown "
        "of the remaining flows by status. Sorted by flows supported."
    )
    out.append("")
    out.append(
        "| Connector | Flows Supported | Not Impl |"
    )
    out.append("|---|---:|---:|")
    for name in sorted(
        per_connector,
        key=lambda n: (-per_connector[n]["flows_supported"], n),
    ):
        fc = per_connector[name]["flow_counts"]
        out.append(
            f"| {name} | {per_connector[name]['flows_supported']}/{n_flows} | "
            f"{fc.get('not_implemented', 0)} |"
        )
    out.append("")
    return "\n".join(out)


def render_pm(agg):
    """Payment-method coverage: unit = connector x method on Authorize."""
    n_conn = agg["n_connectors"]
    pm_status = agg["pm_status"]
    per_connector = agg["per_connector"]

    pm_overall = Counter()
    for c in pm_status.values():
        pm_overall.update(c)
    total = sum(pm_overall.values())

    out = []
    _header(
        out,
        "Payment Method Coverage Summary",
        "Coverage of payment methods across all connectors, derived from field-probe "
        "data. Payment methods are only probed on the **Authorize** flow (the sole "
        "payment-method-aware API), so this report reflects Authorize. The unit is "
        "one connector x payment method. For API/flow coverage see "
        "[coverage_api.md](coverage_api.md); for the full matrix see "
        "[all_connector.md](all_connector.md).",
    )

    out.append("## 1. Overall Payment Method Coverage")
    out.append("")
    out.append(f"- **Connectors:** {n_conn}")
    out.append(f"- **Payment methods probed:** {len(pm_status)}")
    out.append(f"- **Total connector x method units:** {total}")
    out.append("")
    _status_table(out, pm_overall, total)

    out.append("## 2. Per-Payment-Method Coverage")
    out.append("")
    out.append(
        "Number of connectors supporting each payment method, grouped by category. "
        "Methods with zero support are listed for completeness."
    )
    out.append("")
    for category, pms in _PROBE_PM_BY_CATEGORY:
        out.append(f"### {category}")
        out.append("")
        out.append("| Payment Method | Supported | Not Impl |")
        out.append("|---|---:|---:|")
        for pm_key, display in pms:
            c = pm_status.get(pm_key, Counter())
            out.append(
                f"| {display} | {c.get('supported', 0)} | "
                f"{c.get('not_implemented', 0)} |"
            )
        out.append("")

    known = set(_PROBE_PM_DISPLAY.keys())
    extra = sorted(set(pm_status.keys()) - known)
    if extra:
        out.append("### Other / Uncategorized")
        out.append("")
        out.append("| Payment Method | Supported | Not Impl |")
        out.append("|---|---:|---:|")
        for pm_key in extra:
            c = pm_status[pm_key]
            out.append(
                f"| {pm_key} | {c.get('supported', 0)} | "
                f"{c.get('not_implemented', 0)} |"
            )
        out.append("")

    out.append("## 3. Per-Connector Payment Method Coverage")
    out.append("")
    out.append(
        "Payment methods each connector supports on Authorize, with the breakdown of "
        "the remaining methods by status. Sorted by methods supported."
    )
    out.append("")
    out.append(
        "| Connector | Methods Supported | Not Impl |"
    )
    out.append("|---|---:|---:|")
    for name in sorted(
        per_connector,
        key=lambda n: (-per_connector[n]["methods_supported"], n),
    ):
        mc = per_connector[name]["method_counts"]
        out.append(
            f"| {name} | {per_connector[name]['methods_supported']} | "
            f"{mc.get('not_implemented', 0)} |"
        )
    out.append("")
    return "\n".join(out)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--probe-dir", type=Path, default=DEFAULT_PROBE_DIR)
    parser.add_argument("--out-dir", type=Path, default=DEFAULT_OUT.parent)
    args = parser.parse_args()

    connectors = load_probe_files(args.probe_dir)
    if not connectors:
        print(f"No connector probe files found in {args.probe_dir}", file=sys.stderr)
        return 1

    agg = compute(connectors)
    args.out_dir.mkdir(parents=True, exist_ok=True)

    api_out = args.out_dir / "coverage_api.md"
    pm_out = args.out_dir / "coverage_pm.md"
    api_out.write_text(render_api(agg) + "\n")
    pm_out.write_text(render_pm(agg) + "\n")

    print(f"Wrote {api_out}")
    print(f"Wrote {pm_out}")
    print(f"  connectors={agg['n_connectors']} flows={len(agg['all_flow_keys'])}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
