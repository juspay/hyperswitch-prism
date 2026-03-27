**One integration. Any payment processor. Zero lock-in.**

---

## 🎯 What is Prism?

Today, integrating multiple payment processors either makes developers running in circles with AI agents to recreate integrations from specs, or developers spending months of engineering effort.

Because every payment processor has diverse APIs, error codes, authentication methods, pdf documents to read, and above all - different behaviour in the actual environment when compared to documented specs. All this rests as tribal or undocumented knowledge making it harder AI agents which are very good at implementing clearly documented specification.

**Prism is a stateless, unified connector library for AI agents and Developers to connect with any payment processor**

**Prism offers hardened transformation through testing on payment processor environment & iterative bug fixing**

**Prism can be embedded in your server application with its wide range of multi-language SDKs, or run as a gRPC microservice**


| ❌ Without Prism | ✅ With Prism |
|------------------------------|----------------------------|
| 🗂️ 60+ different API schemas | 📋 Single unified schema |
| ⏳ In-deterministic agent loops / months of integration work | ⚡ Deterministic agent loops, hours to integrate |
| 🔗 Brittle, provider-specific code | 🔓 Portable, provider-agnostic code |
| 🚫 Hard to switch providers | 🔄 Change providers in 1 line |


---


## ✨ Features


- **🔌 60+ Connectors** — Stripe, Adyen, Braintree, PayPal, Worldpay, and more
- **🌍 Global Coverage** — Cards, wallets, bank transfers, BNPL, and regional methods
- **🚀 Zero Overhead** — Rust core with native bindings, no overhead
- **🔒 PCI-Compliant by Design** — Stateless, no data storage


---


## 🏗️ Architecture

The Prism library is compliant for payment processing by design. It is:
- **Stateless** — Hence, no PII or PCI data stored
- **Credential free** — The API keys are never logged nor exposed
- **Payment compliance outsourcing supported** — You can continue to outsource your PCI compliance to third party vaults, or payment processor without having to handle credit card data. 


```
┌─────────────────────────────────────────────────────────────────┐
│                        Your Application                         │
└───────────────────────────────┬─────────────────────────────────┘
                                │
                                ▼
┌─────────────────────────────────────────────────────────────────┐
│                         Prism Library                           │
│     (Type-safe, idiomatic interface, Multi-language SDK)        │
└────────────────────────────────┬────────────────────────────────┘
                                 │
                                 ▼
         ┌───────────────────────┼───────────────────────┬───────────────────────┐
         ▼                       ▼                       ▼                       ▼
   ┌──────────┐           ┌──────────┐           ┌──────────┐           ┌──────────┐
   │  Stripe  │           │  Adyen   │           │ Braintree│           │ 50+ more │
   └──────────┘           └──────────┘           └──────────┘           └──────────┘
```

---


## 🚀 Quick Start

### Install the Prism Library

Start by installing the library in the language of your choice.
<!-- tabs:start -->

#### **Node.js**

```bash
npm install hyperswitch-prism
```

#### **Python**

```bash
pip install payments
```

#### **Java**

Add to your `pom.xml`:

```xml
<dependency>
    <groupId>com.juspay.hyperswitch</groupId>
    <artifactId>prism</artifactId>
    <version>1.0.0</version>
</dependency>
```

#### **PHP**

```bash
composer require juspay/hyperswitch-prism
```

For detailed installation instructions, see [Installation Guide](./getting-started/installation.md).

---

### Create a Payment Order

<!-- tabs:start -->

#### **Node.js**

```javascript
const { PaymentClient } = require('hyperswitch-prism');
const { ConnectorConfig, ConnectorSpecificConfig, SdkOptions, Environment } = require('hyperswitch-prism').types;

async function main() {
  // Configure Stripe client (Primary payment processor)
  const stripeConfig = ConnectorConfig.create({
    options: SdkOptions.create({ environment: Environment.SANDBOX }),
  });
  stripeConfig.connectorConfig = ConnectorSpecificConfig.create({
    stripe: { apiKey: { value: process.env.STRIPE_API_KEY } }
  });
  const stripeClient = new PaymentClient(stripeConfig);

  // Configure Adyen client (Secondary payment processor)
  const adyenConfig = ConnectorConfig.create({
    options: SdkOptions.create({ environment: Environment.SANDBOX }),
  });
  adyenConfig.connectorConfig = ConnectorSpecificConfig.create({
    adyen: {
      apiKey: { value: process.env.ADYEN_API_KEY },
      merchantAccount: process.env.ADYEN_MERCHANT_ACCOUNT
    }
  });
  const adyenClient = new PaymentClient(adyenConfig);

  const order = await stripeClient.createOrder({
    merchantOrderId: 'order-123',
    amount: {
      minorAmount: 1000,
      currency: "USD"
    },
    orderType: 'PAYMENT',
    description: 'Test order'
  });
  console.log('Order ID:', order.connectorOrderId);
}

main().catch(console.error);
```

---

## 🔄 Routing between Payment Providers

Once the basic plumbing is implemented you can leverage Prism's core benefit - **switch payment providers by changing one line**.


```javascript
  // Routing rule: EUR -> Adyen, USD -> Stripe
  const currency = 'USD';
  const client = currency === 'EUR' ? adyenClient : stripeClient;

  const order = await client.createOrder({
    merchantOrderId: 'order-123',
    amount: {
      minorAmount: 1000,
      currency: EUR
    },
    orderType: 'PAYMENT',
    description: 'Test order'
  });

  console.log(`Order created with ${currency === 'EUR' ? 'Adyen' : 'Stripe'}`);

// EUR goes to Adyen
createOrder('order-456', 'EUR', 2500);

// USD goes to Stripe
createOrder('order-123', 'USD', 1000);
```

**One integration pattern. Any service category.**

No rewriting. No re-architecting. Just swap the client with rules.
Each flow uses the same unified schema regardless of the underlying processor's API differences. No custom code per provider.

You can learn more about [intelligent routing](https://docs.hyperswitch.io/explore-hyperswitch/workflows/intelligent-routing) and [smart retries](https://docs.hyperswitch.io/explore-hyperswitch/workflows/smart-retries) to add more intelligence. It can help configure and manage diverse payment acceptance setup, as well as improve conversion rates.

---

## 🛠️ Development

### Prerequisites

- Rust 1.70+
- Protocol Buffers (protoc)

### Building from Source

```bash
# Clone the repository
git clone https://github.com/juspay/hyperswitch-prism.git
cd hyperswitch-prism

# Build
cargo build --release

# Run tests
cargo test
```

---

### Reporting Vulnerabilities
Please report security issues to [security@juspay.in](mailto:security@juspay.in).

---

<div align="center">

Built and maintained by [Juspay hyperswitch](https://hyperswitch.io)

</div>