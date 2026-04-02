"""
Configuration loader for documentation generator.

Loads YAML configuration files for connectors, payment methods, and currency overrides.
"""

import yaml
from pathlib import Path
from typing import Optional

# Cache for loaded configs
_config_cache: dict[str, dict] = {}


def _get_config_dir() -> Path:
    """Get the directory containing config files."""
    return Path(__file__).parent


def _load_yaml_config(filename: str) -> dict:
    """Load a YAML config file with caching."""
    global _config_cache
    
    if filename in _config_cache:
        return _config_cache[filename]
    
    config_path = _get_config_dir() / filename
    if not config_path.exists():
        raise FileNotFoundError(f"Config file not found: {config_path}")
    
    with open(config_path, encoding="utf-8") as f:
        config = yaml.safe_load(f)
    
    _config_cache[filename] = config
    return config


def load_connectors() -> dict[str, str]:
    """
    Load connector display names.
    
    Returns: {connector_key: display_name}
    """
    config = _load_yaml_config("connectors.yaml")
    return config.get("connectors", {})


def load_payment_methods() -> list[tuple[str, list[tuple[str, str]]]]:
    """
    Load payment methods grouped by category.
    
    Returns: [(category_name, [(pm_key, display_name), ...]), ...]
    """
    config = _load_yaml_config("payment_methods.yaml")
    result = []
    
    for category in config.get("payment_methods", []):
        category_name = category["category"]
        methods = [
            (method["key"], method["display"])
            for method in category.get("methods", [])
        ]
        result.append((category_name, methods))
    
    return result


def load_payment_methods_flat() -> dict[str, str]:
    """
    Load payment methods as a flat dict.
    
    Returns: {pm_key: display_name}
    """
    result = {}
    for category, methods in load_payment_methods():
        for pm_key, display_name in methods:
            result[pm_key] = display_name
    return result


def load_currency_overrides() -> dict[str, str]:
    """
    Load currency overrides for bank payment methods.
    
    Returns: {pm_key: currency}
    """
    config = _load_yaml_config("currency_overrides.yaml")
    return config.get("currency_overrides", {})
