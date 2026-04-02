"""
Scenario Coverage Report

Generates reports showing which connectors support which scenarios.
"""

import json
from pathlib import Path
from typing import Dict, List, Set, Optional
from dataclasses import dataclass, field
from collections import defaultdict

import yaml


@dataclass
class CoverageEntry:
    """Coverage for a single scenario on a single connector."""
    scenario_key: str
    connector_name: str
    supported: bool
    missing_flows: List[str] = field(default_factory=list)
    available_flows: List[str] = field(default_factory=list)
    prerequisite_flows: List[str] = field(default_factory=list)
    
    @property
    def status(self) -> str:
        if self.supported:
            return "✓"
        elif self.available_flows:
            return "~"  # Partial
        return "✗"


@dataclass  
class ConnectorCoverage:
    """Coverage report for a single connector."""
    connector_name: str
    display_name: str
    total_scenarios: int
    supported: int
    partial: int
    unsupported: int
    entries: List[CoverageEntry]
    
    @property
    def coverage_pct(self) -> float:
        if self.total_scenarios == 0:
            return 0.0
        return (self.supported / self.total_scenarios) * 100


@dataclass
class ScenarioCoverage:
    """Coverage report for a single scenario across all connectors."""
    scenario_key: str
    scenario_name: str
    total_connectors: int
    supported: int
    partial: int
    unsupported: int
    connectors: List[str]  # List of connector names that support it


class CoverageReporter:
    """Generate scenario coverage reports."""
    
    def __init__(self, probe_path: Path, scenarios_path: Path):
        self.probe_path = probe_path
        self.scenarios_path = scenarios_path
        self.probe_data = self._load_probe_data()
        self.scenarios = self._load_scenarios()
        
    def _load_probe_data(self) -> Dict[str, dict]:
        """Load all probe data files."""
        data = {}
        if not self.probe_path.exists():
            return data
            
        for file_path in self.probe_path.glob("*.json"):
            if file_path.stem == "manifest":
                continue
            try:
                with open(file_path) as f:
                    data[file_path.stem] = json.load(f)
            except Exception as e:
                print(f"Warning: Failed to load {file_path}: {e}")
        return data
    
    def _load_scenarios(self) -> List[dict]:
        """Load scenarios from YAML."""
        try:
            with open(self.scenarios_path) as f:
                return yaml.safe_load(f).get("scenarios", [])
        except Exception as e:
            print(f"Warning: Failed to load scenarios: {e}")
            return []
    
    def _get_flows_from_probe(self, probe: dict) -> Set[str]:
        """Extract available flows from probe data.
        
        In the new probe format, flows is a dict where keys are flow names.
        If a flow exists in the dict, it's considered supported.
        """
        flows = set()
        
        # New format: flows dict
        flows_data = probe.get("flows", {})
        for flow_key in flows_data.keys():
            flows.add(flow_key)
        
        # Legacy format: results dict (for backward compatibility)
        results = probe.get("results", {})
        for flow_key, flow_data in results.items():
            if isinstance(flow_data, dict):
                status = flow_data.get("status", "")
                if status == "supported":
                    flows.add(flow_key)
            elif isinstance(flow_data, list):
                # Payment method variants
                for variant in flow_data:
                    if variant.get("status") == "supported":
                        flows.add(flow_key)
                        break
        
        return flows
    
    def _check_scenario_support(
        self, 
        scenario: dict, 
        available_flows: Set[str],
        connector_name: str
    ) -> CoverageEntry:
        """Check if a scenario is supported by a connector."""
        required_flows = []
        missing = []
        available = []
        
        for flow in scenario.get("flows", []):
            flow_name = flow.get("name", "")
            is_required = flow.get("required", True)
            
            if is_required:
                required_flows.append(flow_name)
                if flow_name in available_flows:
                    available.append(flow_name)
                else:
                    missing.append(flow_name)
        
        # Check for prerequisites in flow_graph
        prereqs = []
        flow_graph = self.probe_data.get(connector_name, {}).get("flow_graph", {})
        nodes = flow_graph.get("nodes", {})
        
        for flow_name in required_flows:
            if flow_name in nodes:
                node = nodes[flow_name]
                for req_field, req_info in node.get("requires", {}).items():
                    prereq_flow = req_info.get("from_flow", "")
                    if prereq_flow and prereq_flow not in required_flows:
                        prereqs.append(prereq_flow)
        
        supported = len(missing) == 0
        
        return CoverageEntry(
            scenario_key=scenario.get("key", ""),
            connector_name=connector_name,
            supported=supported,
            missing_flows=missing,
            available_flows=available,
            prerequisite_flows=list(set(prereqs))
        )
    
    def generate_connector_report(self, connector_name: str) -> Optional[ConnectorCoverage]:
        """Generate coverage report for a single connector."""
        probe = self.probe_data.get(connector_name)
        if not probe:
            return None
        
        available_flows = self._get_flows_from_probe(probe)
        display_name = probe.get("display_name", connector_name)
        
        entries = []
        supported = partial = unsupported = 0
        
        for scenario in self.scenarios:
            entry = self._check_scenario_support(scenario, available_flows, connector_name)
            entries.append(entry)
            
            if entry.supported:
                supported += 1
            elif entry.available_flows:
                partial += 1
            else:
                unsupported += 1
        
        return ConnectorCoverage(
            connector_name=connector_name,
            display_name=display_name,
            total_scenarios=len(self.scenarios),
            supported=supported,
            partial=partial,
            unsupported=unsupported,
            entries=entries
        )
    
    def generate_all_connectors_report(self) -> List[ConnectorCoverage]:
        """Generate coverage report for all connectors."""
        reports = []
        for connector_name in sorted(self.probe_data.keys()):
            report = self.generate_connector_report(connector_name)
            if report:
                reports.append(report)
        return reports
    
    def generate_scenario_report(self, scenario_key: str) -> Optional[ScenarioCoverage]:
        """Generate coverage report for a single scenario across all connectors."""
        scenario = None
        for s in self.scenarios:
            if s.get("key") == scenario_key:
                scenario = s
                break
        
        if not scenario:
            return None
        
        supported_connectors = []
        supported = partial = unsupported = 0
        
        for connector_name, probe in self.probe_data.items():
            available_flows = self._get_flows_from_probe(probe)
            entry = self._check_scenario_support(scenario, available_flows, connector_name)
            
            if entry.supported:
                supported += 1
                supported_connectors.append(connector_name)
            elif entry.available_flows:
                partial += 1
            else:
                unsupported += 1
        
        return ScenarioCoverage(
            scenario_key=scenario_key,
            scenario_name=scenario.get("name", scenario_key),
            total_connectors=len(self.probe_data),
            supported=supported,
            partial=partial,
            unsupported=unsupported,
            connectors=supported_connectors
        )
    
    def generate_summary(self) -> str:
        """Generate a text summary of coverage across all connectors."""
        reports = self.generate_all_connectors_report()
        
        if not reports:
            return "No coverage data available."
        
        lines = [
            "=" * 80,
            "SCENARIO COVERAGE REPORT",
            "=" * 80,
            "",
            f"Connectors: {len(reports)}",
            f"Scenarios: {len(self.scenarios)}",
            "",
            "OVERALL COVERAGE",
            "-" * 80,
        ]
        
        # Global stats
        total_supported = sum(r.supported for r in reports)
        total_possible = len(reports) * len(self.scenarios)
        overall_pct = (total_supported / total_possible * 100) if total_possible > 0 else 0
        
        lines.append(f"Overall Coverage: {overall_pct:.1f}% ({total_supported}/{total_possible})")
        lines.append("")
        
        # Per-scenario coverage
        lines.append("PER-SCENARIO COVERAGE")
        lines.append("-" * 80)
        
        for scenario in self.scenarios:
            scenario_key = scenario.get("key", "")
            report = self.generate_scenario_report(scenario_key)
            if report:
                pct = (report.supported / report.total_connectors * 100) if report.total_connectors > 0 else 0
                lines.append(
                    f"  {scenario_key:30} {report.supported:3}/{report.total_connectors:3} connectors ({pct:5.1f}%)"
                )
        
        lines.append("")
        lines.append("TOP CONNECTORS BY COVERAGE")
        lines.append("-" * 80)
        
        # Sort by coverage percentage
        sorted_reports = sorted(reports, key=lambda r: r.coverage_pct, reverse=True)
        for report in sorted_reports[:10]:
            lines.append(
                f"  {report.connector_name:20} {report.supported:2}/{report.total_scenarios:2} scenarios ({report.coverage_pct:5.1f}%)"
            )
        
        lines.append("")
        lines.append("CONNECTORS NEEDING ATTENTION")
        lines.append("-" * 80)
        
        # Low coverage connectors
        low_coverage = [r for r in reports if r.coverage_pct < 50]
        if low_coverage:
            for report in sorted(low_coverage, key=lambda r: r.coverage_pct)[:5]:
                lines.append(
                    f"  {report.connector_name:20} {report.supported:2}/{report.total_scenarios:2} scenarios ({report.coverage_pct:5.1f}%)"
                )
        else:
            lines.append("  All connectors have good coverage!")
        
        lines.append("")
        lines.append("=" * 80)
        
        return "\n".join(lines)
    
    def generate_markdown_report(self) -> str:
        """Generate a markdown coverage report."""
        reports = self.generate_all_connectors_report()
        
        if not reports:
            return "# Scenario Coverage Report\n\nNo coverage data available."
        
        lines = [
            "# Scenario Coverage Report",
            "",
            f"**Connectors:** {len(reports)}  ",
            f"**Scenarios:** {len(self.scenarios)}  ",
            "",
            "## Summary",
            "",
        ]
        
        # Global stats
        total_supported = sum(r.supported for r in reports)
        total_possible = len(reports) * len(self.scenarios)
        overall_pct = (total_supported / total_possible * 100) if total_possible > 0 else 0
        
        lines.append(f"**Overall Coverage:** {overall_pct:.1f}% ({total_supported}/{total_possible})")
        lines.append("")
        
        # Per-scenario coverage table
        lines.append("## Coverage by Scenario")
        lines.append("")
        lines.append("| Scenario | Supported | Coverage |")
        lines.append("|----------|-----------|----------|")
        
        for scenario in self.scenarios:
            scenario_key = scenario.get("key", "")
            report = self.generate_scenario_report(scenario_key)
            if report:
                pct = (report.supported / report.total_connectors * 100) if report.total_connectors > 0 else 0
                lines.append(
                    f"| {scenario.get('name', scenario_key)} | {report.supported}/{report.total_connectors} | {pct:.1f}% |"
                )
        
        lines.append("")
        
        # Connector matrix
        lines.append("## Connector Coverage Matrix")
        lines.append("")
        lines.append("| Connector | " + " | ".join(s.get("key", "")[:12] for s in self.scenarios) + " |")
        lines.append("|" + "-" * 12 + "|" + "|".join("-" * 14 for _ in self.scenarios) + "|")
        
        for report in sorted(reports, key=lambda r: r.coverage_pct, reverse=True):
            row = [f"| {report.connector_name:10}"]
            for scenario in self.scenarios:
                entry = next((e for e in report.entries if e.scenario_key == scenario.get("key", "")), None)
                if entry:
                    row.append(f" {entry.status:12} |")
                else:
                    row.append(f" {'?':12} |")
            lines.append("".join(row))
        
        lines.append("")
        lines.append("**Legend:** ✓ = Fully Supported, ~ = Partial, ✗ = Not Supported")
        lines.append("")
        
        return "\n".join(lines)
