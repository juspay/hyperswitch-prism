#!/usr/bin/env python3
"""
Card Payment (Authorize + Capture) - Universal Example

Works with any connector that supports card payments.
Usage: python checkout_card.py --connector=stripe
"""

import argparse
import asyncio
import json
from pathlib import Path

# Add SDK path (adjust based on your setup)
import sys
sys.path.insert(0, str(Path(__file__).parent.parent.parent / "sdk/python/src"))

from payments import PaymentClient
from payments.generated import sdk_config_pb2, payment_pb2


def load_connector_config(connector_name: str) -> dict:
    """Load connector-specific configuration from probe data."""
    probe_file = Path(__file__).parent.parent.parent / "data/field_probe" / f"{connector_name}.json"
    with open(probe_file) as f:
        return json.load(f)


def build_auth_config(connector_name: str, credentials: dict) -> sdk_config_pb2.ConnectorConfig:
    """Build connector config with appropriate auth."""
    config = sdk_config_pb2.ConnectorConfig(
        connector=connector_name,
        options=sdk_config_pb2.SdkOptions(environment=sdk_config_pb2.Environment.SANDBOX),
    )
    
    # Map connector names to their auth config types
    auth_mapping = {
        "stripe": payment_pb2.StripeConfig,
        "adyen": payment_pb2.AdyenConfig,
        "checkout": payment_pb2.CheckoutConfig,
        # ... add others as needed
    }
    
    if connector_name in auth_mapping:
        auth_class = auth_mapping[connector_name]
        # Build auth config from credentials dict
        auth_fields = {}
        for key, value in credentials.items():
            if hasattr(auth_class, key):
                auth_fields[key] = value
        
        connector_specific = payment_pb2.ConnectorSpecificConfig()
        getattr(connector_specific, connector_name).CopyFrom(auth_class(**auth_fields))
        config.connector_config.CopyFrom(connector_specific)
    
    return config


def build_authorize_request(connector_data: dict) -> dict:
    """Build authorize request from probe data."""
    # Get the first supported card example from probe data
    authorize_flows = connector_data.get("flows", {}).get("authorize", {})
    
    # Find Card or first supported payment method
    card_data = None
    for pm_key, pm_data in authorize_flows.items():
        if pm_data.get("status") == "supported":
            if pm_key == "Card" or card_data is None:
                card_data = pm_data
                if pm_key == "Card":
                    break
    
    if not card_data:
        raise ValueError(f"Connector {connector_data['connector']} doesn't support authorize flow")
    
    proto_request = card_data.get("proto_request", {})
    
    # Ensure capture_method is MANUAL for this scenario
    proto_request["capture_method"] = "MANUAL"
    
    return proto_request


async def process_checkout_card(connector_name: str, credentials: dict) -> str:
    """Execute authorize + capture flow."""
    # Load connector probe data
    connector_data = load_connector_config(connector_name)
    
    # Build config
    config = build_auth_config(connector_name, credentials)
    client = PaymentClient(config)
    
    # Build request from probe data
    auth_request_dict = build_authorize_request(connector_data)
    
    print(f"\n{'='*60}")
    print(f"Processing Card Payment via {connector_name}")
    print(f"{'='*60}")
    print(f"Request: {json.dumps(auth_request_dict, indent=2)}")
    
    # Step 1: Authorize
    print("\n[1/2] Authorizing...")
    auth_response = await client.authorize(auth_request_dict)
    
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
    capture_request = {
        "merchant_capture_id": f"capture_{auth_request_dict['merchant_transaction_id']}",
        "connector_transaction_id": auth_response.connector_transaction_id,
        "amount_to_capture": auth_request_dict["amount"],
    }
    
    capture_response = await client.capture(capture_request)
    print(f"Capture Status: {capture_response.status}")
    
    if capture_response.status in ["CAPTURED", "AUTHORIZED"]:
        print("✅ Payment captured successfully")
        return "success"
    else:
        print(f"⚠️  Capture returned: {capture_response.status}")
        return "partial"


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
        # Use dummy credentials for demo
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
