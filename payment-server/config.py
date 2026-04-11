"""
Connector configuration for PayPal and Cybersource.

Reads credentials from a JSON file and builds SDK ConnectorConfig objects.
"""

import json
from google.protobuf.json_format import ParseDict
from payments.generated import sdk_config_pb2, payment_pb2


def load_credentials(creds_path: str) -> dict:
    with open(creds_path, "r") as f:
        return json.load(f)


def build_paypal_config(creds: dict) -> sdk_config_pb2.ConnectorConfig:
    """Build PayPal connector config from credentials.

    PayPal creds use auth_type=body-key:
      api_key -> client_secret
      key1    -> client_id
    """
    paypal_creds = creds["paypal"]["connector_account_details"]

    config = sdk_config_pb2.ConnectorConfig(
        options=sdk_config_pb2.SdkOptions(
            environment=sdk_config_pb2.Environment.SANDBOX
        ),
    )
    connector_config = ParseDict(
        {
            "paypal": {
                "client_id": {"value": paypal_creds["key1"]},
                "client_secret": {"value": paypal_creds["api_key"]},
            }
        },
        payment_pb2.ConnectorSpecificConfig(),
    )
    config.connector_config.CopyFrom(connector_config)
    return config


def build_cybersource_config(creds: dict) -> sdk_config_pb2.ConnectorConfig:
    """Build Cybersource connector config from credentials.

    Cybersource creds use auth_type=signature-key:
      api_key    -> api_key
      key1       -> merchant_account
      api_secret -> api_secret
    """
    cs_creds = creds["cybersource"]["connector_1"]["connector_account_details"]

    config = sdk_config_pb2.ConnectorConfig(
        options=sdk_config_pb2.SdkOptions(
            environment=sdk_config_pb2.Environment.SANDBOX
        ),
    )
    connector_config = ParseDict(
        {
            "cybersource": {
                "api_key": {"value": cs_creds["api_key"]},
                "merchant_account": {"value": cs_creds["key1"]},
                "api_secret": {"value": cs_creds["api_secret"]},
            }
        },
        payment_pb2.ConnectorSpecificConfig(),
    )
    config.connector_config.CopyFrom(connector_config)
    return config
