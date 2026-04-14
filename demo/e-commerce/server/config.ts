import { types } from 'hyperswitch-prism';
import dotenv from 'dotenv';

dotenv.config();

const { Environment } = types;

// Connector configurations
export const getStripeConfig = (): types.ConnectorConfig => ({
  connectorConfig: {
    stripe: {
      apiKey: { value: process.env.STRIPE_API_KEY! }
    }
  },
  options: {
    environment: Environment.SANDBOX
  }
});

export const getGlobalPayConfig = (): types.ConnectorConfig => ({
  connectorConfig: {
    globalpay: {
      appId: { value: process.env.GLOBALPAY_APP_ID! },
      appKey: { value: process.env.GLOBALPAY_APP_KEY! }
    }
  },
  options: {
    environment: Environment.SANDBOX
  }
});

export const getAdyenConfig = (): types.ConnectorConfig => ({
  connectorConfig: {
    adyen: {
      apiKey: { value: process.env.ADYEN_API_KEY! },
      merchantAccount: { value: process.env.ADYEN_MERCHANT_ACCOUNT! }
    }
  },
  options: {
    environment: Environment.SANDBOX
  }
});

// Routing logic: Amount > $50.00 (5000 cents) -> Adyen, else USD -> Stripe, EUR -> GlobalPay
export const getConnectorConfig = (currency: string, amount: number): types.ConnectorConfig => {
  console.log(`[Routing] Amount: ${amount} cents (${(amount / 100).toFixed(2)})`);
  if (amount > 5000) {
    return getAdyenConfig();
  }
  return currency === 'EUR' ? getGlobalPayConfig() : getStripeConfig();
};

export const getConnectorName = (currency: string, amount: number): string => {
  if (amount > 5000) {
    return 'adyen';
  }
  return currency === 'EUR' ? 'globalpay' : 'stripe';
};

// Server configuration
export const config = {
  port: parseInt(process.env.PORT || '3000', 10),
  nodeEnv: process.env.NODE_ENV || 'development',
  baseUrl: process.env.BASE_URL || 'http://localhost:3000'
};
