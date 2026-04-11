import { types } from 'hs-paylib';

// Connector credentials loaded from environment variables or defaults from creds.json
export const CYBERSOURCE_CONFIG: types.ConnectorConfig = {
  connectorConfig: {
    cybersource: {
      apiKey: { value: process.env.CYBERSOURCE_API_KEY || '06b99051-2e1c-4f37-9e9e-74271c08784b' },
      merchantAccount: { value: process.env.CYBERSOURCE_MERCHANT_ACCOUNT || 'getin_sandbox' },
      apiSecret: { value: process.env.CYBERSOURCE_API_SECRET || 'poOAYlRewyGWK+6rytViOyswhT/qjgAD0gXLOOshKUI=' },
    },
  },
};

export const PAYPAL_CONFIG: types.ConnectorConfig = {
  connectorConfig: {
    paypal: {
      clientId: { value: process.env.PAYPAL_CLIENT_ID || 'ASKAGh2WXgqfQ5TzjpZzLsfhVGlFbjq5VrV5IOX8KXDD2N_XqkGeYNDkWyr_UXnfhXpEkABdmP284b_2' },
      clientSecret: { value: process.env.PAYPAL_CLIENT_SECRET || 'EOpaRHxEgaMJ9OHfsn3ngHy7DoXArNjPgCwsrzaJreO3gXPSJP_r4iOp1UUEn140CsEjaYxtm0g61VFU' },
    },
  },
};

export const ADYEN_CONFIG: types.ConnectorConfig = {
  connectorConfig: {
    adyen: {
      apiKey: { value: process.env.ADYEN_API_KEY || 'AQEqhmfxK43MaR1Hw0m/n3Q5qf3VYp5eHZJTfEA0SnT87rrwTHXDVGtJ+kfCEMFdWw2+5HzctViMSCJMYAc=-sNyhV/b3uZx5d38TcqtscjboxGoH4khiJHYuEuUJ5IQ=-i1i2%dW^xT(m?b+LC7$' },
      merchantAccount: { value: process.env.ADYEN_MERCHANT_ACCOUNT || 'JuspayDEECOM' },
    },
  },
};

export const SERVER_PORT = parseInt(process.env.PORT || '3000', 10);
