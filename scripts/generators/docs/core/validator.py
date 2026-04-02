"""
Configuration Validator

Validates YAML configuration files and provides helpful error messages.
"""

import yaml
from pathlib import Path
from typing import List, Tuple, Optional, TYPE_CHECKING
from dataclasses import dataclass

if TYPE_CHECKING:
    from core.models import RenderManifest


@dataclass
class ValidationError:
    """A single validation error."""
    file: str
    path: str  # Dot-notation path to the error (e.g., "scenarios.0.flows.0.name")
    message: str
    suggestion: Optional[str] = None


class ConfigValidator:
    """Validates documentation generator configuration files."""
    
    REQUIRED_SCENARIO_FIELDS = {"key", "name", "description", "flows"}
    REQUIRED_FLOW_FIELDS = {"name", "required"}
    VALID_STATUS_ACTIONS = {"error", "return_early", "continue"}
    
    def __init__(self, config_dir: Path):
        self.config_dir = config_dir
        self.errors: List[ValidationError] = []
    
    def validate_all(self) -> Tuple[bool, List[ValidationError]]:
        """Validate all configuration files."""
        self.errors = []
        
        self._validate_scenarios()
        self._validate_connectors()
        self._validate_payment_methods()
        self._validate_status_mapping()
        
        return len(self.errors) == 0, self.errors
    
    def _validate_scenarios(self):
        """Validate scenarios.yaml."""
        scenarios_file = self.config_dir.parent / "specs" / "scenarios.yaml"
        if not scenarios_file.exists():
            self.errors.append(ValidationError(
                file="scenarios.yaml",
                path="",
                message="File not found",
                suggestion="Create the file at specs/scenarios.yaml"
            ))
            return
        
        try:
            with open(scenarios_file) as f:
                data = yaml.safe_load(f)
        except yaml.YAMLError as e:
            self.errors.append(ValidationError(
                file="scenarios.yaml",
                path="",
                message=f"Invalid YAML syntax: {e}",
                suggestion="Check for indentation errors, missing colons, or unquoted strings"
            ))
            return
        
        if not isinstance(data, dict):
            self.errors.append(ValidationError(
                file="scenarios.yaml",
                path="",
                message="Root must be a mapping (dictionary)",
                suggestion="Start the file with 'scenarios:'"
            ))
            return
        
        scenarios = data.get("scenarios", [])
        if not isinstance(scenarios, list):
            self.errors.append(ValidationError(
                file="scenarios.yaml",
                path="scenarios",
                message="'scenarios' must be a list",
                suggestion="Use '- key: value' syntax for each scenario"
            ))
            return
        
        scenario_keys = set()
        for idx, scenario in enumerate(scenarios):
            self._validate_scenario(scenario, idx, scenario_keys)
    
    def _validate_scenario(self, scenario: dict, idx: int, scenario_keys: set):
        """Validate a single scenario."""
        path_prefix = f"scenarios.{idx}"
        
        if not isinstance(scenario, dict):
            self.errors.append(ValidationError(
                file="scenarios.yaml",
                path=path_prefix,
                message="Scenario must be a mapping",
                suggestion="Each scenario should have key:, name:, description:, flows:"
            ))
            return
        
        # Check required fields
        missing = self.REQUIRED_SCENARIO_FIELDS - set(scenario.keys())
        if missing:
            self.errors.append(ValidationError(
                file="scenarios.yaml",
                path=path_prefix,
                message=f"Missing required fields: {', '.join(sorted(missing))}",
                suggestion=f"Add the missing fields: {', '.join(sorted(missing))}"
            ))
        
        # Check for duplicate keys
        key = scenario.get("key")
        if key:
            if key in scenario_keys:
                self.errors.append(ValidationError(
                    file="scenarios.yaml",
                    path=f"{path_prefix}.key",
                    message=f"Duplicate scenario key: '{key}'",
                    suggestion="Each scenario must have a unique key"
                ))
            scenario_keys.add(key)
        
        # Validate flows
        flows = scenario.get("flows", [])
        if not isinstance(flows, list):
            self.errors.append(ValidationError(
                file="scenarios.yaml",
                path=f"{path_prefix}.flows",
                message="'flows' must be a list",
                suggestion="Use '- name: flow_name' syntax for each flow"
            ))
        else:
            for flow_idx, flow in enumerate(flows):
                self._validate_flow(flow, f"{path_prefix}.flows.{flow_idx}")
        
        # Validate return_fields
        return_fields = scenario.get("return_fields", {})
        if not isinstance(return_fields, dict):
            self.errors.append(ValidationError(
                file="scenarios.yaml",
                path=f"{path_prefix}.return_fields",
                message="'return_fields' must be a mapping",
                suggestion="Use 'field_name: response.path' syntax"
            ))
    
    def _validate_flow(self, flow: dict, path: str):
        """Validate a single flow definition."""
        if not isinstance(flow, dict):
            self.errors.append(ValidationError(
                file="scenarios.yaml",
                path=path,
                message="Flow must be a mapping",
                suggestion="Flow should have name:, required:, and optionally status_handling:"
            ))
            return
        
        # Check required fields
        missing = self.REQUIRED_FLOW_FIELDS - set(flow.keys())
        if missing:
            self.errors.append(ValidationError(
                file="scenarios.yaml",
                path=path,
                message=f"Missing required fields: {', '.join(sorted(missing))}",
                suggestion="Every flow must have a 'name' and 'required' field"
            ))
        
        # Validate status_handling
        status_handling = flow.get("status_handling", [])
        if not isinstance(status_handling, list):
            self.errors.append(ValidationError(
                file="scenarios.yaml",
                path=f"{path}.status_handling",
                message="'status_handling' must be a list",
                suggestion="Use '- status: [FAILED]\n  action: error' syntax"
            ))
        else:
            for idx, rule in enumerate(status_handling):
                self._validate_status_rule(rule, f"{path}.status_handling.{idx}")
        
        # Validate use_from_previous
        use_from_previous = flow.get("use_from_previous", [])
        if not isinstance(use_from_previous, list):
            self.errors.append(ValidationError(
                file="scenarios.yaml",
                path=f"{path}.use_from_previous",
                message="'use_from_previous' must be a list",
                suggestion="Use 'use_from_previous: [field1, field2]' syntax"
            ))
        elif use_from_previous and not flow.get("depends_on"):
            self.errors.append(ValidationError(
                file="scenarios.yaml",
                path=f"{path}.use_from_previous",
                message="'use_from_previous' requires 'depends_on'",
                suggestion="Add 'depends_on: previous_flow_name' to specify which flow provides the fields"
            ))
    
    def _validate_status_rule(self, rule: dict, path: str):
        """Validate a single status handling rule."""
        if not isinstance(rule, dict):
            self.errors.append(ValidationError(
                file="scenarios.yaml",
                path=path,
                message="Status rule must be a mapping",
                suggestion="Use 'status: [FAILED]\naction: error' syntax"
            ))
            return
        
        # Validate status field
        status = rule.get("status")
        if status is None:
            self.errors.append(ValidationError(
                file="scenarios.yaml",
                path=f"{path}.status",
                message="Status rule missing 'status' field",
                suggestion="Add 'status: [FAILED, AUTHORIZATION_FAILED]'"
            ))
        elif not isinstance(status, list):
            self.errors.append(ValidationError(
                file="scenarios.yaml",
                path=f"{path}.status",
                message="'status' must be a list of strings",
                suggestion="Use 'status: [FAILED, AUTHORIZATION_FAILED]' syntax"
            ))
        
        # Validate action field
        action = rule.get("action")
        if action is None:
            self.errors.append(ValidationError(
                file="scenarios.yaml",
                path=f"{path}.action",
                message="Status rule missing 'action' field",
                suggestion=f"Add 'action: error' (valid actions: {', '.join(self.VALID_STATUS_ACTIONS)})"
            ))
        elif action not in self.VALID_STATUS_ACTIONS:
            self.errors.append(ValidationError(
                file="scenarios.yaml",
                path=f"{path}.action",
                message=f"Invalid action: '{action}'",
                suggestion=f"Valid actions are: {', '.join(self.VALID_STATUS_ACTIONS)}"
            ))
    
    def _validate_connectors(self):
        """Validate connectors.yaml."""
        connectors_file = self.config_dir / "connectors.yaml"
        if not connectors_file.exists():
            return  # Optional file
        
        try:
            with open(connectors_file) as f:
                data = yaml.safe_load(f)
        except yaml.YAMLError as e:
            self.errors.append(ValidationError(
                file="connectors.yaml",
                path="",
                message=f"Invalid YAML syntax: {e}",
                suggestion="Check YAML syntax"
            ))
            return
        
        if not isinstance(data, dict):
            self.errors.append(ValidationError(
                file="connectors.yaml",
                path="",
                message="Root must be a mapping",
                suggestion="Use 'connector_name: Display Name' format"
            ))
            return
        
        # Support both flat format and nested 'connectors:' format
        connectors = data.get("connectors", data)
        
        if not isinstance(connectors, dict):
            self.errors.append(ValidationError(
                file="connectors.yaml",
                path="connectors",
                message="'connectors' must be a mapping",
                suggestion="Use 'connector_name: Display Name' format"
            ))
            return
        
        for key, value in connectors.items():
            if not isinstance(value, str):
                self.errors.append(ValidationError(
                    file="connectors.yaml",
                    path=f"connectors.{key}",
                    message=f"Value must be a string, got {type(value).__name__}",
                    suggestion="Use 'connector_name: Display Name' format"
                ))
    
    def _validate_payment_methods(self):
        """Validate payment_methods.yaml."""
        pm_file = self.config_dir / "payment_methods.yaml"
        if not pm_file.exists():
            return  # Optional file
        
        try:
            with open(pm_file) as f:
                data = yaml.safe_load(f)
        except yaml.YAMLError as e:
            self.errors.append(ValidationError(
                file="payment_methods.yaml",
                path="",
                message=f"Invalid YAML syntax: {e}",
                suggestion="Check YAML syntax"
            ))
            return
        
        if not isinstance(data, dict):
            self.errors.append(ValidationError(
                file="payment_methods.yaml",
                path="",
                message="Root must be a mapping",
                suggestion="File should have 'categories:' at root"
            ))
            return
        
        categories = data.get("categories", [])
        if not isinstance(categories, list):
            self.errors.append(ValidationError(
                file="payment_methods.yaml",
                path="categories",
                message="'categories' must be a list",
                suggestion="Use category list format"
            ))
    
    def _validate_status_mapping(self):
        """Validate status_mapping.yaml."""
        mapping_file = self.config_dir / "status_mapping.yaml"
        if not mapping_file.exists():
            return  # Optional file
        
        try:
            with open(mapping_file) as f:
                data = yaml.safe_load(f)
        except yaml.YAMLError as e:
            self.errors.append(ValidationError(
                file="status_mapping.yaml",
                path="",
                message=f"Invalid YAML syntax: {e}",
                suggestion="Check YAML syntax"
            ))
            return
        
        if not isinstance(data, dict):
            self.errors.append(ValidationError(
                file="status_mapping.yaml",
                path="",
                message="Root must be a mapping",
                suggestion="File should have language mappings at root"
            ))
            return


def format_validation_errors(errors: List[ValidationError]) -> str:
    """Format validation errors for display."""
    if not errors:
        return "✓ All configuration files are valid!"
    
    lines = [f"✗ Found {len(errors)} validation error(s):\n"]
    
    # Group by file
    by_file = {}
    for error in errors:
        by_file.setdefault(error.file, []).append(error)
    
    for file, file_errors in sorted(by_file.items()):
        lines.append(f"\n{file}")
        lines.append("=" * 60)
        
        for error in file_errors:
            if error.path:
                lines.append(f"\n  Path: {error.path}")
            lines.append(f"  Error: {error.message}")
            if error.suggestion:
                lines.append(f"  Suggestion: {error.suggestion}")
    
    return "\n".join(lines)


def validate_configs(config_dir: Path) -> bool:
    """Validate all configs and print results."""
    validator = ConfigValidator(config_dir)
    is_valid, errors = validator.validate_all()

    print(format_validation_errors(errors))

    return is_valid


def validate_structural_parity(manifests: "List[RenderManifest]") -> List[str]:
    """
    Compare render manifests across languages.

    Catches: "JS emitted 2 status checks for authorize, Rust emitted 3"
    Does NOT require AST parsing — just compares counts from the renderers.

    Returns list of error strings. Empty list means all languages are consistent.
    """
    errors: List[str] = []
    if len(manifests) < 2:
        return errors

    first = manifests[0]
    for other in manifests[1:]:
        if other.flow_count != first.flow_count:
            errors.append(
                f"{other.language} has {other.flow_count} flows, "
                f"{first.language} has {first.flow_count} "
                f"(scenario: {first.scenario_key})"
            )

        if other.status_checks != first.status_checks:
            for flow_name in set(first.status_checks) | set(other.status_checks):
                a = first.status_checks.get(flow_name, 0)
                b = other.status_checks.get(flow_name, 0)
                if a != b:
                    errors.append(
                        f"{other.language} has {b} status checks for '{flow_name}', "
                        f"{first.language} has {a} "
                        f"(scenario: {first.scenario_key})"
                    )

        if other.return_field_count != first.return_field_count:
            errors.append(
                f"{other.language} returns {other.return_field_count} fields, "
                f"{first.language} returns {first.return_field_count} "
                f"(scenario: {first.scenario_key})"
            )

        if sorted(other.cross_flow_refs) != sorted(first.cross_flow_refs):
            errors.append(
                f"{other.language} cross-flow refs {sorted(other.cross_flow_refs)} != "
                f"{first.language} {sorted(first.cross_flow_refs)} "
                f"(scenario: {first.scenario_key})"
            )

        if other.prerequisite_count != first.prerequisite_count:
            errors.append(
                f"{other.language} has {other.prerequisite_count} prerequisite flows, "
                f"{first.language} has {first.prerequisite_count} "
                f"(scenario: {first.scenario_key})"
            )

    return errors
