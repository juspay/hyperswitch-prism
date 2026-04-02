"""
Tests for configuration loading.

Verifies that YAML configs are loaded correctly and match expected structure.
"""

import sys
from pathlib import Path
import pytest

# Add parent directory to path
sys.path.insert(0, str(Path(__file__).parent.parent))

from config import load_connectors, load_payment_methods, load_payment_methods_flat, load_currency_overrides


class TestConnectorsConfig:
    """Test suite for connectors.yaml loading."""
    
    def test_load_connectors_returns_dict(self):
        """Verify load_connectors returns a dictionary."""
        connectors = load_connectors()
        assert isinstance(connectors, dict)
    
    def test_load_connectors_has_expected_connectors(self):
        """Verify expected connectors are present."""
        connectors = load_connectors()
        expected = ["stripe", "adyen", "razorpay", "braintree", "checkout"]
        for conn in expected:
            assert conn in connectors, f"Expected connector '{conn}' not found"
    
    def test_load_connectors_has_display_names(self):
        """Verify all connectors have display names."""
        connectors = load_connectors()
        for key, name in connectors.items():
            assert isinstance(key, str)
            assert isinstance(name, str)
            assert len(name) > 0, f"Connector '{key}' has empty display name"
    
    def test_stripe_display_name(self):
        """Verify specific connector display name."""
        connectors = load_connectors()
        assert connectors.get("stripe") == "Stripe"
    
    def test_adyen_display_name(self):
        """Verify specific connector display name."""
        connectors = load_connectors()
        assert connectors.get("adyen") == "Adyen"


class TestPaymentMethodsConfig:
    """Test suite for payment_methods.yaml loading."""
    
    def test_load_payment_methods_returns_list(self):
        """Verify load_payment_methods returns a list."""
        methods = load_payment_methods()
        assert isinstance(methods, list)
        assert len(methods) > 0
    
    def test_load_payment_methods_has_categories(self):
        """Verify payment methods are grouped by category."""
        methods = load_payment_methods()
        categories = [cat for cat, _ in methods]
        expected_categories = ["Card", "Wallet", "BNPL", "Bank Transfer"]
        for cat in expected_categories:
            assert cat in categories, f"Expected category '{cat}' not found"
    
    def test_load_payment_methods_flat_returns_dict(self):
        """Verify load_payment_methods_flat returns a flat dictionary."""
        methods = load_payment_methods_flat()
        assert isinstance(methods, dict)
        assert len(methods) > 0
    
    def test_flat_methods_have_keys_and_display(self):
        """Verify flat methods have both key and display name."""
        methods = load_payment_methods_flat()
        for key, display in methods.items():
            assert isinstance(key, str)
            assert isinstance(display, str)
            assert len(key) > 0
            assert len(display) > 0
    
    def test_specific_payment_method_exists(self):
        """Verify specific payment methods exist."""
        methods = load_payment_methods_flat()
        assert "Card" in methods
        assert "ApplePay" in methods
        assert "GooglePay" in methods
        assert "Sepa" in methods
    
    def test_payment_method_display_names(self):
        """Verify payment method display names are correct."""
        methods = load_payment_methods_flat()
        assert methods.get("Card") == "Card"
        assert methods.get("ApplePay") == "Apple Pay"
        assert methods.get("GooglePay") == "Google Pay"


class TestCurrencyOverridesConfig:
    """Test suite for currency_overrides.yaml loading."""
    
    def test_load_currency_overrides_returns_dict(self):
        """Verify load_currency_overrides returns a dictionary."""
        overrides = load_currency_overrides()
        assert isinstance(overrides, dict)
    
    def test_currency_overrides_have_expected_values(self):
        """Verify expected currency overrides exist."""
        overrides = load_currency_overrides()
        assert overrides.get("Sepa") == "EUR"
        assert overrides.get("Bacs") == "GBP"
        assert overrides.get("Becs") == "AUD"
    
    def test_currency_overrides_keys_are_pm_keys(self):
        """Verify override keys are payment method keys."""
        overrides = load_currency_overrides()
        payment_methods = load_payment_methods_flat()
        
        for pm_key in overrides.keys():
            assert pm_key in payment_methods, f"Override key '{pm_key}' is not a valid payment method"


class TestConfigCaching:
    """Test suite for config caching behavior."""
    
    def test_configs_are_cached(self):
        """Verify configs are cached after first load."""
        # Load configs twice
        connectors1 = load_connectors()
        connectors2 = load_connectors()
        
        # Should be the same object (cached)
        assert connectors1 is connectors2
    
    def test_all_configs_load_without_errors(self):
        """Verify all config files load without raising exceptions."""
        try:
            load_connectors()
            load_payment_methods()
            load_payment_methods_flat()
            load_currency_overrides()
        except Exception as e:
            pytest.fail(f"Config loading failed: {e}")


if __name__ == "__main__":
    # Run tests with pytest if available, otherwise run basic assertions
    try:
        import pytest
        pytest.main([__file__, "-v"])
    except ImportError:
        print("pytest not available, running basic tests...")
        
        # Run basic tests
        test_conn = TestConnectorsConfig()
        test_conn.test_load_connectors_returns_dict()
        test_conn.test_load_connectors_has_expected_connectors()
        print("✓ Connector config tests passed")
        
        test_pm = TestPaymentMethodsConfig()
        test_pm.test_load_payment_methods_returns_list()
        test_pm.test_load_payment_methods_flat_returns_dict()
        print("✓ Payment method config tests passed")
        
        test_curr = TestCurrencyOverridesConfig()
        test_curr.test_load_currency_overrides_returns_dict()
        test_curr.test_currency_overrides_have_expected_values()
        print("✓ Currency override config tests passed")
        
        print("\nAll basic tests passed!")
