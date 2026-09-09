import { Navigate, Route, Routes } from "react-router-dom";
import { Homepage } from "./pages/Homepage";
import { WorkflowPage } from "./pages/WorkflowPage";
import { ConnectorsPage } from "./pages/ConnectorsPage";
import { ConnectorDetailPage } from "./pages/ConnectorDetailPage";
import { MvpReadinessPage } from "./pages/MvpReadinessPage";
import { MvpMetricsPage } from "./pages/MvpMetricsPage";
import { ParityPage } from "./pages/ParityPage";
import { ParityConnectorPage } from "./pages/ParityConnectorPage";
import { PrResolverPage } from "./pages/PrResolverPage";
import { PrResolverDetailPage } from "./pages/PrResolverDetailPage";

/**
 * App shell. Routes:
 *   /                          — Homepage (sessions list, create)
 *   /connectors                — ConnectorsPage (payment processors list)
 *   /connectors/:connectorName — ConnectorDetailPage (individual connector)
 *   /mvp                       — MvpReadinessPage (connector × MVP capability matrix)
 *   /mvp/metrics               — MvpMetricsPage (numeric rollup of the matrix)
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
      <Route
        path="/"
        element={import.meta.env.VITE_STATIC === "1" ? <Navigate to="/mvp" replace /> : <Homepage />}
      />
      <Route path="/connectors" element={<ConnectorsPage />} />
      <Route path="/connectors/:connectorName" element={<ConnectorDetailPage />} />
      <Route path="/mvp" element={<MvpReadinessPage />} />
      <Route path="/mvp/metrics" element={<MvpMetricsPage />} />
      <Route path="/parity" element={<ParityPage />} />
      <Route path="/parity/:connectorName" element={<ParityConnectorPage />} />
      <Route path="/sessions/:sessionId" element={<WorkflowPage />} />
      <Route path="/pr-resolver" element={<PrResolverPage />} />
      <Route path="/pr-resolver/:prNumber" element={<PrResolverDetailPage />} />
      <Route
        path="*"
        element={<Navigate to={import.meta.env.VITE_STATIC === "1" ? "/mvp" : "/"} replace />}
      />
    </Routes>
  );
}
