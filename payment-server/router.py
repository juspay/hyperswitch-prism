"""
Currency-based payment router.

Routes payments to the correct connector based on currency:
  USD -> PayPal
  EUR -> Cybersource
"""

from payments.generated import sdk_config_pb2


CURRENCY_CONNECTOR_MAP = {
    "USD": "paypal",
    "EUR": "cybersource",
}


def get_connector_for_currency(currency: str) -> str:
    """Return connector name for the given currency, or raise ValueError."""
    currency = currency.upper()
    connector = CURRENCY_CONNECTOR_MAP.get(currency)
    if not connector:
        supported = ", ".join(CURRENCY_CONNECTOR_MAP.keys())
        raise ValueError(
            f"Unsupported currency: {currency}. Supported: {supported}"
        )
    return connector


def get_config_for_currency(
    currency: str,
    configs: dict[str, sdk_config_pb2.ConnectorConfig],
) -> tuple[str, sdk_config_pb2.ConnectorConfig]:
    """Return (connector_name, config) for the given currency."""
    connector = get_connector_for_currency(currency)
    return connector, configs[connector]
