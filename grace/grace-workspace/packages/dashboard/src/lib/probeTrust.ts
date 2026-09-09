import type { ConnectorStats } from "../types/connector";

/**
 * The field probe reports whether a request could be BUILT for a payment method,
 * not whether the processor accepts it. A connector whose transformer never
 * branches on payment method therefore probes as supporting almost everything.
 *
 * The signature is rejecting *nothing* while claiming a lot. Today that is
 * revolut, razorpayv2, grabpay and flywire — each 95/100 "supported" with zero
 * not-implemented, against Stripe's 27. Without this check they rank as the most
 * complete connectors in the fleet.
 *
 * A narrow connector with few methods and no rejections (phonepe, UPI only) is
 * NOT permissive, hence the claim floor.
 */
export const PERMISSIVE_CLAIM_FLOOR = 20;

export function isPermissiveProbe(stats: ConnectorStats): boolean {
  return stats.notImplemented === 0 && stats.supported > PERMISSIVE_CLAIM_FLOOR;
}
