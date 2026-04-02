"""
Scenario Hydrator

Combines abstract scenarios with field-probe data to produce hydrated scenarios.
Handles:
- Flow resolution (including PM variants)
- Prerequisite discovery from flow_graph
- Field link resolution from flow_graph
- Missing flow handling
"""

from typing import Optional, List, Dict, Any
from pathlib import Path
import json

from core.models import (
    Scenario, FlowDefinition, StatusRule, FlowAvailability,
    HydratedScenario, HydratedFlow, FieldLink,
    FlowGraph, FlowNode, FlowNodeType
)


class ScenarioHydrator:
    """
    Hydrates scenarios with connector-specific probe data.
    
    Takes abstract scenarios and fills them with:
    - Actual request payloads from probe data
    - Prerequisites from flow_graph
    - Field links from flow_graph requires/provides
    """
    
    def __init__(self, probe_data: dict, connector_name: str):
        self.probe = probe_data
        self.connector_name = connector_name
        self.flow_graph = self._load_flow_graph()
    
    def _load_flow_graph(self) -> Optional[FlowGraph]:
        """Load flow graph from probe data if available."""
        flow_graph_data = self.probe.get("flow_graph")
        if not flow_graph_data:
            return None
        
        try:
            return FlowGraph(**flow_graph_data)
        except Exception as e:
            print(f"Warning: Failed to parse flow_graph for {self.connector_name}: {e}")
            return None
    
    def hydrate(self, scenario: Scenario) -> Optional[HydratedScenario]:
        """
        Fill scenario with probe payloads.
        
        Returns None if required flows are missing (scenario not supported).
        Returns HydratedScenario on success.
        """
        hydrated_flows = []
        skipped_optional = []
        
        for flow in scenario.flows:
            flow_data = self._resolve_flow_data(flow)
            
            if not flow_data:
                if flow.required:
                    return None  # Required flow missing — skip entire scenario
                skipped_optional.append(flow.name)
                continue
            
            if flow_data.get("status") != "supported":
                if flow.required:
                    return None
                skipped_optional.append(flow.name)
                continue
            
            # Build hydrated flow
            hydrated_flow = HydratedFlow(
                name=flow.name,
                payload=flow_data.get("proto_request", {}),
                status_handling=flow.status_handling,
                depends_on=flow.depends_on,
                use_from_previous=flow.use_from_previous,
                capture_method=flow.capture_method,
                is_prerequisite=False,
            )
            
            # Resolve field links from flow_graph
            hydrated_flow.field_links = self._resolve_field_links(flow.name)
            
            hydrated_flows.append(hydrated_flow)
        
        # If no flows could be hydrated, scenario is not supported
        if not hydrated_flows:
            return None
        
        # Determine availability
        if skipped_optional:
            availability = FlowAvailability.PARTIALLY_SUPPORTED
        else:
            availability = FlowAvailability.SUPPORTED
        
        # Resolve prerequisites from flow_graph
        prerequisite_flows = self._resolve_prerequisites(scenario)
        
        return HydratedScenario(
            key=scenario.key,
            name=scenario.name,
            description=scenario.description,
            connector_name=self.connector_name,
            prerequisite_flows=prerequisite_flows,
            flows=hydrated_flows,
            return_fields=scenario.return_fields,
            availability=availability,
            skipped_optional=skipped_optional,
        )
    
    def _resolve_flow_data(self, flow: FlowDefinition) -> Optional[dict]:
        """
        Resolve flow data from probe, trying PM variants if specified.
        
        Returns the first supported flow data found, or None if not supported.
        """
        flows = self.probe.get("flows", {})
        flow_entry = flows.get(flow.name, {})
        
        if not flow_entry:
            return None
        
        # If specific PM type specified, use that
        if flow.pm_type:
            return flow_entry.get(flow.pm_type)
        
        # If PM variants specified, try each in order
        if flow.pm_type_variants:
            for variant in flow.pm_type_variants:
                data = flow_entry.get(variant)
                if data and data.get("status") == "supported" and data.get("proto_request"):
                    return data
            return None
        
        # Otherwise use "default"
        return flow_entry.get("default")
    
    def _resolve_prerequisites(self, scenario: Scenario) -> List[HydratedFlow]:
        """
        Walk the flow_graph to find prerequisite flows needed by this scenario.
        
        Example: For checkout_card (authorize → capture) on paypal,
        discovers that authorize requires create_access_token.
        Returns [HydratedFlow(name="create_access_token", ...)]
        """
        if not self.flow_graph:
            return []
        
        nodes = self.flow_graph.nodes
        needed = set()
        
        # Find all flows in the scenario that have requirements
        for flow_def in scenario.flows:
            node = nodes.get(flow_def.name)
            if not node:
                continue
            
            # Check what this flow requires
            for req_key, req_info in node.requires.items():
                source_flow = req_info.from_flow
                source_node = nodes.get(source_flow)
                
                # Only include prerequisites (not regular dependencies)
                if source_node and source_node.node_type == FlowNodeType.PREREQUISITE:
                    needed.add(source_flow)
        
        # Hydrate each prerequisite
        prereqs = []
        for flow_name in sorted(needed):  # Deterministic order
            flow_data = self._get_flow_data_by_name(flow_name)
            if flow_data and flow_data.get("status") == "supported":
                prereqs.append(HydratedFlow(
                    name=flow_name,
                    payload=flow_data.get("proto_request", {}),
                    status_handling=[],  # Prerequisites don't need status handling in examples
                    is_prerequisite=True,
                ))
        
        return prereqs
    
    def _resolve_field_links(self, flow_name: str) -> Dict[str, FieldLink]:
        """
        Get field-level dependencies for a flow from the flow_graph.
        
        Returns: {"connector_transaction_id": FieldLink(from_flow="authorize", ...)}
        
        Every dependency is explicit — the SDK does not auto-inject anything.
        All links produce generated code showing the developer how to pass
        the field from one response to the next request.
        
        This replaces the 5 hardcoded _DYNAMIC_FIELDS dicts.
        """
        if not self.flow_graph:
            return {}
        
        node = self.flow_graph.nodes.get(flow_name)
        if not node:
            return {}
        
        # Only apply a field link if the target path actually exists in the
        # connector's probe request payload for this flow.  The flow_graph may
        # over-report requirements (e.g. marking every connector that has a
        # create_access_token flow as needing one for authorize, regardless of
        # whether the connector's actual proto request uses that field).
        # Restricting to fields present in the payload avoids injecting phantom
        # references that break generated examples.
        flow_payload = self._get_flow_payload_for(flow_name)

        links = {}
        for field_key, req_info in node.requires.items():
            target_path = req_info.request_path
            # Check the top-level key of the target path against the payload
            top_key = target_path.split(".")[0]
            if flow_payload is not None and top_key not in flow_payload:
                continue  # Target field not present — skip this link
            links[target_path] = FieldLink(
                from_flow=req_info.from_flow,
                from_field=req_info.from_field,
                target_path=target_path,
            )

        return links
    
    def _get_flow_payload_for(self, flow_name: str) -> Optional[dict]:
        """Return the proto_request dict for a flow (any supported PM), or None."""
        flows = self.probe.get("flows", {})
        flow_entry = flows.get(flow_name, {})
        for pm_data in flow_entry.values():
            if isinstance(pm_data, dict) and pm_data.get("status") == "supported":
                return pm_data.get("proto_request") or {}
        return None

    def _get_flow_data_by_name(self, flow_name: str) -> Optional[dict]:
        """Get flow data from probe by flow name (using 'default' key)."""
        flows = self.probe.get("flows", {})
        flow_entry = flows.get(flow_name, {})
        return flow_entry.get("default")


def hydrate_all_scenarios(
    scenarios: List[Scenario],
    probe_data: dict,
    connector_name: str
) -> List[HydratedScenario]:
    """
    Hydrate all scenarios for a connector.
    
    Returns only scenarios that are supported (have all required flows).
    """
    hydrator = ScenarioHydrator(probe_data, connector_name)
    hydrated = []
    
    for scenario in scenarios:
        result = hydrator.hydrate(scenario)
        if result:
            hydrated.append(result)
    
    return hydrated
