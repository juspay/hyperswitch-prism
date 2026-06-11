import fs from "node:fs";
import { topologicalOrder } from "../checkpoints/flow-checkpoint.js";

/**
 * Detect which `.gracerules` flows a connector needs from its techspec.
 *
 * `.gracerules` PHASE 3 distinguishes:
 *   - Core flows: always run (Authorize, PSync, Capture, Refund, RSync, Void)
 *   - Pre-auth flows: conditional on the connector's authentication /
 *     order-creation / tokenisation requirements as described in the spec
 *
 * The detection here mirrors the original (now-deleted) source that lives
 * only as compiled JS at `dist/new-connector/flow-detector.js`: a needle
 * scan of the lower-cased spec body. Cheap and good enough — the wizard
 * lets the user override the detected list if they need to.
 */

export const CORE_FLOWS = [
  "Authorize",
  "PSync",
  "Capture",
  "Refund",
  "RSync",
  "Void",
] as const;

export type CoreFlow = (typeof CORE_FLOWS)[number];

export const PRE_AUTH_FLOWS = [
  "CreateAccessToken",
  "CreateOrder",
  "CreateConnectorCustomer",
  "PaymentMethodToken",
  "CreateSessionToken",
] as const;

export type PreAuthFlow = (typeof PRE_AUTH_FLOWS)[number];

interface PreAuthSignal {
  flow: PreAuthFlow;
  needles: string[];
}

const PRE_AUTH_SIGNALS: PreAuthSignal[] = [
  {
    flow: "CreateAccessToken",
    needles: [
      "oauth",
      "bearer token",
      "access_token",
      "access token",
      "POST /login",
      "POST /oauth/token",
      "createaccesstoken",
      "ServerAuthenticationToken",
    ],
  },
  {
    flow: "CreateOrder",
    needles: [
      "createorder",
      "create order",
      "POST /orders",
      "intent creation",
      "POST /payment_intents",
      "order_id required",
      "create_order",
    ],
  },
  {
    flow: "CreateConnectorCustomer",
    needles: [
      "createconnectorcustomer",
      "POST /customers",
      "customer must be created",
      "create customer first",
      "customer_id required",
    ],
  },
  {
    flow: "PaymentMethodToken",
    needles: [
      "paymentmethodtoken",
      "tokenization",
      "tokenize before",
      "payment_method_token",
      "POST /tokens",
      "card_token",
    ],
  },
  {
    flow: "CreateSessionToken",
    needles: [
      "createsessiontoken",
      "session token",
      "session initialization",
      "applepay session",
      "googlepay session",
      "merchant_session",
    ],
  },
];

export interface DetectedFlows {
  preAuth: PreAuthFlow[];
  core: CoreFlow[];
}

/**
 * Read the spec and return the detected flow set. Core flows are always
 * included; pre-auth flows are conditional on needle matches in the spec.
 * Missing spec file → all core flows, no pre-auth (safest default).
 */
export function detectFlowsFromSpec(specPath: string): DetectedFlows {
  if (!fs.existsSync(specPath)) {
    return { preAuth: [], core: [...CORE_FLOWS] };
  }
  const body = fs.readFileSync(specPath, "utf-8").toLowerCase();
  const preAuth: PreAuthFlow[] = [];
  for (const signal of PRE_AUTH_SIGNALS) {
    if (signal.needles.some((n) => body.includes(n.toLowerCase()))) {
      preAuth.push(signal.flow);
    }
  }
  return { preAuth, core: [...CORE_FLOWS] };
}

/**
 * Sequential implementation order per `.gracerules` PHASE 3: pre-auth
 * flows first, then core flows (Authorize → PSync → Capture → Refund →
 * RSync → Void). Each subagent assumes prior flows are already
 * implemented. Delegates to topologicalOrder so the canonical
 * dependsOn graph in flow-checkpoint.ts FLOWS is the single source of
 * truth — independent of the heuristic ordering this detector used to
 * apply.
 */
export function orderedFlowList(detected: DetectedFlows): string[] {
  return topologicalOrder([...detected.preAuth, ...detected.core]);
}
