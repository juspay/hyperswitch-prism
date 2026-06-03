#!/usr/bin/env python3
"""Probe-driven backfill of `SupportedPaymentMethods` declarations.

For every connector that currently uses the empty stub
`&EMPTY_SUPPORTED_PAYMENT_METHODS`, read `data/field_probe/<connector>.json`
and emit a populated `LazyLock<SupportedPaymentMethods>` block plus an
updated `impl ConnectorSpecifications` body that references it. Sparse
shape — `not_supported` PMs are *omitted* (the matrix encodes absence as `x`).

Classification rules (Phase A of plan Change 6):

    probe.supported       -> FeatureStatus::Supported
    probe.not_implemented -> FeatureStatus::NotImplemented
    probe.error           -> FeatureStatus::NotImplemented  (provisional;
                                Phase B may downgrade after docs lookup)
    probe.not_supported   -> omitted from declaration

Inner fields the probe can't determine fall to conservative defaults:
`mandates = NotImplemented`, `refunds = NotImplemented`,
`supported_capture_methods = [Automatic]`, `specific_features = None`.
Connector authors refine those in their own follow-up PRs.

Usage:
    python3 scripts/generators/docs/bootstrap_capabilities.py            # dry-run, prints summary
    python3 scripts/generators/docs/bootstrap_capabilities.py --apply    # edits source files in place
    python3 scripts/generators/docs/bootstrap_capabilities.py --apply stripe checkout   # subset
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[3]
PROBE_DIR = REPO_ROOT / "data" / "field_probe"
CONNECTORS_DIR = REPO_ROOT / "crates" / "integrations" / "connector-integration" / "src" / "connectors"

# Connectors that already declare via hand-written LazyLock blocks. Phase A
# skips them — their existing data is the source of truth.
ALREADY_DECLARED = frozenset({
    "adyen", "calida", "cashfree", "easebuzz", "hyperpg",
    "payu", "ppro", "razorpay", "zift",
})

# Probe JSON file stems sometimes drop underscores that the source filename keeps.
# Map the divergent few here.
PROBE_TO_SOURCE_STEM: dict[str, str] = {
    "absasanlam": "absa_sanlam",
    "pinelabsonline": "pinelabs_online",
    "twoctwoppaco": "twoc_twop_paco",
}

# Probe PM key  →  (PaymentMethod variant, PaymentMethodType variant).
# Aligned with `_PROBE_PM_BY_CATEGORY` in generate.py and the Rust enums in
# common_enums/src/enums.rs. Multiple probe keys can map to one (PM, PMType)
# pair (e.g. ApplePay aliases) — duplicates are deduped, taking the strongest
# status across aliases.
PROBE_TO_RUST: dict[str, tuple[str, str]] = {
    # Card
    "Card": ("Card", "Card"),
    "BancontactCard": ("Card", "BancontactCard"),
    # Wallet
    "ApplePay": ("Wallet", "ApplePay"),
    "ApplePayDecrypted": ("Wallet", "ApplePay"),
    "ApplePayThirdPartySdk": ("Wallet", "ApplePay"),
    "GooglePay": ("Wallet", "GooglePay"),
    "GooglePayDecrypted": ("Wallet", "GooglePay"),
    "GooglePayThirdPartySdk": ("Wallet", "GooglePay"),
    "PaypalSdk": ("Wallet", "Paypal"),
    "AmazonPayRedirect": ("Wallet", "AmazonPay"),
    "CashappQr": ("Wallet", "Cashapp"),
    "PaypalRedirect": ("Wallet", "Paypal"),
    "WeChatPayQr": ("Wallet", "WeChatPay"),
    "AliPayRedirect": ("Wallet", "AliPay"),
    "RevolutPay": ("Wallet", "RevolutPay"),
    "Mifinity": ("Wallet", "Mifinity"),
    "Bluecode": ("Wallet", "Bluecode"),
    "Paze": ("Wallet", "Paze"),
    "SamsungPay": ("Wallet", "SamsungPay"),
    "MbWay": ("Wallet", "MbWay"),
    "Satispay": ("Wallet", "Satispay"),
    "Wero": ("Wallet", "Wero"),
    "GoPay": ("Wallet", "GoPay"),
    "GCash": ("Wallet", "Gcash"),
    "Momo": ("Wallet", "Momo"),
    "Dana": ("Wallet", "Dana"),
    "KakaoPay": ("Wallet", "KakaoPay"),
    "TouchNGo": ("Wallet", "TouchNGo"),
    "Twint": ("Wallet", "Twint"),
    "Vipps": ("Wallet", "Vipps"),
    "Swish": ("Wallet", "Swish"),
    # PayLater / BNPL
    "Affirm": ("PayLater", "Affirm"),
    "Afterpay": ("PayLater", "AfterpayClearpay"),
    "Klarna": ("PayLater", "Klarna"),
    # UPI
    "UpiCollect": ("Upi", "UpiCollect"),
    "UpiIntent": ("Upi", "UpiIntent"),
    "UpiQr": ("Upi", "UpiQr"),
    # Online Banking
    "OnlineBankingThailand": ("BankRedirect", "OnlineBankingThailand"),
    "OnlineBankingCzechRepublic": ("BankRedirect", "OnlineBankingCzechRepublic"),
    "OnlineBankingFinland": ("BankRedirect", "OnlineBankingFinland"),
    "OnlineBankingFpx": ("BankRedirect", "OnlineBankingFpx"),
    "OnlineBankingPoland": ("BankRedirect", "OnlineBankingPoland"),
    "OnlineBankingSlovakia": ("BankRedirect", "OnlineBankingSlovakia"),
    # Open Banking
    "OpenBankingUk": ("OpenBanking", "OpenBankingUk"),
    "OpenBankingPis": ("OpenBanking", "OpenBankingPIS"),
    "OpenBanking": ("OpenBanking", "OpenBanking"),
    # Bank Redirect
    "LocalBankRedirect": ("BankRedirect", "LocalBankRedirect"),
    "Ideal": ("BankRedirect", "Ideal"),
    "Sofort": ("BankRedirect", "Sofort"),
    "Trustly": ("BankRedirect", "Trustly"),
    "Giropay": ("BankRedirect", "Giropay"),
    "Eps": ("BankRedirect", "Eps"),
    "Przelewy24": ("BankRedirect", "Przelewy24"),
    "Pse": ("BankRedirect", "Pse"),
    "Blik": ("BankRedirect", "Blik"),
    "Interac": ("BankRedirect", "Interac"),
    "Bizum": ("BankRedirect", "Bizum"),
    "Eft": ("BankRedirect", "Eft"),
    "DuitNow": ("BankRedirect", "DuitNow"),
    # Bank Transfer (PMType is bare; PM family disambiguates from BankDebit)
    "AchBankTransfer": ("BankTransfer", "Ach"),
    "SepaBankTransfer": ("BankTransfer", "SepaBankTransfer"),
    "BacsBankTransfer": ("BankTransfer", "Bacs"),
    "MultibancoBankTransfer": ("BankTransfer", "Multibanco"),
    "InstantBankTransfer": ("BankTransfer", "InstantBankTransfer"),
    "InstantBankTransferFinland": ("BankTransfer", "InstantBankTransferFinland"),
    "InstantBankTransferPoland": ("BankTransfer", "InstantBankTransferPoland"),
    "Pix": ("BankTransfer", "Pix"),
    "PermataBankTransfer": ("BankTransfer", "PermataBankTransfer"),
    "BcaBankTransfer": ("BankTransfer", "BcaBankTransfer"),
    "BniVaBankTransfer": ("BankTransfer", "BniVa"),
    "BriVaBankTransfer": ("BankTransfer", "BriVa"),
    "CimbVaBankTransfer": ("BankTransfer", "CimbVa"),
    "DanamonVaBankTransfer": ("BankTransfer", "DanamonVa"),
    "MandiriVaBankTransfer": ("BankTransfer", "MandiriVa"),
    "LocalBankTransfer": ("BankTransfer", "LocalBankTransfer"),
    "IndonesianBankTransfer": ("BankTransfer", "IndonesianBankTransfer"),
    # Bank Debit
    "Ach": ("BankDebit", "Ach"),
    "Sepa": ("BankDebit", "Sepa"),
    "Bacs": ("BankDebit", "Bacs"),
    "Becs": ("BankDebit", "Becs"),
    "SepaGuaranteedDebit": ("BankDebit", "SepaGuaranteedDebit"),
    # Alternative
    "Crypto": ("Crypto", "CryptoCurrency"),
    "ClassicReward": ("Reward", "ClassicReward"),
    "Givex": ("GiftCard", "Givex"),
    "PaySafeCard": ("Voucher", "PaySafeCard"),
    "EVoucher": ("Voucher", "Evoucher"),
    "Boleto": ("Voucher", "Boleto"),
    "Efecty": ("Voucher", "Efecty"),
    "PagoEfectivo": ("Voucher", "PagoEfectivo"),
    "RedCompra": ("Voucher", "RedCompra"),
    "RedPagos": ("Voucher", "RedPagos"),
    "Alfamart": ("Voucher", "Alfamart"),
    "Indomaret": ("Voucher", "Indomaret"),
    "Oxxo": ("Voucher", "Oxxo"),
    "SevenEleven": ("Voucher", "SevenEleven"),
    "Lawson": ("Voucher", "Lawson"),
    "MiniStop": ("Voucher", "MiniStop"),
    "FamilyMart": ("Voucher", "FamilyMart"),
    "Seicomart": ("Voucher", "Seicomart"),
    "PayEasy": ("Voucher", "PayEasy"),
}

# Probe keys the probe emits but that are not matrix columns. Silently dropped
# from the bootstrap output. Move to PROBE_TO_RUST + add to
# `_PROBE_PM_BY_CATEGORY` if a matrix column is wanted for one of these.
PROBE_SKIP_SILENTLY: frozenset[str] = frozenset({
    "BillDeskRedirect", "CashfreeRedirect", "EaseBuzzRedirect", "GcashRedirect",
    "LazyPayRedirect", "MobilePayRedirect", "Netbanking", "PayURedirect",
    "Paysera", "PhonePeRedirect", "Skrill",
})

STATUS_RANK = {"Supported": 3, "NotImplemented": 2, "NotSupported": 1}


def classify_probe_status(status: str) -> str | None:
    """Map probe-emitted status string to FeatureStatus variant.

    Returns None to mean "skip this PM" — used for statuses we can't classify
    (shouldn't happen with current probe output).
    """
    if status == "supported":
        return "Supported"
    if status == "not_supported":
        return "NotSupported"
    if status in ("not_implemented", "error"):
        # `error` is probe stuck-on-field / unrecognised-error bucket.
        # Default to NotImplemented (UCS TODO); Phase B may downgrade.
        return "NotImplemented"
    return None


def derive_entries(probe_data: dict) -> list[tuple[str, str, str]]:
    """Walk `flows.authorize.*` and produce deduped (PM, PMT, status) triples.

    Sparse: NotSupported entries are excluded. Aliases dedup to the strongest
    status across them.
    """
    auth = probe_data.get("flows", {}).get("authorize", {})
    bucket: dict[tuple[str, str], str] = {}
    for probe_key, info in auth.items():
        if probe_key in PROBE_SKIP_SILENTLY:
            continue
        mapped = PROBE_TO_RUST.get(probe_key)
        if mapped is None:
            # Unknown probe key — log to stderr and move on.
            print(f"  warn: unmapped probe key {probe_key!r}", file=sys.stderr)
            continue
        status = classify_probe_status(info.get("status", ""))
        if status is None or status == "NotSupported":
            # Sparse: omit NotSupported entirely.
            continue
        existing = bucket.get(mapped)
        if existing is None or STATUS_RANK[status] > STATUS_RANK[existing]:
            bucket[mapped] = status
    return [(pm, pmt, st) for (pm, pmt), st in sorted(bucket.items())]


def to_screaming_snake(name: str) -> str:
    """connector → CONNECTOR_NAME for the static identifier."""
    out: list[str] = []
    for i, ch in enumerate(name):
        if ch.isupper() and i > 0 and not name[i - 1].isupper():
            out.append("_")
        out.append(ch.upper())
    s = "".join(out).replace("__", "_")
    # Normalise the snake-case file stems too (e.g. absa_sanlam → ABSA_SANLAM).
    return s.replace("_", "_")


def render_lazy_lock_block(connector_name: str, entries: list[tuple[str, str, str]]) -> str:
    """The `static <NAME>_SUPPORTED_PAYMENT_METHODS: LazyLock<...> = ...;` block."""
    static_name = f"{to_screaming_snake(connector_name)}_SUPPORTED_PAYMENT_METHODS"
    lines: list[str] = []
    a = lines.append
    a(f"static {static_name}: std::sync::LazyLock<domain_types::types::SupportedPaymentMethods> =")
    a("    std::sync::LazyLock::new(|| {")
    a("        let mut m = domain_types::types::SupportedPaymentMethods::new();")
    a("        let default_capture = vec![common_enums::enums::CaptureMethod::Automatic];")
    for pm, pmt, status in entries:
        a("        m.entry(common_enums::enums::PaymentMethod::" + pm + ")")
        a("            .or_default()")
        a(f"            .insert(common_enums::enums::PaymentMethodType::{pmt},")
        a("                domain_types::types::PaymentMethodDetails {")
        a(f"                    status: domain_types::types::FeatureStatus::{status},")
        a("                    mandates: domain_types::types::FeatureStatus::NotImplemented,")
        a("                    refunds: domain_types::types::FeatureStatus::NotImplemented,")
        a("                    supported_capture_methods: default_capture.clone(),")
        a("                    specific_features: None,")
        a("                });")
    a("        m")
    a("    });")
    a("")
    return "\n".join(lines)


# Regex that matches the stub impl block for any connector — captures the
# leading `impl<...>` header text and the connector type up to the `<T>`.
STUB_IMPL_PATTERN = re.compile(
    r"(impl(?:<[^>]+>)?\s*"
    r"(?:[\w:]+::)?ConnectorSpecifications\s+for\s+(\w+)<[^>]+>\s*\{\s*"
    r"fn\s+get_supported_payment_methods\(&self\)\s*->\s*&'static\s+"
    r"(?:domain_types::types::)?SupportedPaymentMethods\s*\{\s*)"
    r"&domain_types::types::EMPTY_SUPPORTED_PAYMENT_METHODS",
    re.MULTILINE,
)


def apply_to_file(connector_name: str, entries: list[tuple[str, str, str]], *, apply: bool) -> str:
    """Update <connector>.rs to use a populated LazyLock instead of the empty stub.

    Returns a one-line status string.
    """
    source_stem = PROBE_TO_SOURCE_STEM.get(connector_name, connector_name)
    path = CONNECTORS_DIR / f"{source_stem}.rs"
    if not path.exists():
        return f"  skip: source not found ({path.name})"
    src = path.read_text(encoding="utf-8")

    match = STUB_IMPL_PATTERN.search(src)
    if match is None:
        if "EMPTY_SUPPORTED_PAYMENT_METHODS" not in src:
            return f"  skip: already declared (no EMPTY_… ref in source)"
        return f"  warn: EMPTY_… present but stub impl pattern didn't match — manual look needed"

    static_name = f"{to_screaming_snake(source_stem)}_SUPPORTED_PAYMENT_METHODS"

    if not entries:
        return f"  skip: probe gave zero usable PMs"

    block = render_lazy_lock_block(source_stem, entries)

    # Insert the LazyLock block immediately before the impl header, then
    # change the body to reference it.
    start = match.start()
    body_start = match.end()
    new_src = (
        src[:start]
        + block
        + "\n"
        + match.group(1)
        + f"&{static_name}"
        + src[body_start:]
    )

    if apply:
        path.write_text(new_src, encoding="utf-8")
        n_supp = sum(1 for *_, s in entries if s == "Supported")
        n_ni = sum(1 for *_, s in entries if s == "NotImplemented")
        return f"  ✓ {connector_name}.rs: {n_supp} Supported, {n_ni} NotImplemented"
    n_supp = sum(1 for *_, s in entries if s == "Supported")
    n_ni = sum(1 for *_, s in entries if s == "NotImplemented")
    return f"  dry-run {connector_name}: {n_supp} Supported, {n_ni} NotImplemented"


def main(argv: list[str]) -> int:
    apply = "--apply" in argv
    targets = {a for a in argv if not a.startswith("-")}

    processed: list[tuple[str, int, int]] = []
    skipped: list[str] = []

    for json_path in sorted(PROBE_DIR.glob("*.json")):
        name = json_path.stem
        if targets and name not in targets:
            continue
        if not targets and name in ALREADY_DECLARED:
            skipped.append(name)
            continue
        try:
            data = json.loads(json_path.read_text(encoding="utf-8"))
        except Exception as exc:
            print(f"  warn: failed to load {json_path}: {exc}", file=sys.stderr)
            continue
        entries = derive_entries(data)
        result = apply_to_file(name, entries, apply=apply)
        print(result)
        if entries:
            n_supp = sum(1 for *_, s in entries if s == "Supported")
            n_ni = sum(1 for *_, s in entries if s == "NotImplemented")
            processed.append((name, n_supp, n_ni))

    print()
    print(f"Processed {len(processed)} connectors. Skipped (already declared): {len(skipped)}.")
    total_supp = sum(s for _, s, _ in processed)
    total_ni = sum(n for _, _, n in processed)
    print(f"Totals: {total_supp} Supported entries, {total_ni} NotImplemented entries.")
    if not apply:
        print("Dry run — pass --apply to write source files.")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
