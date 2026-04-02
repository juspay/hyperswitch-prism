"""
Tests for the hydrator module.

Verifies that scenarios are correctly hydrated with probe data.
"""

import sys
from pathlib import Path

# Add parent directory to path
sys.path.insert(0, str(Path(__file__).parent.parent))

from core.models import Scenario, FlowDefinition, StatusRule, StatusAction, FlowAvailability
from core.hydrator import ScenarioHydrator


class TestScenarioHydrator:
    """Test suite for ScenarioHydrator."""
    
    def test_hydrate_simple_scenario(self):
        """Test hydrating a simple scenario with one flow."""
        probe_data = {
            "flows": {
                "authorize": {
                    "Card": {
                        "status": "supported",
                        "proto_request": {"amount": {"value": 100}}
                    }
                }
            }
        }
        
        scenario = Scenario(
            key="test_card",
            name="Test Card",
            description="Test scenario",
            flows=[
                FlowDefinition(
                    name="authorize",
                    pm_type="Card",
                    required=True,
                )
            ],
        )
        
        hydrator = ScenarioHydrator(probe_data, "test_connector")
        result = hydrator.hydrate(scenario)
        
        assert result is not None
        assert result.key == "test_card"
        assert result.connector_name == "test_connector"
        assert len(result.flows) == 1
        assert result.flows[0].payload == {"amount": {"value": 100}}
    
    def test_hydrate_returns_none_for_missing_required_flow(self):
        """Test that hydration returns None if a required flow is missing."""
        probe_data = {
            "flows": {}  # No flows supported
        }
        
        scenario = Scenario(
            key="test_card",
            name="Test Card",
            description="Test scenario",
            flows=[
                FlowDefinition(
                    name="authorize",
                    pm_type="Card",
                    required=True,
                )
            ],
        )
        
        hydrator = ScenarioHydrator(probe_data, "test_connector")
        result = hydrator.hydrate(scenario)
        
        assert result is None
    
    def test_hydrate_skips_optional_missing_flow(self):
        """Test that optional missing flows are skipped but scenario still works."""
        probe_data = {
            "flows": {
                "authorize": {
                    "Card": {
                        "status": "supported",
                        "proto_request": {}
                    }
                }
            }
        }
        
        scenario = Scenario(
            key="test_card",
            name="Test Card",
            description="Test scenario",
            flows=[
                FlowDefinition(
                    name="authorize",
                    pm_type="Card",
                    required=True,
                ),
                FlowDefinition(
                    name="capture",
                    required=False,  # Optional
                )
            ],
        )
        
        hydrator = ScenarioHydrator(probe_data, "test_connector")
        result = hydrator.hydrate(scenario)
        
        assert result is not None
        assert result.availability == FlowAvailability.PARTIALLY_SUPPORTED
        assert "capture" in result.skipped_optional
    
    def test_pm_variant_resolution(self):
        """Test resolving PM variants in order."""
        probe_data = {
            "flows": {
                "authorize": {
                    "GooglePay": {
                        "status": "supported",
                        "proto_request": {"payment_method": "google_pay"}
                    },
                    "ApplePay": {
                        "status": "not_supported",
                        "proto_request": {}
                    }
                }
            }
        }
        
        scenario = Scenario(
            key="test_wallet",
            name="Test Wallet",
            description="Test scenario",
            flows=[
                FlowDefinition(
                    name="authorize",
                    pm_type_variants=["ApplePay", "GooglePay"],  # ApplePay first but not supported
                    required=True,
                )
            ],
        )
        
        hydrator = ScenarioHydrator(probe_data, "test_connector")
        result = hydrator.hydrate(scenario)
        
        assert result is not None
        assert result.flows[0].payload == {"payment_method": "google_pay"}


def run_all_tests():
    """Run all hydrator tests."""
    print("Running hydrator tests...\n")
    
    test_class = TestScenarioHydrator()
    passed = 0
    failed = 0
    
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
        pytest.main([__file__, "-v"])
    except ImportError:
        success = run_all_tests()
        sys.exit(0 if success else 1)
