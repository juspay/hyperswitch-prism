#!/usr/bin/env python3
"""
Card Payment (Authorize + Capture) - Universal Example

Usage:
    python checkout_card.py --connector=stripe
    python checkout_card.py --connector=adyen
    python checkout_card.py --connector=checkout
"""

import argparse
import asyncio
import json
from pathlib import Path
from typing import Optional

# [START imports]
from google.protobuf.json_format import ParseDict
from payments.generated import sdk_config_pb2, payment_pb2
from payments import PaymentClient
# [END imports]


# [START load_probe_data]
def load_probe_data(connector_name: str) -> dict:
    """Load connector probe data from field_probe."""
    probe_file = Path(__file__).parent.parent.parent / "data" / "field_probe" / f"{connector_name}.json"
    with open(probe_file) as f:
        return json.load(f)
# [END load_probe_data]


# [START stripe_config]
def get_stripe_config(api_key: str) -> sdk_config_pb2.ConnectorConfig:
    """Configuration for Stripe."""
    config = sdk_config_pb2.ConnectorConfig(
        options=sdk_config_pb2.SdkOptions(environment=sdk_config_pb2.Environment.SANDBOX),
    )
    config.connector_config.CopyFrom(payment_pb2.ConnectorSpecificConfig(
        stripe=payment_pb2.StripeConfig(api_key=api_key),
    ))
    return config
# [END stripe_config]


# [START adyen_config]
def get_adyen_config(api_key: str, merchant_account: str) -> sdk_config_pb2.ConnectorConfig:
    """Configuration for Adyen."""
    config = sdk_config_pb2.ConnectorConfig(
        options=sdk_config_pb2.SdkOptions(environment=sdk_config_pb2.Environment.SANDBOX),
    )
    config.connector_config.CopyFrom(payment_pb2.ConnectorSpecificConfig(
        adyen=payment_pb2.AdyenConfig(
            api_key=api_key,
            merchant_account=merchant_account,
        ),
    ))
    return config
# [END adyen_config]


# [START checkout_config]
def get_checkout_config(api_key: str) -> sdk_config_pb2.ConnectorConfig:
    """Configuration for Checkout.com."""
    config = sdk_config_pb2.ConnectorConfig(
        options=sdk_config_pb2.SdkOptions(environment=sdk_config_pb2.Environment.SANDBOX),
    )
    config.connector_config.CopyFrom(payment_pb2.ConnectorSpecificConfig(
        checkout=payment_pb2.CheckoutConfig(api_key=api_key),
    ))
    return config
# [END checkout_config]


# [START build_authorize_request]
def build_authorize_request(probe_data: dict, capture_method: str = "MANUAL") -> dict:
    """Build authorize request from probe data."""
    flows = probe_data.get("flows", {})
    authorize_flows = flows.get("authorize", {})
    
    # Find Card payment method or first supported
    card_data = None
    for pm_key, pm_data in authorize_flows.items():
        if pm_data.get("status") == "supported":
            if pm_key == "Card":
                card_data = pm_data
                break
            elif card_data is None:
                card_data = pm_data
    
    if not card_data:
        raise ValueError("No supported payment method found for authorize flow")
    
    proto_request = dict(card_data.get("proto_request", {}))
    proto_request["capture_method"] = capture_method
    
    return proto_request
# [END build_authorize_request]


# [START build_capture_request]
def build_capture_request(
    connector_transaction_id: str,
    amount: dict,
    merchant_capture_id: str = "capture_001"
) -> dict:
    """Build capture request."""
    return {
        "merchant_capture_id": merchant_capture_id,
        "connector_transaction_id": connector_transaction_id,
        "amount_to_capture": amount,
    }
# [END build_capture_request]


# [START process_checkout_card]
async def process_checkout_card(
    connector_name: str,
    credentials: dict
) -> str:
    """
    Process a card payment with authorize + capture flow.
    
    Args:
        connector_name: Name of the connector (stripe, adyen, etc.)
        credentials: Connector-specific credentials
    
    Returns:
        "success" | "pending" | "failed"
    """
    print(f"\n{'='*60}")
    print(f"Processing Card Payment via {connector_name}")
    print(f"{'='*60}")
    
    # Load probe data for this connector
    probe_data = load_probe_data(connector_name)
    
    # Get configuration
    config = get_connector_config(connector_name, credentials)
    client = PaymentClient(config)
    
    # Build authorize request
    auth_request_dict = build_authorize_request(probe_data, capture_method="MANUAL")
    auth_request = ParseDict(auth_request_dict, payment_pb2.PaymentServiceAuthorizeRequest())
    
    print(f"\nRequest: {json.dumps(auth_request_dict, indent=2)}")
    
    # Step 1: Authorize
    print("\n[1/2] Authorizing...")
    auth_response = await client.authorize(auth_request)
    
    print(f"Status: {auth_response.status}")
    print(f"Connector Transaction ID: {auth_response.connector_transaction_id}")
    
    # Handle status
    if auth_response.status == "FAILED":
        print(f"❌ Payment declined: {auth_response.error_message}")
        return "failed"
    
    if auth_response.status == "PENDING":
        print("⏳ Payment pending - awaiting async confirmation")
        print("   (In production: wait for webhook, then poll get())")
        return "pending"
    
    if auth_response.status != "AUTHORIZED":
        print(f"⚠️  Unexpected status: {auth_response.status}")
        return "error"
    
    print("✅ Funds reserved successfully")
    
    # Step 2: Capture
    print("\n[2/2] Capturing...")
    capture_request_dict = build_capture_request(
        connector_transaction_id=auth_response.connector_transaction_id,
        amount=auth_request_dict["amount"],
    )
    capture_request = ParseDict(capture_request_dict, payment_pb2.PaymentServiceCaptureRequest())
    
    capture_response = await client.capture(capture_request)
    print(f"Capture Status: {capture_response.status}")
    
    if capture_response.status in ["CAPTURED", "AUTHORIZED"]:
        print("✅ Payment captured successfully")
        return "success"
    else:
        print(f"⚠️  Capture returned: {capture_response.status}")
        return "partial"
# [END process_checkout_card]


# [START get_connector_config]
def get_connector_config(connector_name: str, credentials: dict) -> sdk_config_pb2.ConnectorConfig:
    """Get configuration for any supported connector."""
    config_functions = {
        "stripe": lambda: get_stripe_config(credentials.get("api_key")),
        "adyen": lambda: get_adyen_config(
            credentials.get("api_key"),
            credentials.get("merchant_account")
        ),
        "checkout": lambda: get_checkout_config(credentials.get("api_key")),
    }
    
    if connector_name not in config_functions:
        raise ValueError(f"Unknown connector: {connector_name}")
    
    return config_functions[connector_name]()
# [END get_connector_config]


# [START main]
def main():
    parser = argparse.ArgumentParser(description="Card Payment (Authorize + Capture)")
    parser.add_argument(
        "--connector",
        required=True,
        help="Connector name (stripe, adyen, checkout, etc.)"
    )
    parser.add_argument(
        "--credentials",
        help="JSON file with connector credentials"
    )
    
    args = parser.parse_args()
    
    # Load credentials
    if args.credentials:
        with open(args.credentials) as f:
            credentials = json.load(f)
    else:
        credentials = {"api_key": "sk_test_dummy"}
        print("⚠️  Using dummy credentials. Set --credentials for real API calls.")
    
    # Run the flow
    result = asyncio.run(process_checkout_card(args.connector, credentials))
    
    print(f"\n{'='*60}")
    print(f"Result: {result}")
    print(f"{'='*60}")
    
    return 0 if result in ["success", "pending"] else 1


if __name__ == "__main__":
    exit(main())
# [END main]
