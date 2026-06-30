// Small presentation helpers so the demos read like a "payment experience".

import { types } from 'hyperswitch-prism';

// Reverse-lookup a numeric enum value to its name (e.g. 8 -> 'CHARGED').
function enumName(enumObj: Record<string, unknown>, value: number): string {
  for (const [k, v] of Object.entries(enumObj)) {
    if (v === value) return k;
  }
  return `UNKNOWN(${value})`;
}

export function paymentStatusText(status: number): string {
  return enumName(types.PaymentStatus as unknown as Record<string, unknown>, status);
}

export function refundStatusText(status: number): string {
  return enumName(types.RefundStatus as unknown as Record<string, unknown>, status);
}

export function money(minorAmount: number, currency: string): string {
  return `${(minorAmount / 100).toFixed(2)} ${currency}`;
}

export function banner(title: string): void {
  const line = '─'.repeat(Math.max(title.length + 2, 60));
  console.log(`\n┌${line}┐`);
  console.log(`│ ${title}`);
  console.log(`└${line}┘`);
}

export function step(msg: string): void {
  console.log(`  ▸ ${msg}`);
}
