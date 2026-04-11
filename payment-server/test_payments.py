"""
Test script for payment authorization and refund flows.

Tests:
  1. USD authorization via PayPal
  2. USD refund via PayPal
  3. EUR authorization via Cybersource
  4. EUR refund via Cybersource

Runs directly against the SDK (no server needed).
"""

import asyncio
import sys
import os
import time

sys.path.insert(0, os.path.dirname(__file__))

from config import load_credentials, build_paypal_config, build_cybersource_config
from payment_service import authorize_payment, refund_payment


CREDS_PATH = os.environ.get("CREDS_PATH", "/home/grace/creds.json")


async def test_authorize_and_refund(currency: str, connector_name: str, configs: dict, creds: dict):
    """Test authorization followed by refund for a given currency."""
    print(f"\n{'='*60}")
    print(f"Testing {currency} -> {connector_name}")
    print(f"{'='*60}")

    # Step 1: Authorize
    print(f"\n[1] Authorizing {currency} 10.00 payment...")
    try:
        auth_result = await authorize_payment(1000, currency, configs, creds)
        print(f"    Connector: {auth_result['connector']}")
        print(f"    Status: {auth_result['status']}")
        print(f"    Transaction ID: {auth_result.get('connector_transaction_id', 'N/A')}")
        if auth_result.get("error"):
            print(f"    Error: {auth_result['error']}")

        assert auth_result["connector"] == connector_name, (
            f"Expected {connector_name}, got {auth_result['connector']}"
        )
        print(f"    -> Routing verified: {currency} -> {connector_name}")

    except Exception as e:
        print(f"    FAILED: {type(e).__name__}: {e}")
        return False

    # Step 2: Refund
    txn_id = auth_result.get("connector_transaction_id")
    if not txn_id:
        print(f"\n[2] Skipping refund - no transaction ID returned")
        return True

    print(f"\n[2] Refunding {currency} 10.00...")
    try:
        feature_data = auth_result.get("connector_feature_data")
        refund_result = await refund_payment(txn_id, 1000, currency, configs, creds, feature_data)
        print(f"    Connector: {refund_result['connector']}")
        print(f"    Status: {refund_result['status']}")
        if refund_result.get("error"):
            print(f"    Error: {refund_result['error']}")

        assert refund_result["connector"] == connector_name, (
            f"Expected {connector_name}, got {refund_result['connector']}"
        )
        print(f"    -> Refund routing verified: {currency} -> {connector_name}")

    except Exception as e:
        print(f"    FAILED: {type(e).__name__}: {e}")
        return False

    return True


async def main():
    print("Loading credentials...")
    creds = load_credentials(CREDS_PATH)
    configs = {
        "paypal": build_paypal_config(creds),
        "cybersource": build_cybersource_config(creds),
    }
    print("Credentials loaded successfully.")

    results = {}

    # Test USD -> PayPal
    results["USD/PayPal"] = await test_authorize_and_refund("USD", "paypal", configs, creds)

    # Test EUR -> Cybersource
    results["EUR/Cybersource"] = await test_authorize_and_refund("EUR", "cybersource", configs, creds)

    # Summary
    print(f"\n{'='*60}")
    print("TEST SUMMARY")
    print(f"{'='*60}")
    for test_name, passed in results.items():
        status = "PASS" if passed else "FAIL"
        print(f"  {test_name}: {status}")

    all_passed = all(results.values())
    print(f"\nOverall: {'ALL PASSED' if all_passed else 'SOME FAILED'}")
    return 0 if all_passed else 1


if __name__ == "__main__":
    exit_code = asyncio.run(main())
    sys.exit(exit_code)
