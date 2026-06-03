#!/usr/bin/env python3
"""Reconcile each connector's declared `SupportedPaymentMethods` map with
what the runtime probe actually observed.

For each (PaymentMethod, PaymentMethodType) pair the probe found as
`supported` or `not_implemented` AND the declaration omits, append a new
`<local_var>.add(...)` call to the connector's `LazyLock::new` body just
before the trailing implicit return.

Add-only — never touches an existing entry. Idempotent — a second run
after the first finds no drift because the new entries are now declared.

Default is dry-run; pass --apply to mutate source files.

Targets the 6 connectors that bypassed Change 6's bootstrap because they
had hand-written declarations: adyen, calida, easebuzz, hyperpg, ppro, zift.
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[3]
CONNECTORS_DIR = REPO_ROOT / "crates" / "integrations" / "connector-integration" / "src" / "connectors"
STATIC_DIR = REPO_ROOT / "data" / "connector_capabilities"
RUNTIME_DIR = REPO_ROOT / "data" / "field_probe"

# Probe-stem → source-file-stem (probe JSONs drop underscores some sources keep).
STEM_MAP: dict[str, str] = {
    "absasanlam": "absa_sanlam",
    "pinelabsonline": "pinelabs_online",
    "twoctwoppaco": "twoc_twop_paco",
}

# Probe PM key → (PaymentMethod variant, PaymentMethodType variant).
# Kept in sync with `_PROBE_PM_BY_CATEGORY` in generate.py and
# `bootstrap_capabilities.py:PROBE_TO_RUST`.
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

RANK = {"Supported": 3, "NotImplemented": 2}


def compute_additions(
    probe_name: str,
    include_stuck: bool = False,
) -> list[tuple[str, str, str]]:
    """Return `[(pm, pmt, status), ...]` for entries the probe found but the
    declaration omits. Dedup aliases by (pm, pmt), keeping strongest status.

    When `include_stuck` is True (Change 9 mode), additionally treat
    `probe.status == "error"` with an error message starting with
    `"Stuck on field:"` as evidence that the transformer's code path runs.
    These cells get added as `NotImplemented` (conservative — surfaces as
    `⚠ TODO` in the matrix instead of the `x` NotSupported-by-absence
    that Change 8 left them as).
    """
    static_path = STATIC_DIR / f"{probe_name}.json"
    runtime_path = RUNTIME_DIR / f"{probe_name}.json"
    if not static_path.exists() or not runtime_path.exists():
        return []

    declared: set[tuple[str, str]] = set()
    static = json.loads(static_path.read_text(encoding="utf-8"))
    for pm, types in (static.get("supported_payment_methods") or {}).items():
        for pmt in types.keys():
            declared.add((pm, pmt))

    runtime = json.loads(runtime_path.read_text(encoding="utf-8"))
    auth = runtime.get("flows", {}).get("authorize", {})

    candidates: dict[tuple[str, str], str] = {}
    for probe_key, info in auth.items():
        mapped = PROBE_TO_RUST.get(probe_key)
        if mapped is None or mapped in declared:
            continue
        s = info.get("status")
        err = info.get("error") or ""
        if s == "supported":
            new_status: str | None = "Supported"
        elif s == "not_implemented":
            new_status = "NotImplemented"
        elif include_stuck and s == "error" and err.startswith("Stuck on field:"):
            # Change 9: the transformer ran past initial validation and
            # demanded a specific field — strong evidence the code path
            # exists. We can't tell from the probe alone whether the PM
            # arm is Supported / NotImplemented / catch-all NotSupported,
            # so surface as NotImplemented (visible TODO) rather than the
            # default `x` (NotSupported by absence).
            new_status = "NotImplemented"
        else:
            new_status = None

        if new_status is None:
            continue
        cur = candidates.get(mapped)
        if cur is None or RANK[new_status] > RANK[cur]:
            candidates[mapped] = new_status

    return sorted((pm, pmt, st) for (pm, pmt), st in candidates.items())


def render_entry(local_var: str, pm: str, pmt: str, status: str) -> str:
    """One fully-qualified entry-insert call.

    Uses `.entry(...).or_default().insert(...)` (raw HashMap API) rather
    than the `SupportedPaymentMethodsExt::add(...)` helper, because not
    every connector imports the trait. The HashMap API works regardless
    of what `use` statements exist in the target file. Fully-qualified
    paths likewise avoid depending on existing imports.
    """
    return (
        f"    {local_var}\n"
        f"        .entry(common_enums::enums::PaymentMethod::{pm})\n"
        f"        .or_default()\n"
        f"        .insert(\n"
        f"            common_enums::enums::PaymentMethodType::{pmt},\n"
        f"            domain_types::types::PaymentMethodDetails {{\n"
        f"                status: domain_types::types::FeatureStatus::{status},\n"
        f"                mandates: domain_types::types::FeatureStatus::NotImplemented,\n"
        f"                refunds: domain_types::types::FeatureStatus::NotImplemented,\n"
        f"                supported_capture_methods: vec![common_enums::enums::CaptureMethod::Automatic],\n"
        f"                specific_features: None,\n"
        f"            }},\n"
        f"        );\n"
    )


# Anchor for the implicit return at the end of a LazyLock::new(|| { ... }) body.
# Matches `\n<indent><identifier>\n<indent>})` where the identifier is the
# local variable being returned and `})` closes the closure. The semicolon
# may follow immediately (Adyen style: `});`) or one level up (HyperPG style:
# the LazyLock::new is wrapped in an outer block expression, so `})` then
# `\n};`). We don't require the `;` here — the static-name walkback below
# disambiguates which closure we're at.
TAIL_PATTERN = re.compile(
    r"(\n[ \t]+)([a-z][a-z0-9_]*)(\n[ \t]*\}\))",
)


def apply(probe_name: str, additions: list[tuple[str, str, str]], do_write: bool) -> str:
    """Insert add() calls before the LazyLock body's implicit return line.

    Returns a one-line status string.
    """
    if not additions:
        return f"  {probe_name:<14s} 0 additions (declaration already in sync)"

    source_stem = STEM_MAP.get(probe_name, probe_name)
    path = CONNECTORS_DIR / f"{source_stem}.rs"
    if not path.exists():
        return f"  {probe_name:<14s} SKIP: source not found at {path.name}"

    src = path.read_text(encoding="utf-8")

    # Find the implicit-return tail. There may be multiple LazyLock::new blocks
    # in the file (some connectors have multiple statics — e.g. WEBHOOK_FLOWS),
    # so we look for the one whose static name ends with SUPPORTED_PAYMENT_METHODS.
    # Strategy: find each TAIL_PATTERN match, walk backward in source to find the
    # nearest `static <NAME>: ... LazyLock`, and pick the one ending in
    # SUPPORTED_PAYMENT_METHODS.
    target_match = None
    for m in TAIL_PATTERN.finditer(src):
        head = src[: m.start()]
        static_decl = re.search(r"static\s+([A-Z_]+):", head[::-1])
        # easier: just look backward for the most-recent `static <NAME>:`
        statics = list(re.finditer(r"static\s+([A-Z_]+)\s*:", head))
        if not statics:
            continue
        last = statics[-1].group(1)
        if last.endswith("SUPPORTED_PAYMENT_METHODS"):
            target_match = m
            break

    if target_match is None:
        return f"  {probe_name:<14s} FAIL: could not locate LazyLock tail"

    local_var = target_match.group(2)
    # Insertion point: just before the leading `\n<indent>` that introduces the
    # return-identifier line. target_match.start() is the start of the leading \n;
    # we insert the new entries there so the return line stays where it was.
    insertion = "".join(
        render_entry(local_var, pm, pmt, st) for pm, pmt, st in additions
    )

    new_src = src[: target_match.start()] + "\n" + insertion + src[target_match.start():]

    if do_write:
        path.write_text(new_src, encoding="utf-8")

    n_supp = sum(1 for *_, st in additions if st == "Supported")
    n_ni = sum(1 for *_, st in additions if st == "NotImplemented")
    verb = "WROTE" if do_write else "DRY-RUN"
    return (
        f"  {probe_name:<14s} {verb:<7s} +{n_supp:>3d} Supported +{n_ni:>3d} NotImplemented "
        f"→ {path.name} (returning `{local_var}`)"
    )


def main(argv: list[str]) -> int:
    do_write = "--apply" in argv
    include_stuck = "--include-stuck" in argv
    cli_names = [a for a in argv if not a.startswith("-")]
    targets = cli_names if cli_names else None

    # If no explicit subset, iterate every probe JSON. The compute_additions
    # call returns [] for connectors already in sync, so the apply loop is
    # naturally narrowed to drifted ones.
    if targets is None:
        targets = sorted(f.stem for f in RUNTIME_DIR.glob("*.json"))

    print(f"{'connector':<14s} {'mode':<7s} {'adds':<43s}")
    print("-" * 70)
    drifted = 0
    for name in targets:
        adds = compute_additions(name, include_stuck=include_stuck)
        if adds:
            drifted += 1
        print(apply(name, adds, do_write))

    print()
    flag_note = " (with --include-stuck)" if include_stuck else ""
    print(
        f"Total drifted connectors: {drifted}{flag_note}. "
        f"{'Wrote.' if do_write else 'Dry run — pass --apply to write source files.'}"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
