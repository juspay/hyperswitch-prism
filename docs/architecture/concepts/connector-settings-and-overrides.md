# Connector Settings and Overrides

Prism provides three configurable settings per connector: **Proxy**, **Timeout**, and **Retry**. 

It offers the flexibility to enable the setting could be enabled at a connector level or overridden per request.

| Setting | Description | Default |
|---------|-------------|---------|
| **Proxy** | HTTP proxy URL for routing requests to a target endpoint. YOu may leverage this when you choose to outsource PCI compliance to compliant third party endpoint | None |
| **Timeout** | Request timeout from API call in milliseconds. You may tweak this for processor which are slower to respond. | 30000ms |
| **Retry** | Number of API retry attempts on failure, incase of network failures | 0 |

## Configuration at Connector Level

The below example is a connector level configuration of timeout, retry as well as a proxy of the request to a target endpoint. You may choose to use all or some in combination as required.

```javascript
const client = new ConnectorClient({
    connectors: {
        stripe: {
            apiKey: process.env.STRIPE_API_KEY,
            timeout: 60000,    // 60 second timeout
            retry: 2,          // Retry twice on failure
            proxy: 'http://proxy.example.com:8080'
        },
        adyen: {
            apiKey: process.env.ADYEN_API_KEY,
            timeout: 45000
        }
    }
});
```

## Override at a Request Level

This is an example of override the settings for a single request for timeout and retry.

```javascript
// Longer timeout for 3D Secure flows
const response = await client.payments.authorize({
    amount: { minorAmount: 1000, currency: 'USD' },
    paymentMethod: { card: { ... } },
    connector: 'stripe',
    timeout: 120000,   // 2 minute timeout for this request
    retry: 3           // More retries for this operation
});
```


## Proxy for PCI Vault

If you wish your payment request to be routed through a PCI complaint endpoint, you may configure the client like below.

```javascript
const client = new ConnectorClient({
    connectors: {
        stripe: {
            apiKey: process.env.STRIPE_API_KEY,
            proxy: 'https://tntxxx.sandbox.verygoodproxy.com'
        }
    }
});
```