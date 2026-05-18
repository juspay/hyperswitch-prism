import { Navigate, Route, Routes } from "react-router-dom";
import { Homepage } from "./pages/Homepage";
import { WorkflowPage } from "./pages/WorkflowPage";
import { ConnectorsPage } from "./pages/ConnectorsPage";
import { ConnectorDetailPage } from "./pages/ConnectorDetailPage";
import { ParityPage } from "./pages/ParityPage";
import { ParityConnectorPage } from "./pages/ParityConnectorPage";

/**
 * App shell. Routes:
 *   /                          — Homepage (sessions list, create)
 *   /connectors                — ConnectorsPage (payment processors list)
 *   /connectors/:connectorName — ConnectorDetailPage (individual connector)
 *   /parity                    — ParityPage (parity-autopilot leaf table)
 *   /parity/:connectorName     — ParityConnectorPage (per-connector drill-down)
 *   /sessions/:sessionId       — WorkflowPage (per-session pipeline UI)
 *
 * Anything unmatched bounces to "/" so the back-button never strands the
 * user on a dead URL after a session is deleted.
 */
export function App() {
  return (
    <Routes>
      <Route path="/" element={<Homepage />} />
      <Route path="/connectors" element={<ConnectorsPage />} />
      <Route path="/connectors/:connectorName" element={<ConnectorDetailPage />} />
      <Route path="/parity" element={<ParityPage />} />
      <Route path="/parity/:connectorName" element={<ParityConnectorPage />} />
      <Route path="/sessions/:sessionId" element={<WorkflowPage />} />
      <Route path="*" element={<Navigate to="/" replace />} />
    </Routes>
  );
}
