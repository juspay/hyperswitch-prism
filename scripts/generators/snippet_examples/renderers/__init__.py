"""Language renderers package for SDK snippet generation.

Usage:
    from renderers import get_renderer
    renderer = get_renderer("python")
    snippet = renderer.config_snippet("stripe")

Adding a new language:
    1. Create renderers/<lang>_renderer.py with a Renderer class
    2. That's it! Auto-discovery will pick it up.

Example - create renderers/go_renderer.py:
    class Renderer(BaseRenderer):
        lang = "go"
        extension = ".go"
        
        def config_snippet(self, connector_name):
            return "..."
        
        def render_flow(self, flow_key, connector_name, proto_req, ...):
            return "..."
"""

from __future__ import annotations

import importlib
import pkgutil
from pathlib import Path
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from .base import LanguageRenderer


def _discover_renderers() -> dict[str, type]:
    """Auto-discover all renderer classes in this package.
    
    Finds all *_renderer.py files and extracts the Renderer class.
    """
    renderers = {}
    package_dir = Path(__file__).parent
    
    for module_info in pkgutil.iter_modules([str(package_dir)]):
        if module_info.name.endswith('_renderer'):
            lang = module_info.name.replace('_renderer', '')
            try:
                module = importlib.import_module(f'.{module_info.name}', package=__name__)
                if hasattr(module, 'Renderer'):
                    renderers[lang] = module.Renderer
            except ImportError:
                pass
    
    return renderers


# Auto-discover renderer classes
_RENDERER_CLASSES: dict[str, type] = _discover_renderers()

# Cache for instantiated renderers
_RENDERER_CACHE: dict[str, 'LanguageRenderer'] = {}


def get_renderer(lang: str) -> 'LanguageRenderer':
    """Get a language renderer instance (creates on first access).
    
    Renderers are lazy-loaded and cached.
    
    Args:
        lang: Language identifier (e.g., 'python', 'go', 'ruby')
        
    Returns:
        Renderer instance
        
    Raises:
        KeyError: If language not found
        
    Example:
        >>> renderer = get_renderer('python')
        >>> snippet = renderer.config_snippet('stripe')
    """
    if lang not in _RENDERER_CACHE:
        if lang not in _RENDERER_CLASSES:
            raise KeyError(f"Unknown language: {lang}. Available: {list(_RENDERER_CLASSES.keys())}")
        _RENDERER_CACHE[lang] = _RENDERER_CLASSES[lang]()
    return _RENDERER_CACHE[lang]


class _LazyRenderersDict:
    """Lazy-loading dict interface for RENDERERS."""
    
    def __init__(self):
        self._discovered = False
    
    def _ensure_discovered(self):
        """Trigger discovery if not already done."""
        global _RENDERER_CLASSES
        if not self._discovered:
            _RENDERER_CLASSES = _discover_renderers()
            self._discovered = True
    
    def __getitem__(self, key: str) -> 'LanguageRenderer':
        self._ensure_discovered()
        return get_renderer(key)
    
    def __contains__(self, key: str) -> bool:
        self._ensure_discovered()
        return key in _RENDERER_CLASSES
    
    def __iter__(self):
        self._ensure_discovered()
        return iter(_RENDERER_CLASSES.keys())
    
    def __len__(self) -> int:
        self._ensure_discovered()
        return len(_RENDERER_CLASSES)
    
    def keys(self):
        self._ensure_discovered()
        return list(_RENDERER_CLASSES.keys())
    
    def values(self):
        return [get_renderer(k) for k in self.keys()]
    
    def items(self):
        return [(k, get_renderer(k)) for k in self.keys()]
    
    def get(self, key: str, default=None):
        try:
            return self[key]
        except KeyError:
            return default


# Public API - lazy loading dict
RENDERERS = _LazyRenderersDict()

# Re-export for convenience
from .base import LanguageRenderer
from ._shared import (
    ScenarioSpec,
    _SchemaDB,
    load_proto_type_map,
    set_scenario_groups,
    _build_annotated,
    _conn_enum,
    _conn_enum_rust,
    _conn_display,
    _client_class,
    _td,
    _STEP_DESCRIPTIONS,
    _AUTHORIZE_STATUS_HANDLING,
    _AUTOCAPTURE_STATUS_HANDLING,
    _SETUP_RECURRING_STATUS_HANDLING,
    _PROBE_PM_LABELS,
    _SCENARIO_GROUPS,
    _CARD_AUTHORIZE_SCENARIOS,
)

__all__ = [
    "LanguageRenderer",
    "get_renderer",
    "RENDERERS",
    "ScenarioSpec",
    "_SchemaDB",
    "load_proto_type_map",
    "set_scenario_groups",
    "_build_annotated",
    "_conn_enum",
    "_conn_enum_rust",
    "_conn_display",
    "_client_class",
    "_td",
    "_STEP_DESCRIPTIONS",
    "_AUTHORIZE_STATUS_HANDLING",
    "_AUTOCAPTURE_STATUS_HANDLING",
    "_SETUP_RECURRING_STATUS_HANDLING",
    "_PROBE_PM_LABELS",
    "_SCENARIO_GROUPS",
    "_CARD_AUTHORIZE_SCENARIOS",
]
