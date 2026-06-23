import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { SERVER_NAME, SERVER_VERSION } from "./constants.js";
import { registerIntegrationGuide } from "./tools/integrationGuide.js";
import { registerListConnectors } from "./tools/listConnectors.js";
import { registerConnectorRequirements } from "./tools/connectorRequirements.js";
import { registerScaffoldIntegration } from "./tools/scaffoldIntegration.js";
import { registerGenerateConfig } from "./tools/generateConfig.js";
import { registerValidateConfig } from "./tools/validateConfig.js";
import { registerExplainError } from "./tools/explainError.js";
import { registerStatusReference } from "./tools/statusReference.js";
import { registerDoctor } from "./tools/doctor.js";
import { registerTestCharge } from "./tools/testCharge.js";
import { registerResources } from "./resources/index.js";

export function createServer(): McpServer {
  const server = new McpServer(
    { name: SERVER_NAME, version: SERVER_VERSION },
    {
      instructions:
        "Helps integrate the hyperswitch-prism payments SDK into a merchant app. Call prism_integration_guide first, " +
        "then follow its steps: prism_connector_requirements → prism_generate_config → prism_scaffold_integration → " +
        "prism_test_charge. Use prism_validate_config, prism_explain_error, prism_status_reference, and prism_doctor as " +
        "needed. All connector field names and status codes are sourced from the SDK proto (never guessed).",
    },
  );

  registerIntegrationGuide(server);
  registerListConnectors(server);
  registerConnectorRequirements(server);
  registerScaffoldIntegration(server);
  registerGenerateConfig(server);
  registerValidateConfig(server);
  registerExplainError(server);
  registerStatusReference(server);
  registerDoctor(server);
  registerTestCharge(server);

  registerResources(server);

  return server;
}
