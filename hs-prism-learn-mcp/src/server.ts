import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { SERVER_NAME, SERVER_VERSION } from "./constants.js";
import { registerStartHere } from "./tools/startHere.js";
import { registerArchitectureOverview } from "./tools/architectureOverview.js";
import { registerExplainConcept } from "./tools/explainConcept.js";
import { registerGlossary } from "./tools/glossary.js";
import { registerRepoMap } from "./tools/repoMap.js";
import { registerSearch } from "./tools/search.js";
import { registerReadDoc } from "./tools/readDoc.js";
import { registerHowTo } from "./tools/howTo.js";
import { registerExplainFlow } from "./tools/explainFlow.js";
import { registerConnectorWalkthrough } from "./tools/connectorWalkthrough.js";
import { registerLearningPath } from "./tools/learningPath.js";
import { registerTroubleshoot } from "./tools/troubleshoot.js";
import { registerCoverage } from "./tools/coverage.js";
import { registerFaq } from "./tools/faq.js";
import { registerResources } from "./resources/index.js";

export function createServer(): McpServer {
  const server = new McpServer(
    { name: SERVER_NAME, version: SERVER_VERSION },
    {
      instructions:
        "Helps someone NEW to the hyperswitch-prism (UCS) codebase understand it in plain language, grounded in the repo's " +
        "real docs and code. Call prism_learn_start_here first. Use prism_learn_explain_concept for 'what is X' (depth=tldr " +
        "for non-engineers), prism_learn_architecture_overview for the big picture, prism_learn_repo_map / prism_learn_search " +
        "for 'where is X', prism_learn_how_to for 'how do I do X', prism_learn_explain_flow / prism_learn_connector_walkthrough " +
        "for flows and connectors, prism_learn_troubleshoot for errors, and prism_learn_read_doc to quote the source verbatim. " +
        "Every answer cites a real file path — never invent facts about the repo. This server is read-only and complements " +
        "integration-mcp (which is for embedding the SDK into an app).",
    },
  );

  registerStartHere(server);
  registerArchitectureOverview(server);
  registerExplainConcept(server);
  registerGlossary(server);
  registerRepoMap(server);
  registerSearch(server);
  registerReadDoc(server);
  registerHowTo(server);
  registerExplainFlow(server);
  registerConnectorWalkthrough(server);
  registerLearningPath(server);
  registerTroubleshoot(server);
  registerCoverage(server);
  registerFaq(server);

  registerResources(server);

  return server;
}
