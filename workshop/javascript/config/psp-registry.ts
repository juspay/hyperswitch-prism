// PSP registry
// ─────────────────────────────────────────────────────────────────────────────
// One place that knows about every payment processor (PSP) the workshop can use.
// Each entry describes:
//   - displayName : human label for output
//   - currencies  : currencies the PSP is configured to accept in this workshop
//   - envKeys     : which environment variables hold its sandbox credentials
//   - isConfigured: whether those credentials are actually present in .env
//   - buildConfig : turns env vars into the `ConnectorConfig` the SDK expects
//
// ★ STEP 7 — "ADD A NEW PROCESSOR" ★
//   Adding a processor to the whole workshop is just adding ONE entry below.
//   No orchestrator, routing, retry, or demo code has to change — they all read
//   from this registry. See `cybersource` for a worked example, and STEPS.md.
// ─────────────────────────────────────────────────────────────────────────────

import { types } from 'hyperswitch-prism';

const { Environment } = types;

export type PspName = 'stripe' | 'adyen' | 'cybersource';

export interface PspEntry {
  displayName: string;
  currencies: string[];
  envKeys: string[];
  isConfigured: () => boolean;
  buildConfig: () => types.ConnectorConfig;
}

const env = (key: string): string => process.env[key] ?? '';

// A credential value counts as "configured" only if it is set and not still the
// placeholder text shipped in .env.example.
const isSet = (key: string): boolean => {
  const v = env(key).trim();
  return v.length > 0 && !v.startsWith('your_') && !v.startsWith('sk_test_your');
};

export const PSP_REGISTRY: Record<PspName, PspEntry> = {
  // ── PSP-1 ──────────────────────────────────────────────────────────────────
  stripe: {
    displayName: 'Stripe',
    currencies: ['USD', 'EUR', 'GBP'],
    envKeys: ['STRIPE_API_KEY'],
    isConfigured: () => isSet('STRIPE_API_KEY'),
    buildConfig: () => ({
      options: { environment: Environment.SANDBOX },
      connectorConfig: {
        stripe: {
          apiKey: { value: env('STRIPE_API_KEY') },
        },
      },
    }),
  },

  // ── PSP-2 ──────────────────────────────────────────────────────────────────
  adyen: {
    displayName: 'Adyen',
    currencies: ['USD', 'EUR', 'GBP'],
    envKeys: ['ADYEN_API_KEY', 'ADYEN_MERCHANT_ACCOUNT'],
    isConfigured: () => isSet('ADYEN_API_KEY') && isSet('ADYEN_MERCHANT_ACCOUNT'),
    buildConfig: () => ({
      options: { environment: Environment.SANDBOX },
      connectorConfig: {
        adyen: {
          apiKey: { value: env('ADYEN_API_KEY') },
          merchantAccount: { value: env('ADYEN_MERCHANT_ACCOUNT') },
        },
      },
    }),
  },

  // ── PSP-3 (added in STEP 7 as the "new processor" example) ──────────────────
  cybersource: {
    displayName: 'Cybersource',
    currencies: ['USD', 'EUR', 'GBP'],
    envKeys: ['CYBERSOURCE_API_KEY', 'CYBERSOURCE_MERCHANT_ACCOUNT', 'CYBERSOURCE_API_SECRET'],
    isConfigured: () =>
      isSet('CYBERSOURCE_API_KEY') &&
      isSet('CYBERSOURCE_MERCHANT_ACCOUNT') &&
      isSet('CYBERSOURCE_API_SECRET'),
    buildConfig: () => ({
      options: { environment: Environment.SANDBOX },
      connectorConfig: {
        cybersource: {
          apiKey: { value: env('CYBERSOURCE_API_KEY') },
          merchantAccount: { value: env('CYBERSOURCE_MERCHANT_ACCOUNT') },
          apiSecret: { value: env('CYBERSOURCE_API_SECRET') },
        },
      },
    }),
  },
};

export function getPsp(name: PspName): PspEntry {
  const entry = PSP_REGISTRY[name];
  if (!entry) {
    const known = Object.keys(PSP_REGISTRY).join(', ');
    throw new Error(`Unknown PSP "${name}". Known PSPs: ${known}`);
  }
  return entry;
}

export function listPsps(): PspName[] {
  return Object.keys(PSP_REGISTRY) as PspName[];
}
