#!/usr/bin/env python3
"""Run Santander payout create + transfer flows through grpcurl.

Options:
  1. Create access token
  2. Create + transfer payout with PIX key
  3. Create + transfer payout with PIX EMV QR code
  4. Create + transfer payout with PIX bank account
  5. Create + transfer all three PIX variants
  6. Create + transfer PIX key without access token

Environment variables can be used to avoid prompts:
  UCS_HOST, SANTANDER_CLIENT_ID, SANTANDER_CLIENT_SECRET,
  SANTANDER_WORKSPACE_ID, SANTANDER_ENVIRONMENT, SANTANDER_BASE_URL,
  SANTANDER_SECONDARY_BASE_URL, SANTANDER_HOST, SANTANDER_PIX_KEY,
  SANTANDER_PIX_EMV, SANTANDER_PIX_BANK_BRANCH,
  SANTANDER_PIX_BANK_ACCOUNT_NUMBER, SANTANDER_PIX_TAX_ID,
  SANTANDER_PIX_ISPB, SANTANDER_SOURCE_BANK_BRANCH,
  SANTANDER_SOURCE_BANK_ACCOUNT_NUMBER, SANTANDER_ACCESS_TOKEN,
  SANTANDER_PFX_FILE, SANTANDER_PFX_PASSPHRASE
"""

from __future__ import annotations

import argparse
import base64
from dataclasses import dataclass
import getpass
import json
import os
import pathlib
import re
import subprocess
import sys
from typing import Any


SCRIPT_PATH = pathlib.Path(__file__).resolve()
REPO_ROOT = SCRIPT_PATH.parents[2]
DEFAULT_PROTO_DIR = REPO_ROOT / "crates/types-traits/grpc-api-types/proto"
DEFAULT_SANTANDER_PFX_FILE = pathlib.Path(
    "/Users/aniket.burman/Downloads/GDPP-BR-Cert.pfx"
)


@dataclass(frozen=True)
class SantanderRunConfig:
    ucs_host: str = "localhost:8000"
    org_id: str = "default"
    merchant_id: str = "test_merchant_123"
    environment: str = "production"
    santander_host: str = "trust-open-h.api.santander.com.br"
    client_id: str = "dDGtNUroCCISFiTaZkUWslbIM6XdWa7p"
    client_secret: str = ""
    workspace_id: str = "92ee8a27-de05-4ce9-87e3-2a8031a53607"
    pfx_file: pathlib.Path | None = DEFAULT_SANTANDER_PFX_FILE
    pfx_passphrase: str | None = None
    pix_key: str = "teste_api_projeto_cobranca@santander.com.br"
    pix_emv: str = ""
    pix_bank_branch: str = "0001"
    pix_bank_account_number: str = ""
    pix_tax_id: str = ""
    pix_ispb: str = ""
    source_bank_branch: str = "0001"
    source_bank_account_number: str = "130375431"
    access_token: str = ""


DEFAULT_CONFIG = SantanderRunConfig()
SANTANDER_URLS = {
    "sandbox": {
        "base_url": "https://trust-open-h.api.santander.com.br",
        "secondary_base_url": "https://trust-open-h.api.santander.com.br",
    },
    "production": {
        "base_url": "https://trust-open-h.api.santander.com.br",
        "secondary_base_url": "https://trust-open-h.api.santander.com.br",
    },
}
PEM_BLOCK_RE = re.compile(
    r"-----BEGIN ([^-]+)-----.*?-----END \1-----\s*",
    re.DOTALL,
)


def read_value(
    env_name: str,
    prompt: str,
    *,
    default: str | None = None,
    required: bool = True,
    secret: bool = False,
) -> str | None:
    value = os.getenv(env_name)
    if value:
        return value

    suffix = f" [{default}]" if default is not None else ""
    if secret:
        entered = getpass.getpass(f"{prompt}{suffix}: ").strip()
    else:
        entered = input(f"{prompt}{suffix}: ").strip()

    if entered:
        return entered
    if default is not None:
        return default
    if required:
        raise SystemExit(f"Missing required value: {env_name}")
    return None


def configured_value(
    cli_value: str | None,
    env_name: str,
    config_value: str | None,
    prompt: str,
    *,
    required: bool = True,
    secret: bool = False,
) -> str | None:
    if cli_value:
        return cli_value

    env_value = os.getenv(env_name)
    if env_value:
        return env_value

    if config_value:
        return config_value

    return read_value(env_name, prompt, required=required, secret=secret)


def default_pfx_file() -> pathlib.Path | None:
    env_value = os.getenv("SANTANDER_PFX_FILE")
    if env_value:
        return pathlib.Path(env_value)
    if DEFAULT_CONFIG.pfx_file is not None and DEFAULT_CONFIG.pfx_file.exists():
        return DEFAULT_CONFIG.pfx_file
    return None


def read_pfx_passphrase(args: argparse.Namespace) -> str:
    if args.pfx_passphrase is not None:
        return args.pfx_passphrase

    env_value = os.getenv("SANTANDER_PFX_PASSPHRASE")
    if env_value is not None:
        return env_value

    if DEFAULT_CONFIG.pfx_passphrase is not None:
        return DEFAULT_CONFIG.pfx_passphrase

    return getpass.getpass("Santander PFX passphrase (press Enter if empty): ")


def run_openssl_pkcs12(
    pfx_file: pathlib.Path,
    passphrase: str,
    pkcs12_args: list[str],
) -> str:
    def run_once(*, legacy: bool) -> subprocess.CompletedProcess[str]:
        command = [
            "openssl",
            "pkcs12",
            *(["-legacy"] if legacy else []),
            "-in",
            str(pfx_file),
            "-passin",
            "stdin",
            *pkcs12_args,
        ]

        return subprocess.run(
            command,
            input=f"{passphrase}\n",
            text=True,
            capture_output=True,
            check=False,
        )

    try:
        result = run_once(legacy=False)
        error = result.stderr.strip() or result.stdout.strip()
        if result.returncode != 0 and ("RC2" in error or "unsupported" in error):
            result = run_once(legacy=True)
            error = result.stderr.strip() or result.stdout.strip()
    except FileNotFoundError as exc:
        raise SystemExit("openssl is required to read the Santander PFX file") from exc

    if result.returncode != 0:
        if "Mac verify error" in error or "invalid password" in error.lower():
            raise SystemExit(
                "Unable to read Santander PFX file: invalid PFX passphrase.\n"
                "Set it once in DEFAULT_CONFIG.pfx_passphrase, pass "
                "--pfx-passphrase, or export SANTANDER_PFX_PASSPHRASE."
            )
        raise SystemExit(f"Unable to read Santander PFX file with openssl:\n{error}")

    return result.stdout


def extract_pem_blocks(openssl_output: str, block_type: str) -> str:
    blocks: list[str] = []

    for match in PEM_BLOCK_RE.finditer(openssl_output):
        label = match.group(1)
        if block_type == "certificate" and label == "CERTIFICATE":
            blocks.append(match.group(0).strip() + "\n")
        if block_type == "private_key" and label.endswith("PRIVATE KEY"):
            blocks.append(match.group(0).strip() + "\n")

    if not blocks:
        raise SystemExit(f"No {block_type} PEM block found in Santander PFX file")

    return "".join(blocks)


def encode_santander_pfx(pfx_file: pathlib.Path, passphrase: str) -> tuple[str, str]:
    certificate_output = run_openssl_pkcs12(pfx_file, passphrase, ["-nokeys"])
    private_key_output = run_openssl_pkcs12(
        pfx_file,
        passphrase,
        ["-nocerts", "-nodes"],
    )
    certificates_pem = extract_pem_blocks(certificate_output, "certificate")
    private_key_pem = extract_pem_blocks(private_key_output, "private_key")

    return (
        base64.b64encode(certificates_pem.encode()).decode(),
        base64.b64encode(private_key_pem.encode()).decode(),
    )


def santander_host_url(args: argparse.Namespace) -> str | None:
    host = args.santander_host or os.getenv("SANTANDER_HOST") or DEFAULT_CONFIG.santander_host
    if not host:
        return None
    if host.startswith(("http://", "https://")):
        return host.rstrip("/")
    return f"https://{host.rstrip('/')}"


def build_connector_config(args: argparse.Namespace) -> str:
    default_urls = SANTANDER_URLS[args.environment]
    host_url = santander_host_url(args)
    santander: dict[str, str] = {
        "client_id": configured_value(
            args.client_id,
            "SANTANDER_CLIENT_ID",
            DEFAULT_CONFIG.client_id,
            "Santander client_id",
        ),
        "client_secret": configured_value(
            args.client_secret,
            "SANTANDER_CLIENT_SECRET",
            DEFAULT_CONFIG.client_secret,
            "Santander client_secret",
            secret=True,
        ),
        "workspace_id": configured_value(
            args.workspace_id,
            "SANTANDER_WORKSPACE_ID",
            DEFAULT_CONFIG.workspace_id,
            "Santander workspace_id",
        ),
        "base_url": args.base_url
        or os.getenv("SANTANDER_BASE_URL")
        or host_url
        or default_urls["base_url"],
        "secondary_base_url": args.secondary_base_url
        or os.getenv("SANTANDER_SECONDARY_BASE_URL")
        or host_url
        or default_urls["secondary_base_url"],
    }

    if not args.no_certificate and args.pfx_file:
        certificates, private_key = encode_santander_pfx(
            args.pfx_file,
            read_pfx_passphrase(args),
        )
        santander["certificates"] = certificates
        santander["private_key"] = private_key

    return json.dumps({"config": {"Santander": santander}}, separators=(",", ":"))


def run_grpcurl(
    *,
    args: argparse.Namespace,
    connector_config: str,
    request_id: str,
    method: str,
    payload: dict[str, Any],
) -> dict[str, Any] | None:
    command = [
        "grpcurl",
        "-plaintext",
        "-import-path",
        str(args.proto_dir),
        "-proto",
        "services.proto",
        "-H",
        f"x-connector-config: {connector_config}",
        "-H",
        "x-payout-connector: santander",
        "-H",
        f"x-merchant-id: {args.merchant_id}",
        "-H",
        f"x-tenant-id: {args.tenant_id}",
        "-H",
        f"x-request-id: {request_id}",
        "-H",
        f"x-connector-request-reference-id: {request_id}",
        "-d",
        json.dumps(payload, separators=(",", ":")),
        args.host,
        method,
    ]

    print(f"\nCalling {method}")
    result = subprocess.run(command, text=True, capture_output=True, check=False)

    if result.stdout:
        print("\nResponse:")
        print(result.stdout)
    if result.stderr:
        print("\nErrors:")
        print(result.stderr, file=sys.stderr)

    if result.returncode != 0:
        print(f"\ngrpcurl exited with status {result.returncode}")
        return None

    try:
        return json.loads(result.stdout)
    except json.JSONDecodeError:
        return None


def check_grpc_server(args: argparse.Namespace) -> None:
    command = ["grpcurl", "-plaintext", args.host, "list"]

    try:
        result = subprocess.run(command, text=True, capture_output=True, check=False)
    except FileNotFoundError as exc:
        raise SystemExit("grpcurl is required to run this helper") from exc

    output = "\n".join(part for part in (result.stdout, result.stderr) if part).strip()
    if result.returncode == 0:
        return

    if "404" in output or "missing HTTP content-type" in output:
        raise SystemExit(
            f"{args.host} is not the UCS gRPC endpoint. "
            "Use the server port, usually localhost:8000. "
            "Port 8080 is normally the metrics/http port in this repo."
        )

    if "connection refused" in output.lower() or "failed to dial" in output.lower():
        raise SystemExit(f"Could not connect to UCS gRPC server at {args.host}:\n{output}")

    print(f"\nWarning: grpcurl preflight could not list services at {args.host}.")
    print("Continuing because some local servers disable reflection.")


def request_access_token(args: argparse.Namespace, connector_config: str) -> str:
    payload = {
        "merchantAccessTokenId": args.token_request_id,
        "connector": "SANTANDER",
    }
    response = run_grpcurl(
        args=args,
        connector_config=connector_config,
        request_id=args.token_request_id,
        method="types.MerchantAuthenticationService/CreateServerAuthenticationToken",
        payload=payload,
    )

    token = (
        response.get("accessToken", {}).get("value")
        if isinstance(response, dict)
        else None
    )
    if not token:
        raise SystemExit("Access token was not returned by Santander auth flow")
    return token


def create_access_token(args: argparse.Namespace, connector_config: str) -> None:
    token = request_access_token(args, connector_config)
    print(f"\nAccess token for transfer options:\n{token}")


def secret_value(value: str) -> dict[str, str]:
    return {"value": value}


def read_configured_case_value(
    cli_value: str | None,
    env_name: str,
    config_value: str,
    prompt: str,
) -> str:
    value = configured_value(cli_value, env_name, config_value, prompt)
    if value is None:
        raise SystemExit(f"Missing required value: {env_name}")
    return value


def build_payout_method_data(args: argparse.Namespace, pix_case: str) -> dict[str, Any]:
    if pix_case == "pix_key":
        pix_key = read_configured_case_value(
            args.pix_key,
            "SANTANDER_PIX_KEY",
            DEFAULT_CONFIG.pix_key,
            "Pix key",
        )
        return {
            "pixKey": {
                "pixKey": secret_value(pix_key),
            }
        }

    if pix_case == "pix_emv":
        emv = read_configured_case_value(
            args.pix_emv,
            "SANTANDER_PIX_EMV",
            DEFAULT_CONFIG.pix_emv,
            "Pix EMV QR code",
        )
        return {
            "pixEmv": {
                "emv": secret_value(emv),
            }
        }

    if pix_case == "pix_bank":
        branch = read_configured_case_value(
            args.pix_bank_branch,
            "SANTANDER_PIX_BANK_BRANCH",
            DEFAULT_CONFIG.pix_bank_branch,
            "Pix beneficiary bank branch",
        )
        account_number = read_configured_case_value(
            args.pix_bank_account_number,
            "SANTANDER_PIX_BANK_ACCOUNT_NUMBER",
            DEFAULT_CONFIG.pix_bank_account_number,
            "Pix beneficiary bank account number",
        )
        tax_id = read_configured_case_value(
            args.pix_tax_id,
            "SANTANDER_PIX_TAX_ID",
            DEFAULT_CONFIG.pix_tax_id,
            "Pix beneficiary CPF/CNPJ",
        )
        ispb = configured_value(
            args.pix_ispb,
            "SANTANDER_PIX_ISPB",
            DEFAULT_CONFIG.pix_ispb,
            "Pix beneficiary ISPB",
            required=False,
        )

        pix: dict[str, Any] = {
            "bankBranch": branch,
            "bankAccountNumber": secret_value(account_number),
            "taxId": secret_value(tax_id),
        }
        if ispb:
            pix["ispb"] = secret_value(ispb)

        return {"pix": pix}

    raise SystemExit(f"Unknown PIX case: {pix_case}")


def build_source_bank_data(args: argparse.Namespace) -> dict[str, Any]:
    branch = read_configured_case_value(
        args.source_bank_branch,
        "SANTANDER_SOURCE_BANK_BRANCH",
        DEFAULT_CONFIG.source_bank_branch,
        "Santander source/debit bank branch",
    )
    account_number = read_configured_case_value(
        args.source_bank_account_number,
        "SANTANDER_SOURCE_BANK_ACCOUNT_NUMBER",
        DEFAULT_CONFIG.source_bank_account_number,
        "Santander source/debit bank account number",
    )

    return {
        "pix": {
            "bankBranch": branch,
            "bankAccountNumber": secret_value(account_number),
        }
    }


def build_create_payload(
    args: argparse.Namespace,
    access_token: str | None,
    pix_case: str,
    payout_id: str,
) -> dict[str, Any]:
    payload: dict[str, Any] = {
        "merchantPayoutId": payout_id,
        "amount": {
            "minorAmount": args.amount,
            "currency": args.currency,
        },
        "destinationCurrency": args.currency,
        "payoutMethodData": build_payout_method_data(args, pix_case),
        "customer": {
            "firstName": args.customer_first_name,
            "lastName": args.customer_last_name,
        },
    }

    if access_token:
        payload["accessToken"] = {"value": access_token}

    return payload


def build_transfer_payload(
    args: argparse.Namespace,
    access_token: str | None,
    payout_id: str,
    connector_payout_id: str,
) -> dict[str, Any]:
    payload: dict[str, Any] = {
        "merchantPayoutId": payout_id,
        "connectorPayoutId": connector_payout_id,
        "amount": {
            "minorAmount": args.amount,
            "currency": args.currency,
        },
        "destinationCurrency": args.currency,
    }

    if not args.skip_debit_account:
        payload["sourceBankData"] = build_source_bank_data(args)

    if access_token:
        payload["accessToken"] = {"value": access_token}

    return payload


def configured_access_token(args: argparse.Namespace) -> str | None:
    if args.access_token:
        return args.access_token

    env_value = os.getenv("SANTANDER_ACCESS_TOKEN")
    if env_value:
        return env_value

    if DEFAULT_CONFIG.access_token:
        return DEFAULT_CONFIG.access_token

    return None


def read_access_token(args: argparse.Namespace, connector_config: str) -> str:
    access_token = configured_access_token(args)
    if access_token is None:
        print("No access token supplied; creating one before payout transfer.")
        access_token = request_access_token(args, connector_config)

    if access_token.lower().startswith("bearer "):
        access_token = access_token[7:].strip()
    print("Access token received; calling payout create.")
    return access_token


def connector_payout_id_from_create_response(response: dict[str, Any] | None) -> str:
    connector_payout_id = (
        response.get("connectorPayoutId") if isinstance(response, dict) else None
    )
    if not connector_payout_id:
        raise SystemExit("Payout create did not return connectorPayoutId")
    return connector_payout_id


def create_payout(
    args: argparse.Namespace,
    connector_config: str,
    pix_case: str,
    payout_id: str,
    access_token: str | None,
) -> str:
    response = run_grpcurl(
        args=args,
        connector_config=connector_config,
        request_id=f"{payout_id}-create",
        method="types.PayoutService/Create",
        payload=build_create_payload(args, access_token, pix_case, payout_id),
    )
    connector_payout_id = connector_payout_id_from_create_response(response)
    print(f"\nCreated Santander payout id: {connector_payout_id}")
    return connector_payout_id


def transfer_payout(
    args: argparse.Namespace,
    connector_config: str,
    payout_id: str,
    connector_payout_id: str,
    access_token: str | None,
) -> None:
    print("Calling payout transfer/fulfill.")
    run_grpcurl(
        args=args,
        connector_config=connector_config,
        request_id=f"{payout_id}-transfer",
        method="types.PayoutService/Transfer",
        payload=build_transfer_payload(
            args,
            access_token,
            payout_id,
            connector_payout_id,
        ),
    )


def create_then_transfer_with_access_token(
    args: argparse.Namespace,
    connector_config: str,
    pix_case: str,
    payout_id: str | None = None,
) -> None:
    payout_id = payout_id or args.payout_id
    access_token = read_access_token(args, connector_config)
    connector_payout_id = create_payout(
        args,
        connector_config,
        pix_case,
        payout_id,
        access_token,
    )
    transfer_payout(
        args,
        connector_config,
        payout_id,
        connector_payout_id,
        access_token,
    )


def create_then_transfer_without_access_token(
    args: argparse.Namespace,
    connector_config: str,
    pix_case: str,
) -> None:
    payout_id = f"{args.payout_id}-{pix_case}-no-token"
    connector_payout_id = create_payout(args, connector_config, pix_case, payout_id, None)
    transfer_payout(args, connector_config, payout_id, connector_payout_id, None)


def create_then_transfer_all_pix_cases(
    args: argparse.Namespace,
    connector_config: str,
) -> None:
    access_token = read_access_token(args, connector_config)
    for pix_case in ("pix_key", "pix_emv", "pix_bank"):
        payout_id = f"{args.payout_id}-{pix_case}"
        connector_payout_id = create_payout(
            args,
            connector_config,
            pix_case,
            payout_id,
            access_token,
        )
        transfer_payout(
            args,
            connector_config,
            payout_id,
            connector_payout_id,
            access_token,
        )


def select_option(cli_option: str | None) -> str:
    if cli_option:
        return cli_option

    print("\nSelect flow:")
    print("1. Create access token")
    print("2. Payout create + transfer with PIX key")
    print("3. Payout create + transfer with PIX EMV QR code")
    print("4. Payout create + transfer with PIX bank account")
    print("5. Run all three PIX create + transfer variants")
    print("6. Payout create + transfer with PIX key without access token")
    return input("Enter 1, 2, 3, 4, 5, or 6: ").strip()


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Santander payout grpcurl helper")
    parser.add_argument("option", nargs="?", choices=["1", "2", "3", "4", "5", "6"])
    parser.add_argument(
        "--host",
        default=os.getenv("UCS_HOST", DEFAULT_CONFIG.ucs_host),
        help="UCS gRPC server host, not the Santander API host.",
    )
    parser.add_argument("--proto-dir", type=pathlib.Path, default=DEFAULT_PROTO_DIR)
    parser.add_argument(
        "--merchant-id",
        default=os.getenv("UCS_MERCHANT_ID", DEFAULT_CONFIG.merchant_id),
    )
    parser.add_argument(
        "--org-id",
        "--tenant-id",
        dest="tenant_id",
        default=os.getenv("UCS_ORG_ID")
        or os.getenv("UCS_TENANT_ID")
        or DEFAULT_CONFIG.org_id,
        help="UCS org/tenant id sent as x-tenant-id.",
    )
    parser.add_argument(
        "--environment",
        choices=["sandbox", "production"],
        default=os.getenv("SANTANDER_ENVIRONMENT", DEFAULT_CONFIG.environment),
        help="Chooses the hardcoded Santander URL defaults.",
    )

    parser.add_argument("--client-id")
    parser.add_argument("--client-secret")
    parser.add_argument("--workspace-id")
    parser.add_argument("--base-url")
    parser.add_argument("--secondary-base-url")
    parser.add_argument(
        "--santander-host",
        help="Santander API host. Example: trust-open-h.api.santander.com.br",
    )
    parser.add_argument("--pfx-file", type=pathlib.Path, default=default_pfx_file())
    parser.add_argument("--pfx-passphrase")
    parser.add_argument(
        "--no-certificate",
        action="store_true",
        help="Skip Santander mTLS certificate fields in x-connector-config.",
    )

    parser.add_argument("--token-request-id", default="santander-token-001")
    parser.add_argument("--payout-id", default="santander-payout-001")
    parser.add_argument("--amount", type=int, default=1000)
    parser.add_argument("--currency", default="BRL")
    parser.add_argument("--pix-key")
    parser.add_argument("--pix-emv")
    parser.add_argument("--pix-bank-branch")
    parser.add_argument("--pix-bank-account-number")
    parser.add_argument("--pix-tax-id")
    parser.add_argument("--pix-ispb")
    parser.add_argument("--source-bank-branch")
    parser.add_argument("--source-bank-account-number")
    parser.add_argument(
        "--skip-debit-account",
        action="store_true",
        help="Do not send sourceBankData/debitAccount in payout transfer.",
    )
    parser.add_argument("--access-token")
    parser.add_argument("--customer-first-name", default="Test")
    parser.add_argument("--customer-last-name", default="User")
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    check_grpc_server(args)
    connector_config = build_connector_config(args)
    option = select_option(args.option)

    if option == "1":
        create_access_token(args, connector_config)
    elif option == "2":
        create_then_transfer_with_access_token(args, connector_config, "pix_key")
    elif option == "3":
        create_then_transfer_with_access_token(args, connector_config, "pix_emv")
    elif option == "4":
        create_then_transfer_with_access_token(args, connector_config, "pix_bank")
    elif option == "5":
        create_then_transfer_all_pix_cases(args, connector_config)
    elif option == "6":
        create_then_transfer_without_access_token(args, connector_config, "pix_key")
    else:
        raise SystemExit("Invalid option. Choose 1, 2, 3, 4, 5, or 6.")


if __name__ == "__main__":
    main()
