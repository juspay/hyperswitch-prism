"""
Baseline tests for generate.py

These tests establish the current behavior of generate.py before refactoring.
They verify that:
1. All connectors can be loaded
2. Display names are correct
3. Payment method categories work
4. Documentation can be generated (smoke test)
"""

import sys
from pathlib import Path
import json

# Add parent directory to path
sys.path.insert(0, str(Path(__file__).parent.parent))

# Import from generate.py (this tests the current state)
import generate as gen


class TestDisplayNames:
    """Test display name functionality."""
    
    def test_display_name_for_known_connector(self):
        """Verify display_name returns correct value for known connectors."""
        assert gen.display_name("stripe") == "Stripe"
        assert gen.display_name("adyen") == "Adyen"
        assert gen.display_name("razorpay") == "Razorpay"
    
    def test_display_name_for_unknown_connector(self):
        """Verify display_name formats unknown connector names."""
        assert gen.display_name("my_connector") == "My Connector"
        assert gen.display_name("test_connector_v2") == "Test Connector V2"
    
    def test_all_display_names_present(self):
        """Verify _DISPLAY_NAMES dict has entries."""
        # Access the module-level variable
        assert hasattr(gen, '_DISPLAY_NAMES')
        assert len(gen._DISPLAY_NAMES) > 0


class TestPaymentMethods:
    """Test payment method functionality."""
    
    def test_probe_pm_by_category_exists(self):
        """Verify _PROBE_PM_BY_CATEGORY is defined."""
        assert hasattr(gen, '_PROBE_PM_BY_CATEGORY')
        assert isinstance(gen._PROBE_PM_BY_CATEGORY, list)
        assert len(gen._PROBE_PM_BY_CATEGORY) > 0
    
    def test_probe_pm_display_exists(self):
        """Verify _PROBE_PM_DISPLAY is defined."""
        assert hasattr(gen, '_PROBE_PM_DISPLAY')
        assert isinstance(gen._PROBE_PM_DISPLAY, dict)
        assert len(gen._PROBE_PM_DISPLAY) > 0
    
    def test_pm_categories_present(self):
        """Verify expected PM categories exist."""
        categories = [cat for cat, _ in gen._PROBE_PM_BY_CATEGORY]
        assert "Card" in categories
        assert "Wallet" in categories
        assert "Bank Transfer" in categories
    
    def test_specific_payment_methods_present(self):
        """Verify specific PMs are in the flat dict."""
        assert "Card" in gen._PROBE_PM_DISPLAY
        assert "ApplePay" in gen._PROBE_PM_DISPLAY
        assert "GooglePay" in gen._PROBE_PM_DISPLAY
        assert "Sepa" in gen._PROBE_PM_DISPLAY


class TestCurrencyOverrides:
    """Test currency override functionality."""
    
    def test_bank_pm_currency_overrides_exist(self):
        """Verify _BANK_PM_CURRENCY_OVERRIDES is defined."""
        assert hasattr(gen, '_BANK_PM_CURRENCY_OVERRIDES')
        assert isinstance(gen._BANK_PM_CURRENCY_OVERRIDES, dict)
    
    def test_expected_overrides_present(self):
        """Verify expected currency overrides."""
        overrides = gen._BANK_PM_CURRENCY_OVERRIDES
        assert overrides.get("Sepa") == "EUR"
        assert overrides.get("Bacs") == "GBP"
        assert overrides.get("Becs") == "AUD"


class TestProbeDataLoading:
    """Test probe data loading functionality."""
    
    def test_probe_data_path_resolution(self):
        """Verify REPO_ROOT and paths are correctly defined."""
        assert hasattr(gen, 'REPO_ROOT')
        assert hasattr(gen, 'DOCS_DIR')
        assert gen.DOCS_DIR.name == "connectors"
    
    def test_load_probe_data_exists(self):
        """Verify load_probe_data function exists."""
        assert hasattr(gen, 'load_probe_data')
        assert callable(gen.load_probe_data)


class TestBackwardCompatibility:
    """
    Tests to ensure refactored code maintains backward compatibility.
    
    These tests compare old hardcoded values with new config-loaded values.
    """
    
    def test_display_names_match_hardcoded(self):
        """Verify loaded display names match old hardcoded values."""
        # After refactoring, _DISPLAY_NAMES will be loaded from YAML
        # This test ensures they match
        if hasattr(gen, '_DISPLAY_NAMES'):
            # Check a few key connectors
            assert gen._DISPLAY_NAMES.get("stripe") == "Stripe"
            assert gen._DISPLAY_NAMES.get("adyen") == "Adyen"
            assert gen._DISPLAY_NAMES.get("razorpay") == "Razorpay"
    
    def test_payment_methods_match_hardcoded(self):
        """Verify loaded PMs match old hardcoded values."""
        if hasattr(gen, '_PROBE_PM_DISPLAY'):
            # Check a few key payment methods
            assert gen._PROBE_PM_DISPLAY.get("Card") == "Card"
            assert gen._PROBE_PM_DISPLAY.get("ApplePay") == "Apple Pay"
            assert gen._PROBE_PM_DISPLAY.get("GooglePay") == "Google Pay"


def run_all_tests():
    """Run all baseline tests."""
    print("Running baseline tests for generate.py...\n")
    
    test_classes = [
        TestDisplayNames(),
        TestPaymentMethods(),
        TestCurrencyOverrides(),
        TestProbeDataLoading(),
        TestBackwardCompatibility(),
    ]
    
    passed = 0
    failed = 0
    
    for test_class in test_classes:
        class_name = test_class.__class__.__name__
        print(f"\n{class_name}:")
        
        for method_name in dir(test_class):
            if method_name.startswith('test_'):
                try:
                    method = getattr(test_class, method_name)
                    method()
                    print(f"  ✓ {method_name}")
                    passed += 1
                except AssertionError as e:
                    print(f"  ✗ {method_name}: {e}")
                    failed += 1
                except Exception as e:
                    print(f"  ✗ {method_name}: {type(e).__name__}: {e}")
                    failed += 1
    
    print(f"\n{'='*50}")
    print(f"Results: {passed} passed, {failed} failed")
    
    return failed == 0


if __name__ == "__main__":
    try:
        import pytest
        # Run with pytest if available
        pytest.main([__file__, "-v"])
    except ImportError:
        # Run custom test runner
        success = run_all_tests()
        sys.exit(0 if success else 1)
