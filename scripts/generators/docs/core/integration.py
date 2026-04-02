"""
Integration module connecting new architecture to generate.py pipeline.

This module provides a bridge between the old generate.py code and the new
refactored architecture:
- Loads scenarios from YAML
- Hydrates them with probe data
- Renders code for all 4 languages
- Returns data in format expected by existing markdown generator
"""

from pathlib import Path
from typing import Dict, List, Optional
import yaml
import json

from core.models import Scenario, HydratedScenario
from core.hydrator import ScenarioHydrator
from renderers import PythonRenderer, JavaScriptRenderer, KotlinRenderer, RustRenderer


def load_status_mapping(config_path: Path) -> Dict[str, Dict[str, str]]:
    """Load status mapping from YAML."""
    with open(config_path / "status_mapping.yaml", 'r') as f:
        return yaml.safe_load(f)


def load_scenarios(specs_path: Path) -> List[Scenario]:
    """Load scenarios from YAML."""
    with open(specs_path / "scenarios.yaml", 'r') as f:
        data = yaml.safe_load(f)
    return [Scenario(**s) for s in data.get('scenarios', [])]


def render_all_languages(
    hydrated: HydratedScenario,
    status_mapping: Dict[str, Dict[str, str]]
) -> Dict[str, str]:
    """
    Render hydrated scenario in all 4 languages.
    
    Returns: {"python": "...", "javascript": "...", "kotlin": "...", "rust": "..."}
    """
    return {
        'python': PythonRenderer(hydrated, status_mapping['python']).render(),
        'javascript': JavaScriptRenderer(hydrated, status_mapping['javascript']).render(),
        'kotlin': KotlinRenderer(hydrated, status_mapping['kotlin']).render(),
        'rust': RustRenderer(hydrated, status_mapping['rust']).render(),
    }


def generate_scenario_examples(
    connector_name: str,
    probe_data: dict,
    base_path: Path
) -> Dict[str, Dict[str, str]]:
    """
    Generate examples for all supported scenarios for a connector.
    
    Returns: {scenario_key: {language: code}}
    """
    # Load scenarios and status mapping
    scenarios = load_scenarios(base_path / "specs")
    status_mapping = load_status_mapping(base_path / "config")
    
    # Hydrate all scenarios
    hydrator = ScenarioHydrator(probe_data, connector_name)
    results = {}
    
    for scenario in scenarios:
        hydrated = hydrator.hydrate(scenario)
        if hydrated:
            # Render in all languages
            results[scenario.key] = render_all_languages(hydrated, status_mapping)
    
    return results


# Legacy compatibility - function signature matching old API
def get_scenario_examples(
    connector_name: str,
    probe_data: dict,
    base_path: Optional[Path] = None
) -> Dict[str, Dict[str, str]]:
    """
    Legacy-compatible function for generating scenario examples.
    
    This replaces the old _scenario_step_* and _scenario_return_* functions
    with the new refactored architecture.
    """
    if base_path is None:
        base_path = Path(__file__).parent.parent
    
    return generate_scenario_examples(connector_name, probe_data, base_path)
