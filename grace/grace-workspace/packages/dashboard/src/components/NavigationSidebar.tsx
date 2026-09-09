import { Link, useLocation } from "react-router-dom";
import { T } from "../theme";
import mvpData from "../data/mvp.json";

// mvp.json (114 KB) rather than connectors.json (1,657 KB): the shared layout
// pulled the larger file into every route just to print two integers, and its
// 110-connector count contradicted the 108 shown everywhere else.
const CONNECTOR_COUNT = (mvpData as { connectors: unknown[] }).connectors.length;
const CAPABILITY_COUNT = (mvpData as { capabilities: unknown[] }).capabilities.length;

const SIDEBAR_WIDTH = 240;

interface NavItem {
  id: string;
  label: string;
  icon: string;
  path: string;
}

// A static deploy (Netlify) has no supervisor WebSocket and no dev-server /api/*
// middleware, so only the MVP views work there. Hiding the rest beats shipping
// nav that leads to permanently-broken pages.
const STATIC_BUILD = import.meta.env.VITE_STATIC === "1";

const ALL_NAV_ITEMS: NavItem[] = [
  { id: "home", label: "Home", icon: "🏠", path: "/" },
  { id: "connectors", label: "Payment Processors", icon: "💳", path: "/connectors" },
  { id: "mvp", label: "MVP Readiness", icon: "🎯", path: "/mvp" },
  { id: "mvp-metrics", label: "MVP Metrics", icon: "📊", path: "/mvp/metrics" },
  { id: "parity", label: "Parity Checker", icon: "⚖️", path: "/parity" },
  { id: "pr-resolver", label: "PR Resolver", icon: "🔧", path: "/pr-resolver" },
];

const NAV_ITEMS: NavItem[] = STATIC_BUILD
  ? ALL_NAV_ITEMS.filter((i) => i.path.startsWith("/mvp"))
  : ALL_NAV_ITEMS;

export function NavigationSidebar() {
  const location = useLocation();
  const currentPath = location.pathname;

  // The nav entry that owns the current route: an exact match, else the longest
  // path the route sits under. Without the "longest" rule, /mvp/metrics would
  // highlight both "MVP Readiness" (/mvp) and "MVP Metrics".
  const activePath = NAV_ITEMS.map((i) => i.path)
    .filter((path) => currentPath === path || (path !== "/" && currentPath.startsWith(path + "/")))
    .sort((a, b) => b.length - a.length)[0];

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
          // Longest matching path wins, so a nested item (/mvp/metrics) does not
          // also light up its parent (/mvp).
          const isActiveOrChild = item.path === activePath;
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
                color: isActiveOrChild ? T.text : T.textMuted,
                background: isActiveOrChild ? T.accentSoft : "transparent",
                borderLeft: isActiveOrChild ? `3px solid ${T.accent}` : "3px solid transparent",
                fontWeight: isActiveOrChild ? 600 : 500,
                fontSize: 14,
                transition: "all 150ms ease",
              }}
              onMouseEnter={(e) => {
                if (!isActiveOrChild) {
                  e.currentTarget.style.background = T.bgElev;
                }
              }}
              onMouseLeave={(e) => {
                if (!isActiveOrChild) {
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
          {`${CONNECTOR_COUNT} connectors · ${CAPABILITY_COUNT} capabilities`}
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
          // A flex item defaults to min-width:auto, so a wide child (the MVP
          // matrix) stretches main past the viewport instead of scrolling
          // inside its own panel, which clipped the header and stat cards.
          minWidth: 0,
          minHeight: "100vh",
        }}
      >
        {children}
      </main>
    </div>
  );
}
