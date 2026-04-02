"""
Integration tests for the full documentation generation pipeline.

Tests the complete flow: probe data → hydration → rendering.
"""

import json
import tempfile
from pathlib import Path

import pytest

# Add parent to path
import sys
sys.path.insert(0, str(Path(__file__).parent.parent))

from core.models import Scenario
from core.hydrator import ScenarioHydrator
from renderers.python import PythonRenderer
from renderers.javascript import JavaScriptRenderer


@pytest.fixture
def sample_probe_data():
    """Minimal probe data for testing (matches real probe format)."""
    return {
        "connector": "test_connector",
        "display_name": "Test Connector",
        "flows": {
            "authorize": {
                # PM variants as keys
                "Card": {
                    "status": "supported",
                    "proto_request": {
                        "amount": 100,
                        "currency": "USD",
                        "payment_method_data": {"card": {"number": "4111111111111111"}}
                    }
                }
            },
            "capture": {
                # Non-PM-aware flows use "default"
                "default": {
                    "status": "supported",
                    "proto_request": {
                        "connector_transaction_id": "txn_123",
                        "amount": 100
                    }
                }
            }
        },
        "flow_graph": {
            "nodes": {
                "authorize": {
                    "node_type": "entry_point",
                    "provides": {
                        "connector_transaction_id": {
                            "response_path": "connector_transaction_id",
                            "description": "Transaction ID"
                        }
                    }
                },
                "capture": {
                    "node_type": "dependent",
                    "requires": {
                        "connector_transaction_id": {
                            "from_flow": "authorize",
                            "from_field": "connector_transaction_id",
                            "request_path": "connector_transaction_id"
                        }
                    }
                }
            },
            "edges": [
                {"from": "authorize", "to": "capture"}
            ]
        }
    }


@pytest.fixture
def sample_scenario():
    """Simple checkout scenario."""
    return Scenario(
        key="checkout_card",
        name="Card Checkout",
        description="Standard card payment",
        flows=[
            {
                "name": "authorize",
                "required": True,
                "pm_type_variants": ["Card"],
                "status_handling": [
                    {"status": ["FAILED"], "action": "error", "message": "Payment failed"}
                ]
            },
            {
                "name": "capture",
                "required": False,
                "depends_on": "authorize",
                "use_from_previous": ["connector_transaction_id"]
            }
        ],
        return_fields={
            "status": "response.status",
            "transaction_id": "response.connector_transaction_id"
        }
    )


class TestFullPipeline:
    """Integration tests for the complete pipeline."""
    
    def test_hydrate_scenario(self, sample_probe_data, sample_scenario):
        """Test scenario hydration with probe data."""
        # Hydrate
        hydrator = ScenarioHydrator(sample_probe_data, "test_connector")
        hydrated = hydrator.hydrate(sample_scenario)
        
        assert hydrated is not None
        assert hydrated.key == "checkout_card"
        assert hydrated.name == "Card Checkout"
        assert hydrated.connector_name == "test_connector"
        assert len(hydrated.flows) == 2
        
        # Check flows have payloads
        auth_flow = next(f for f in hydrated.flows if f.name == "authorize")
        assert auth_flow.payload is not None
        assert auth_flow.payload.get("amount") == 100
        
        # Check field links were resolved for capture
        capture_flow = next(f for f in hydrated.flows if f.name == "capture")
        assert "connector_transaction_id" in capture_flow.field_links
        link = capture_flow.field_links["connector_transaction_id"]
        assert link.from_flow == "authorize"
        assert link.from_field == "connector_transaction_id"
        
    def test_hydrate_preserves_status_handling(self, sample_probe_data, sample_scenario):
        """Test that status handling rules are preserved during hydration."""
        hydrator = ScenarioHydrator(sample_probe_data, "test_connector")
        hydrated = hydrator.hydrate(sample_scenario)
        
        auth_flow = next(f for f in hydrated.flows if f.name == "authorize")
        assert len(auth_flow.status_handling) == 1
        rule = auth_flow.status_handling[0]
        assert rule.action.value == "error"
        assert "FAILED" in rule.status
        
    def test_missing_required_flow_returns_none(self, sample_probe_data):
        """Scenario with missing required flow should return None."""
        scenario = Scenario(
            key="refund_payment",
            name="Refund Payment",
            description="Refund a payment",
            flows=[
                {
                    "name": "refund",
                    "required": True,
                    "status_handling": []
                }
            ],
            return_fields={}
        )
        
        hydrator = ScenarioHydrator(sample_probe_data, "test_connector")
        hydrated = hydrator.hydrate(scenario)
        
        # refund flow doesn't exist in probe data
        assert hydrated is None
        
    def test_probe_data_roundtrip(self, sample_probe_data, tmp_path):
        """Test that probe data can be saved and loaded correctly."""
        # Write probe data
        probe_dir = tmp_path / "probes"
        probe_dir.mkdir()
        probe_file = probe_dir / "test_connector.json"
        with open(probe_file, 'w') as f:
            json.dump(sample_probe_data, f)
        
        # Load and verify structure
        with open(probe_file) as f:
            data = json.load(f)
        
        assert data["connector"] == "test_connector"
        assert "flows" in data
        assert "flow_graph" in data
        
        # Verify we can hydrate with loaded data
        scenario = Scenario(
            key="checkout_card",
            name="Card Checkout",
            description="Test",
            flows=[
                {"name": "authorize", "required": True, "pm_type_variants": ["Card"], "status_handling": []},
                {"name": "capture", "required": False, "depends_on": "authorize", "use_from_previous": ["connector_transaction_id"], "status_handling": []}
            ],
            return_fields={"status": "response.status"}
        )
        
        hydrator = ScenarioHydrator(data, "test_connector")
        hydrated = hydrator.hydrate(scenario)
        
        assert hydrated is not None
        assert len(hydrated.flows) == 2
