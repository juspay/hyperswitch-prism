# Environment Settings

Your code behaves differently in development, staging, and production. Prism lets you configure environments explicitly so test transactions don't hit live payment processors and production keys don't leak into debug logs.

## Why Environment Control Matters?

Payment integrations have three distinct modes. And typically most processors support Sandbox and Production.
The Development environment will matter when there is a need to mock the payment processor request or response, especially if their sandboxes are flaky for your testing pipeline.

| Environment | Use Case |
|-------------|----------|
| **Production** | Use this to process live customer transactions. Developers monitor real payment flows, handle actual disputes, and optimize conversion based on production data. |
| **Sandbox/Staging** | Use this to validate end-to-end integrations with real payment processors. Developers verify webhook handling, test edge cases with simulated failures, and ensure the full flow works before going live. |
| **Development** | You may use this to test features locally without network calls. Developers run unit tests against mock responses and iterate on business logic without waiting for external APIs. |

## How to Configure Environment?

You may configure environments at the client level using the SdkOptions as shown below.

```javascript
const { PaymentClient } = require('hyperswitch-prism');

// Sandbox environment configuration
const sandboxConfig = {
    options: { environment: "SANDBOX" },
    connectorConfig: {
        stripe: {
            apiKey: { value: process.env.STRIPE_TEST_API_KEY }
        }
    }
};
const sandboxClient = new PaymentClient(sandboxConfig);

// Production environment configuration
const prodConfig = {
    options: { environment: "PRODUCTION" },
    connectorConfig: {
        stripe: {
            apiKey: { value: process.env.STRIPE_LIVE_API_KEY }
        }
    }
};
const prodClient = new PaymentClient(prodConfig);
```

