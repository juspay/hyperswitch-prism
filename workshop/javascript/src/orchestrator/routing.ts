// Condition-based routing (orchestrator use-case A)
// ─────────────────────────────────────────────────────────────────────────────
// A pure decision function: given a payment context, pick which PSP should
// handle it. "Pure" means it has no side effects and never touches the network,
// so it is trivial to unit-test (see test/routing.test.ts).
//
// A routing plan is an ordered list of rules. The first rule whose `when`
// predicate matches wins. If none match, `fallback` is used.
// ─────────────────────────────────────────────────────────────────────────────

import type { PspName } from '../../config/psp-registry.js';

export interface RoutingContext {
  minorAmount: number; // amount in minor units (cents)
  currency: string; // ISO 4217, e.g. 'USD'
}

export interface RoutingRule {
  reason: string;
  when: (ctx: RoutingContext) => boolean;
  use: PspName;
}

export interface RoutingPlan {
  rules: RoutingRule[];
  fallback: PspName;
}

export interface RoutingDecision {
  psp: PspName;
  reason: string;
}

export function selectPsp(plan: RoutingPlan, ctx: RoutingContext): RoutingDecision {
  for (const rule of plan.rules) {
    if (rule.when(ctx)) {
      return { psp: rule.use, reason: rule.reason };
    }
  }
  return { psp: plan.fallback, reason: 'default (no rule matched)' };
}

// A sample plan used by the demo. Read it top-to-bottom:
//   1. Anything over $50.00 → Adyen (e.g. better high-value acceptance rates)
//   2. EUR payments        → Cybersource
//   3. otherwise           → Stripe
export const DEFAULT_ROUTING_PLAN: RoutingPlan = {
  rules: [
    {
      reason: 'high-value payment (> $50.00) → Adyen',
      when: (ctx) => ctx.minorAmount > 5000,
      use: 'adyen',
    },
    {
      reason: 'EUR currency → Cybersource',
      when: (ctx) => ctx.currency.toUpperCase() === 'EUR',
      use: 'cybersource',
    },
  ],
  fallback: 'stripe',
};
