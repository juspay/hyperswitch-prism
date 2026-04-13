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

// Routing logic: USD -> Stripe, EUR -> GlobalPay
export const getConnectorConfig = (currency: string): types.ConnectorConfig => {
  return currency === 'EUR' ? getGlobalPayConfig() : getStripeConfig();
};

export const getConnectorName = (currency: string): string => {
  return currency === 'EUR' ? 'globalpay' : 'stripe';
};

// Get publishable key for client SDK
export const getPublishableKey = (currency: string): string => {
  if (currency === 'EUR') {
    return process.env.GLOBALPAY_PUBLISHABLE_KEY || '';
  }
  return process.env.STRIPE_PUBLISHABLE_KEY || '';
};

// Server configuration
export const config = {
  port: parseInt(process.env.PORT || '3000', 10),
  nodeEnv: process.env.NODE_ENV || 'development',
  baseUrl: process.env.BASE_URL || 'http://localhost:3000'
};
