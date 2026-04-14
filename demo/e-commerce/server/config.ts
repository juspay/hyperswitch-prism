import { types } from 'hyperswitch-prism';
import dotenv from 'dotenv';

dotenv.config();

const { Environment } = types;

// Connector configurations
export const getStripeConfig = (): types.ConnectorConfig => ({

});

export const getGlobalPayConfig = (): types.ConnectorConfig => ({

});

export const getAdyenConfig = (): types.ConnectorConfig => ({

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
