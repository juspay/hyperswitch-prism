"""Unit tests for language renderers."""

from __future__ import annotations

import sys
from pathlib import Path

# Add parent directory to path for imports
sys.path.insert(0, str(Path(__file__).parent.parent))

from renderers import (
    RENDERERS,
    get_renderer,
    LanguageRenderer,
    ScenarioSpec,
    _SchemaDB,
)
from renderers.base_renderer import BaseRenderer


class TestLazyLoading:
    """Tests for lazy loading functionality."""

    def test_lazy_renderers_dict_interface(self):
        """Test that _LazyRenderersDict provides dict-like interface."""
        assert len(RENDERERS) == 4
        assert list(RENDERERS.keys()) == ["python", "javascript", "kotlin", "rust"]
        assert "python" in RENDERERS
        assert "unknown" not in RENDERERS

    def test_renderer_instantiation_on_access(self):
        """Test that renderers are instantiated on first access."""
        from renderers import _RENDERER_CACHE
        
        # Clear cache to test fresh instantiation
        original_cache = dict(_RENDERER_CACHE)
        _RENDERER_CACHE.clear()
        
        try:
            # First access should create instance
            renderer = RENDERERS["python"]
            assert isinstance(renderer, LanguageRenderer)
            assert "python" in _RENDERER_CACHE
            
            # Second access should return cached instance
            renderer2 = RENDERERS["python"]
            assert renderer is renderer2
        finally:
            # Restore cache
            _RENDERER_CACHE.update(original_cache)

    def test_get_renderer_function(self):
        """Test the get_renderer convenience function."""
        renderer = get_renderer("rust")
        assert isinstance(renderer, LanguageRenderer)
        assert renderer.lang == "rust"
        assert renderer.extension == ".rs"

    def test_iteration(self):
        """Test iterating over RENDERERS."""
        langs = list(RENDERERS)
        assert langs == ["python", "javascript", "kotlin", "rust"]

    def test_items_iteration(self):
        """Test items() method."""
        items = list(RENDERERS.items())
        assert len(items) == 4
        for lang, renderer in items:
            assert isinstance(renderer, LanguageRenderer)
            assert renderer.lang == lang


class TestBaseRenderer:
    """Tests for BaseRenderer shared functionality."""

    def test_base_renderer_is_abstract(self):
        """Test that BaseRenderer inherits from LanguageRenderer."""
        renderer = get_renderer("python")
        assert isinstance(renderer, BaseRenderer)

    def test_collect_flows_from_scenarios(self):
        """Test _collect_flows_from_scenarios method."""
        renderer = get_renderer("python")
        
        # Create test scenario
        scenario = ScenarioSpec(
            key="checkout_card",
            title="Card Payment",
            flows=["authorize", "capture"],
            pm_key="Card",
            description="Test scenario",
            status_handling={},
        )
        
        flow_metadata = {
            "authorize": {"service_name": "PaymentService", "grpc_request": "AuthRequest"},
            "capture": {"service_name": "PaymentService", "grpc_request": "CaptureRequest"},
        }
        
        service_names, flow_keys = renderer._collect_flows_from_scenarios(
            [(scenario, {})],
            [],
            flow_metadata,
        )
        
        assert "PaymentService" in service_names
        assert "authorize" in flow_keys
        assert "capture" in flow_keys

    def test_should_use_builder(self):
        """Test _should_use_builder method."""
        renderer = get_renderer("python")
        has_builder = {"authorize", "capture"}
        
        # Should use builder for card scenarios
        assert renderer._should_use_builder("authorize", "checkout_card", has_builder)
        
        # Should not use builder if not in has_builder
        assert not renderer._should_use_builder("authorize", "checkout_card", set())
        
        # Should use builder for standalone flow (no scenario)
        assert renderer._should_use_builder("capture", None, has_builder)

    def test_get_capture_method_for_scenario(self):
        """Test _get_capture_method_for_scenario method."""
        renderer = get_renderer("python")
        
        assert renderer._get_capture_method_for_scenario("checkout_card") == "MANUAL"
        assert renderer._get_capture_method_for_scenario("refund") == "AUTOMATIC"
        assert renderer._get_capture_method_for_scenario("unknown") == "AUTOMATIC"


class TestRendererProperties:
    """Tests for basic renderer properties."""

    def test_all_renderers_have_required_properties(self):
        """Test that all renderers have lang and extension properties."""
        expected = {
            "python": ".py",
            "javascript": ".js",
            "kotlin": ".kt",
            "rust": ".rs",
        }
        
        for lang, ext in expected.items():
            renderer = get_renderer(lang)
            assert renderer.lang == lang, f"{lang}: lang property mismatch"
            assert renderer.extension == ext, f"{lang}: extension property mismatch"

    def test_all_renderers_have_required_methods(self):
        """Test that all renderers have required abstract methods."""
        required_methods = [
            "config_snippet",
            "payload_lines",
            "scenario_step",
            "scenario_return",
            "render_scenario",
            "render_flow",
            "render_consolidated",
        ]
        
        for lang in RENDERERS:
            renderer = get_renderer(lang)
            for method in required_methods:
                assert hasattr(renderer, method), f"{lang}: missing {method}"
                assert callable(getattr(renderer, method)), f"{lang}: {method} not callable"


class TestConfigSnippets:
    """Tests for config_snippet generation."""

    def test_python_config_snippet(self):
        """Test Python config snippet generation."""
        renderer = get_renderer("python")
        snippet = renderer.config_snippet("stripe")
        
        assert "from payments import PaymentClient" in snippet
        assert "PaymentClient(config)" in snippet
        assert "stripe" in snippet.lower() or "YOUR_" in snippet

    def test_javascript_config_snippet(self):
        """Test JavaScript config snippet generation."""
        renderer = get_renderer("javascript")
        snippet = renderer.config_snippet("stripe")
        
        assert "require('hyperswitch-prism')" in snippet
        assert "PaymentClient" in snippet

    def test_kotlin_config_snippet(self):
        """Test Kotlin config snippet generation."""
        renderer = get_renderer("kotlin")
        snippet = renderer.config_snippet("stripe")
        
        assert "import payments.PaymentClient" in snippet
        assert "ConnectorConfig.newBuilder()" in snippet

    def test_rust_config_snippet(self):
        """Test Rust config snippet generation."""
        renderer = get_renderer("rust")
        snippet = renderer.config_snippet("stripe")
        
        assert "use" in snippet
        assert "ConnectorClient" in snippet


class TestScenarioReturn:
    """Tests for scenario_return method."""

    def test_checkout_card_return(self):
        """Test return statement for checkout_card scenario."""
        renderer = get_renderer("python")
        scenario = ScenarioSpec(
            key="checkout_card",
            title="Card Payment",
            flows=["authorize", "capture"],
            pm_key="Card",
            description="Test",
            status_handling={},
        )
        
        result = renderer.scenario_return(scenario, [])
        assert len(result) > 0
        assert "capture_response" in result[0]
        assert "authorize_response" in result[0]

    def test_refund_return(self):
        """Test return statement for refund scenario."""
        renderer = get_renderer("python")
        scenario = ScenarioSpec(
            key="refund",
            title="Refund",
            flows=["authorize", "refund"],
            pm_key=None,
            description="Test",
            status_handling={},
        )
        
        result = renderer.scenario_return(scenario, [])
        assert "refund_response" in result[0]


class TestIntegration:
    """Integration tests."""

    def test_render_config_section_integration(self):
        """Test that render_config_section works with lazy renderers."""
        from generate import render_config_section
        
        result = render_config_section("stripe")
        assert isinstance(result, list)
        assert len(result) > 0
        assert "## SDK Configuration" in result

    def test_all_languages_generate_non_empty_snippets(self):
        """Test that all languages generate non-empty config snippets."""
        for lang in RENDERERS:
            renderer = get_renderer(lang)
            snippet = renderer.config_snippet("test_connector")
            assert len(snippet) > 0, f"{lang}: empty config snippet"
            assert isinstance(snippet, str), f"{lang}: snippet not a string"


if __name__ == "__main__":
    import pytest
    
    # Run tests with pytest if available, otherwise use simple test runner
    try:
        pytest.main([__file__, "-v"])
    except ImportError:
        print("pytest not available, running basic tests...")
        
        # Run basic tests
        test_classes = [
            TestLazyLoading,
            TestBaseRenderer,
            TestRendererProperties,
            TestConfigSnippets,
            TestScenarioReturn,
            TestIntegration,
        ]
        
        for test_class in test_classes:
            print(f"\n{test_class.__name__}:")
            instance = test_class()
            for name in dir(instance):
                if name.startswith("test_"):
                    try:
                        getattr(instance, name)()
                        print(f"  ✓ {name}")
                    except Exception as e:
                        print(f"  ✗ {name}: {e}")
        
        print("\nAll basic tests completed!")
