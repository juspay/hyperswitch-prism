#!/usr/bin/env python3
"""Apply Phase B per-PM verdicts back to connector source files.

Reads `data/backfill_audit.json` (output of the Phase B workflow) and, for
each verdict where `status == NotSupported` and `confidence in {high, medium}`,
removes the corresponding `m.entry(PaymentMethod::X).or_default().insert(
PaymentMethodType::Y, ...)` block from the connector's `<connector>.rs`.

Verdicts with status NotImplemented or low confidence are left as-is —
the matrix shows `⚠` (TODO) and the human author can refine later.

Usage:
    python3 scripts/generators/docs/apply_phase_b_verdicts.py            # dry-run
    python3 scripts/generators/docs/apply_phase_b_verdicts.py --apply    # edit in place
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[3]
AUDIT = REPO_ROOT / "data" / "backfill_audit.json"
SRC_DIR = REPO_ROOT / "crates" / "integrations" / "connector-integration" / "src" / "connectors"

STEM_MAP = {
    "absasanlam": "absa_sanlam",
    "pinelabsonline": "pinelabs_online",
    "twoctwoppaco": "twoc_twop_paco",
}

# Same probe-key → (PM, PMType) table as in bootstrap_capabilities.py.
# Duplicated here so this script is self-contained.
PROBE_TO_RUST: dict[str, tuple[str, str]] = {
    "Card": ("Card", "Card"),
    "BancontactCard": ("Card", "BancontactCard"),
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
    "Affirm": ("PayLater", "Affirm"),
    "Afterpay": ("PayLater", "AfterpayClearpay"),
    "Klarna": ("PayLater", "Klarna"),
    "UpiCollect": ("Upi", "UpiCollect"),
    "UpiIntent": ("Upi", "UpiIntent"),
    "UpiQr": ("Upi", "UpiQr"),
    "OnlineBankingThailand": ("BankRedirect", "OnlineBankingThailand"),
    "OnlineBankingCzechRepublic": ("BankRedirect", "OnlineBankingCzechRepublic"),
    "OnlineBankingFinland": ("BankRedirect", "OnlineBankingFinland"),
    "OnlineBankingFpx": ("BankRedirect", "OnlineBankingFpx"),
    "OnlineBankingPoland": ("BankRedirect", "OnlineBankingPoland"),
    "OnlineBankingSlovakia": ("BankRedirect", "OnlineBankingSlovakia"),
    "OpenBankingUk": ("OpenBanking", "OpenBankingUk"),
    "OpenBankingPis": ("OpenBanking", "OpenBankingPIS"),
    "OpenBanking": ("OpenBanking", "OpenBanking"),
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
    "Ach": ("BankDebit", "Ach"),
    "Sepa": ("BankDebit", "Sepa"),
    "Bacs": ("BankDebit", "Bacs"),
    "Becs": ("BankDebit", "Becs"),
    "SepaGuaranteedDebit": ("BankDebit", "SepaGuaranteedDebit"),
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


def _entry_pattern(pm: str, pmt: str) -> re.Pattern:
    """Regex matching the full `m.entry(...).or_default().insert(...);` block for (pm, pmt)."""
    return re.compile(
        r"        m\.entry\(common_enums::enums::PaymentMethod::" + re.escape(pm) + r"\)\n"
        r"            \.or_default\(\)\n"
        r"            \.insert\(common_enums::enums::PaymentMethodType::" + re.escape(pmt) + r",\n"
        r"                domain_types::types::PaymentMethodDetails \{[^}]+?\}\);\n",
        re.DOTALL,
    )


def remove_entry(src: str, pm: str, pmt: str) -> tuple[str, bool]:
    """Remove the (pm, pmt) entry block. Returns (new_source, removed_flag)."""
    new_src, n = _entry_pattern(pm, pmt).subn("", src)
    return new_src, n > 0


def upgrade_status(src: str, pm: str, pmt: str, new_status: str) -> tuple[str, bool]:
    """Change the `status: FeatureStatus::X,` field inside the (pm, pmt) entry to `new_status`.

    Returns (new_source, upgraded_flag).
    """
    pat = _entry_pattern(pm, pmt)
    m = pat.search(src)
    if not m:
        return src, False
    block = m.group(0)
    new_block = re.sub(
        r"status: domain_types::types::FeatureStatus::\w+,",
        f"status: domain_types::types::FeatureStatus::{new_status},",
        block,
        count=1,
    )
    if new_block == block:
        return src, False
    return src[: m.start()] + new_block + src[m.end():], True


def apply_verdicts(audit: list[dict], apply: bool) -> dict[str, int]:
    """Walk each connector's verdicts, remove NotSupported (high/medium conf) entries.

    Returns a stats dict: {connector: removed_count}.
    """
    stats: dict[str, int] = {}
    for report in audit:
        connector = report.get("connector")
        verdicts = report.get("verdicts", [])
        if not connector:
            continue

        stem = STEM_MAP.get(connector, connector)
        src_path = SRC_DIR / f"{stem}.rs"
        if not src_path.exists():
            print(f"  skip {connector}: source not found at {src_path.name}", file=sys.stderr)
            continue

        src = src_path.read_text(encoding="utf-8")
        removed = 0
        upgraded = 0
        kept = 0

        # Dedup verdicts by (PM, PMType) — multiple probe-key aliases (e.g.
        # ApplePay, ApplePayDecrypted) map to one entry. Voting rules:
        #   - all aliases say NotSupported (high/medium) → REMOVE
        #   - any alias says Supported (high/medium)     → UPGRADE to Supported
        #   - otherwise                                  → KEEP as NotImplemented
        agreement: dict[tuple[str, str], list[tuple[str, str]]] = {}
        for v in verdicts:
            pm_probe = v["pm"]
            mapped = PROBE_TO_RUST.get(pm_probe)
            if mapped is None:
                continue
            agreement.setdefault(mapped, []).append((v["status"], v["confidence"]))

        for (pm, pmt), votes in agreement.items():
            all_ns = all(
                s == "NotSupported" and c in ("high", "medium")
                for s, c in votes
            )
            any_supp = any(
                s == "Supported" and c in ("high", "medium")
                for s, c in votes
            )

            if all_ns:
                new_src, did = remove_entry(src, pm, pmt)
                if did:
                    src = new_src
                    removed += 1
            elif any_supp:
                new_src, did = upgrade_status(src, pm, pmt, "Supported")
                if did:
                    src = new_src
                    upgraded += 1
                else:
                    kept += 1
            else:
                kept += 1

        if (removed or upgraded) and apply:
            src_path.write_text(src, encoding="utf-8")
        stats[connector] = removed
        if removed or upgraded or kept:
            print(f"  {connector:<22} removed={removed:>3} upgraded={upgraded:>2} kept={kept:>3}")
    return stats


def main(argv: list[str]) -> int:
    if not AUDIT.exists():
        print(f"Error: audit file not found at {AUDIT}", file=sys.stderr)
        return 1
    audit = json.loads(AUDIT.read_text(encoding="utf-8"))
    apply = "--apply" in argv
    stats = apply_verdicts(audit, apply=apply)
    total = sum(stats.values())
    print(f"\nTotal removed: {total} entries across {sum(1 for n in stats.values() if n)} connectors")
    if not apply:
        print("Dry run — pass --apply to edit source files.")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
