import { Link, useLocation } from "react-router-dom";
import { T } from "../theme";
import connectorsData from "../data/connectors.json";
import type { Connector } from "../types/connector";

const CONNECTORS = connectorsData as Connector[];
const CONNECTOR_COUNT = CONNECTORS.length;
const FLOW_COUNT = CONNECTORS[0]?.flows.length ?? 0;

const SIDEBAR_WIDTH = 240;

interface NavItem {
  id: string;
  label: string;
  icon: string;
  path: string;
}

const NAV_ITEMS: NavItem[] = [
  { id: "home", label: "Home", icon: "🏠", path: "/" },
  { id: "connectors", label: "Payment Processors", icon: "💳", path: "/connectors" },
];

export function NavigationSidebar() {
  const location = useLocation();
  const currentPath = location.pathname;

  return (
    <aside
      style={{
        width: SIDEBAR_WIDTH,
        background: T.bgSidebar,
        borderRight: `1px solid ${T.border}`,
        display: "flex",
        flexDirection: "column",
        height: "100vh",
        position: "fixed",
        left: 0,
        top: 0,
        zIndex: 10,
      }}
    >
      {/* Header */}
      <div
        style={{
          padding: "22px 20px 18px",
          borderBottom: `1px solid ${T.border}`,
        }}
      >
        <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
          <div
            style={{
              width: 34,
              height: 34,
              borderRadius: 10,
              background: `linear-gradient(135deg, ${T.accent}, #c97a45)`,
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              color: "#fff",
              fontWeight: 700,
              fontSize: 15,
              boxShadow: "0 2px 6px rgba(160, 82, 45, 0.25)",
            }}
          >
            C
          </div>
          <div>
            <div
              style={{
                fontSize: 15,
                fontWeight: 700,
                color: T.text,
                lineHeight: 1.2,
              }}
            >
              10XGRACE
            </div>
            <div style={{ fontSize: 10, color: T.textMuted, marginTop: 2 }}>
              spec-driven dev
            </div>
          </div>
        </div>
      </div>

      {/* Navigation Items */}
      <nav style={{ flex: 1, padding: "16px 12px" }}>
        {NAV_ITEMS.map((item) => {
          const isActive = currentPath === item.path;
          return (
            <Link
              key={item.id}
              to={item.path}
              style={{
                display: "flex",
                alignItems: "center",
                gap: 12,
                padding: "12px 16px",
                borderRadius: 8,
                marginBottom: 4,
                textDecoration: "none",
                color: isActive ? T.text : T.textMuted,
                background: isActive ? T.accentSoft : "transparent",
                borderLeft: isActive ? `3px solid ${T.accent}` : "3px solid transparent",
                fontWeight: isActive ? 600 : 500,
                fontSize: 14,
                transition: "all 150ms ease",
              }}
              onMouseEnter={(e) => {
                if (!isActive) {
                  e.currentTarget.style.background = T.bgElev;
                }
              }}
              onMouseLeave={(e) => {
                if (!isActive) {
                  e.currentTarget.style.background = "transparent";
                }
              }}
            >
              <span style={{ fontSize: 18 }}>{item.icon}</span>
              <span>{item.label}</span>
            </Link>
          );
        })}
      </nav>

      {/* Footer */}
      <div
        style={{
          padding: "16px 20px",
          borderTop: `1px solid ${T.border}`,
          fontSize: 11,
          color: T.textSubtle,
        }}
      >
        <div>Grace Workflow v2.3</div>
        <div style={{ marginTop: 4, opacity: 0.7 }}>
          {`${CONNECTOR_COUNT} connectors · ${FLOW_COUNT} flows`}
        </div>
      </div>
    </aside>
  );
}

export function SidebarLayout({ children }: { children: React.ReactNode }) {
  return (
    <div style={{ display: "flex", minHeight: "100vh" }}>
      <NavigationSidebar />
      <main
        style={{
          marginLeft: SIDEBAR_WIDTH,
          flex: 1,
          minHeight: "100vh",
        }}
      >
        {children}
      </main>
    </div>
  );
}
