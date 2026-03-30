"""Base classes for language renderers."""

from __future__ import annotations

from abc import ABC, abstractmethod
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from ._shared import _SchemaDB


class LanguageRenderer(ABC):
    """Abstract base class defining the contract for language-specific renderers."""

    @property
    @abstractmethod
    def lang(self) -> str:
        """Language identifier: 'python', 'go', 'ruby', etc."""

    @property
    @abstractmethod
    def extension(self) -> str:
        """File extension: '.py', '.go', '.rb', etc."""

    @abstractmethod
    def config_snippet(self, connector_name: str) -> str:
        """SDK setup snippet for docs table."""

    @abstractmethod
    def render_consolidated(self, connector_name: str, scenarios_with_payloads,
                            flow_metadata: dict, message_schemas: dict,
                            flow_items=None) -> str:
        """Generate consolidated file with all scenarios."""


class BaseRenderer(LanguageRenderer):
    """Base renderer with common implementation.
    
    Subclasses only need to implement:
    - lang, extension (class attributes)
    - config_snippet() 
    - render_consolidated() (optional - can use template)
    """

    def _format_dict_literal(self, obj: dict, indent: int = 0) -> list[str]:
        """Format a dictionary as language-appropriate code lines."""
        from ._shared import _json_scalar
        lines = []
        items = list(obj.items())
        
        for idx, (key, val) in enumerate(items):
            trailing = "," if idx < len(items) - 1 else ""
            
            if isinstance(val, dict):
                lines.append(f'"{key}": {{' + trailing)
                lines.extend(self._format_dict_literal(val, indent + 1))
                lines.append("}" + trailing)
            elif isinstance(val, str):
                lines.append(f'"{key}": {_json_scalar(val)}' + trailing)
            else:
                lines.append(f'"{key}": {val}' + trailing)
        
        return ["    " * indent + line for line in lines]

    def _get_capture_method(self, scenario_key: str) -> str:
        """Get capture_method value for scenario."""
        if scenario_key in ("checkout_card", "void_payment", "get_payment"):
            return "MANUAL"
        return "AUTOMATIC"

    def _generate_file_header(self, connector_name: str) -> str:
        """Generate standard file header."""
        return f"""# Auto-generated for {connector_name}
# Language: {self.lang}
"""

    def render_consolidated(self, connector_name: str, scenarios_with_payloads,
                            flow_metadata: dict, message_schemas: dict,
                            flow_items=None) -> str:
        """Default implementation - subclasses can override for customization."""
        # This is a minimal default implementation
        # Real implementations should use templates or custom logic
        header = self._generate_file_header(connector_name)
        
        funcs = []
        for scenario, _ in scenarios_with_payloads:
            funcs.append(f"# Function: process_{scenario.key}")
        
        return header + "\n".join(funcs)
