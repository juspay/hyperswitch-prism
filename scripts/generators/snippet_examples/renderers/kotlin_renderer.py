"""Kotlin language renderer."""

from __future__ import annotations

from .base import BaseRenderer


class Renderer(BaseRenderer):
    """Kotlin SDK snippet renderer."""
    
    lang = "kotlin"
    extension = ".kt"

    def config_snippet(self, connector_name: str) -> str:
        return '''import payments.PaymentClient
import payments.ConnectorConfig

val config = ConnectorConfig.newBuilder()
    .setEnvironment(Environment.SANDBOX)
    .build()
val client = PaymentClient(config)'''

    def render_consolidated(self, connector_name, scenarios_with_payloads,
                           flow_metadata, message_schemas, flow_items=None):
        """Generate Kotlin file with all scenarios."""
        functions = []
        
        for scenario, _ in scenarios_with_payloads:
            func = self._gen_scenario_func(scenario)
            functions.append(func)
        
        return f'''// Auto-generated for {connector_name}
package examples.{connector_name}

import payments.PaymentClient

{chr(10).join(functions)}'''

    def _gen_scenario_func(self, scenario):
        """Generate single scenario function."""
        return f'''fun process{scenario.key.title().replace("_", "")}(txnId: String, config: ConnectorConfig) {{
    // {scenario.title}
    val client = PaymentClient(config)
}}'''
