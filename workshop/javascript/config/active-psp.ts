// ╔══════════════════════════════════════════════════════════════════════════╗
// ║  WORKSHOP STEP 3 — SWITCH THE PSP                                          ║
// ║                                                                            ║
// ║  This single constant decides which payment processor `npm run run:payment`║
// ║  uses. That is the whole point of a unified library: the rest of your app  ║
// ║  code never changes when you swap processors.                              ║
// ║                                                                            ║
// ║  Step 1–2 : leave this as 'stripe' and run  ->  npm run run:payment        ║
// ║  Step 3   : change 'stripe' to 'adyen' (one line!)                         ║
// ║  Step 4   : run again  ->  npm run run:payment                             ║
// ║                                                                            ║
// ║  Valid values are any key from PSP_REGISTRY (see config/psp-registry.ts):  ║
// ║    'stripe' | 'adyen' | 'cybersource'                                      ║
// ╚══════════════════════════════════════════════════════════════════════════╝

import type { PspName } from './psp-registry.js';

export const ACTIVE_PSP: PspName = 'stripe';
