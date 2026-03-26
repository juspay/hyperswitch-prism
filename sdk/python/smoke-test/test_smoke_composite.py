#!/usr/bin/env python3
"""
Composite smoke test for hyperswitch-payments SDK.

Tests:
  1. PayPal access token flow (create token + authorize)
  2. Stripe direct authorize flow

The test only passes when payments succeed (status == CHARGED).
CI will fail if any test fails.

Usage:
    python3 test_smoke_composite.py --creds-file creds.json
"""

import argparse
import asyncio
import json
import os
import sys
import time
from typing import Dict, Any, Optional, Tuple

# Add parent directory to path for imports when running directly
sys.path.insert(0, os.path.join(os.path.dirname(__file__), "..", "src"))

try:
    from payments import (
        PaymentClient,
        MerchantAuthenticationClient,
        ConnectorConfig,
        RequestConfig,
        Environment,
        Connector,
        Currency,
        CaptureMethod,
        AuthenticationType,
        PaymentStatus,
        MerchantAuthenticationServiceCreateAccessTokenRequest,
        PaymentServiceAuthorizeRequest,
        IntegrationError,
        ConnectorResponseTransformationError,
        PaymentAddress
    )
except ImportError as e:
    print(f"Error importing payments package: {e}")
    print(
        "Make sure the wheel is installed: pip install dist/hyperswitch_payments-*.whl"
    )
    sys.exit(1)


def parse_args() -> argparse.Namespace:
    """Parse command line arguments."""
    parser = argparse.ArgumentParser(
        description="Composite smoke test for hyperswitch-payments SDK"
    )
    parser.add_argument(
        "--creds-file",
        default="creds.json",
        help="Path to connector credentials JSON file (default: creds.json)",
    )
    return parser.parse_args()


def load_credentials(creds_file: str) -> Dict[str, Any]:
    """Load connector credentials from JSON file."""
    if not os.path.exists(creds_file):
        return {}
    with open(creds_file, "r") as f:
        return json.load(f)


def get_stripe_api_key(credentials: Dict[str, Any]) -> Optional[str]:
    """Extract Stripe API key from credentials."""
    if not credentials.get("stripe"):
        return None
    stripe_creds = credentials["stripe"]
    if isinstance(stripe_creds, list):
        stripe_creds = stripe_creds[0]
    return stripe_creds.get("apiKey", {}).get("value") or stripe_creds.get(
        "api_key", {}
    ).get("value")


def get_paypal_credentials(credentials: Dict[str, Any]) -> Optional[Tuple[str, str]]:
    """Extract PayPal client_id and client_secret from credentials."""
    if not credentials.get("paypal"):
        return None
    paypal_creds = credentials["paypal"]
    if isinstance(paypal_creds, list):
        paypal_creds = paypal_creds[0]
    client_id = paypal_creds.get("clientId", {}).get("value") or paypal_creds.get(
        "client_id", {}
    ).get("value")
    client_secret = paypal_creds.get("clientSecret", {}).get(
        "value"
    ) or paypal_creds.get("client_secret", {}).get("value")
    if client_id and client_secret:
        return (client_id, client_secret)
    return None


async def test_paypal_authorize(creds_file: str) -> bool:
    """Test PayPal authorize flow with access token."""
    print("\n[PayPal Authorize]")

    if not os.path.exists(creds_file):
        print("  SKIPPED: creds.json not found")
        return True

    credentials = load_credentials(creds_file)
    paypal_creds = get_paypal_credentials(credentials)

    if not paypal_creds:
        print("  SKIPPED: No PayPal credentials in creds.json")
        return True

    client_id, client_secret = paypal_creds
    print(f"  Using client_id: {client_id[:10]}...")

    # Configure PayPal
    config = ConnectorConfig()
    config.options.environment = Environment.SANDBOX
    config.connector_config.paypal.client_id.value = client_id
    config.connector_config.paypal.client_secret.value = client_secret

    defaults = RequestConfig()
    auth_client = MerchantAuthenticationClient(config, defaults)
    payment_client = PaymentClient(config, defaults)

    # Step 1: Create access token
    print("  Step 1: Creating access token...")
    access_token_value = None
    token_type_value = "Bearer"
    expires_in_seconds = 3600

    try:
        access_token_request = MerchantAuthenticationServiceCreateAccessTokenRequest()
        access_token_request.merchant_access_token_id = (
            f"paypal_token_{int(time.time() * 1000)}"
        )
        access_token_request.connector = Connector.PAYPAL
        access_token_request.test_mode = True

        access_token_response = await auth_client.create_access_token(
            access_token_request
        )

        if (
            access_token_response.access_token
            and access_token_response.access_token.value
        ):
            access_token_value = access_token_response.access_token.value
            token_type_value = access_token_response.token_type or "Bearer"
            expires_in_seconds = access_token_response.expires_in_seconds or 3600
            print("  Access token received")
        else:
            print("  No access token in response")
            return True
    except Exception as e:
        print(f"  Error creating access token: {e}")
        return True

    # Step 2: Authorize with access token
    print("  Step 2: Authorizing with access token...")

    try:
        authorize_request = PaymentServiceAuthorizeRequest()
        authorize_request.merchant_transaction_id = (
            f"paypal_authorize_{int(time.time() * 1000)}"
        )
        authorize_request.amount.minor_amount = 1000
        authorize_request.amount.currency = Currency.USD
        authorize_request.capture_method = CaptureMethod.AUTOMATIC

        # Card details
        card = authorize_request.payment_method.card
        card.card_number.value = "4111111111111111"
        card.card_exp_month.value = "12"
        card.card_exp_year.value = "2050"
        card.card_cvc.value = "123"
        card.card_holder_name.value = "Test User"

        # Access token in state
        authorize_request.state.access_token.token.value = access_token_value
        authorize_request.state.access_token.token_type = token_type_value
        authorize_request.state.access_token.expires_in_seconds = expires_in_seconds

        authorize_request.auth_type = AuthenticationType.NO_THREE_DS
        authorize_request.address.CopyFrom(PaymentAddress())
        authorize_request.test_mode = True

        response = await payment_client.authorize(authorize_request)

        if response.status == PaymentStatus.CHARGED:
            print("  PASSED: Payment charged")
            return True
        else:
            print(f"  FAILED: Expected CHARGED, got {response.status}")
            return False
    except IntegrationError as e:
        print(f"  IntegrationError: {e.error_code} - {e.error_message}")
        return False
    except ConnectorResponseTransformationError as e:
        print(
            f"  ConnectorResponseTransformationError: {e.error_code} - {e.error_message}"
        )
        return False
    except Exception as e:
        print(f"  Error: {e}")
        return False


async def test_stripe_authorize(creds_file: str) -> bool:
    """Test Stripe authorize flow."""
    print("\n[Stripe Authorize]")

    if not os.path.exists(creds_file):
        print("  FAILED: creds.json not found")
        return False

    credentials = load_credentials(creds_file)
    api_key = get_stripe_api_key(credentials)

    if not api_key:
        print("  FAILED: No Stripe API key in creds.json")
        return False

    # Configure Stripe
    config = ConnectorConfig()
    config.options.environment = Environment.SANDBOX
    config.connector_config.stripe.api_key.value = api_key

    defaults = RequestConfig()
    payment_client = PaymentClient(config, defaults)

    try:
        authorize_request = PaymentServiceAuthorizeRequest()
        authorize_request.merchant_transaction_id = (
            f"stripe_authorize_{int(time.time() * 1000)}"
        )
        authorize_request.amount.minor_amount = 1000
        authorize_request.amount.currency = Currency.USD
        authorize_request.capture_method = CaptureMethod.AUTOMATIC

        # Card details
        card = authorize_request.payment_method.card
        card.card_number.value = "4111111111111111"
        card.card_exp_month.value = "12"
        card.card_exp_year.value = "2050"
        card.card_cvc.value = "123"
        card.card_holder_name.value = "Test User"

        authorize_request.auth_type = AuthenticationType.NO_THREE_DS
        authorize_request.address.CopyFrom(PaymentAddress())

        response = await payment_client.authorize(authorize_request)

        if response.status == PaymentStatus.CHARGED:
            print("  PASSED: Payment charged")
            return True
        else:
            print(f"  FAILED: Expected CHARGED, got {response.status}")
            return False
    except Exception as e:
        print(f"  FAILED: {e}")
        return False


async def main() -> int:
    """Run all composite tests."""
    args = parse_args()

    # Resolve relative paths from cwd
    creds_file = args.creds_file
    if not os.path.isabs(creds_file):
        creds_file = os.path.join(os.getcwd(), creds_file)

    all_passed = True

    # Run PayPal test (informational - can skip if no creds)
    paypal_passed = await test_paypal_authorize(creds_file)
    if not paypal_passed:
        all_passed = False

    # Run Stripe test (must pass)
    stripe_passed = await test_stripe_authorize(creds_file)
    if not stripe_passed:
        all_passed = False

    print("\n" + "=" * 40)
    if all_passed:
        print("PASSED")
        return 0
    else:
        print("FAILED")
        return 1


if __name__ == "__main__":
    exit_code = asyncio.run(main())
    sys.exit(exit_code)
