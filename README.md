<div align="center">


# Hyperswitch Prism


**One integration. Any payment processor. Zero lock-in.**


[![License: Apache 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)


*A high-performance payment abstraction library, and part of [Juspay Hyperswitch](https://hyperswitch.io/) — the open-source, composable payments platform with 40,000+ GitHub stars, trusted by leading brands worldwide.*


[GitHub](https://github.com/juspay/hyperswitch) · [Website](https://hyperswitch.io/) · [Documentation](https://docs.hyperswitch.io/)


</div>


---


## 🎯 What is Prism?

Today, integrating multiple payment processors either makes developers running in circles with AI agents to recreate integrations from specs, or developers spending months of engineering effort.

Because every payment processor has diverse APIs, error codes, authentication methods, pdf documents to read, and above all - different behaviour in the actual environment when compared to documented specs. All this rests as tribal or undocumented knowledge making it harder AI agents which are very good at implementing clearly documented specification.

**Prism is a stateless, unified connector library for AI agents and Developers to connect with any payment processor.**

**Prism offers hardened transformation through testing on payment processor environment & iterative bug fixing.**

**Prism can be embedded in your server application with its wide range of multi-language SDKs, or run as a gRPC microservice**


| ❌ Without Prism | ✅ With Prism |
|------------------------------|----------------------------|
| 🗂️ 100+ different API schemas | 📋 Single unified schema |
| ⏳ In-deterministic agent loops / months of integration work | ⚡ Deterministic agent loops, hours to integrate |
| 🔗 Brittle, provider-specific code | 🔓 Portable, provider-agnostic code |
| 🚫 Hard to switch providers | 🔄 Change providers in 1 line |


---


## ✨ Features


- **🔌 100+ Connectors** — Stripe, Adyen, Braintree, PayPal, Worldpay, and more
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
    <groupId>io.hyperswitch</groupId>
    <artifactId>prism</artifactId>
    <version>0.0.1</version>
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

```typescript
import { PaymentClient, types, IntegrationError, ConnectorResponseTransformationError } from 'hyperswitch-prism';

let config: types.ConnectorConfig = {
    connectorConfig: {
        stripe: {
            apiKey: { value: "sk_test_" }
        }
    }
}

const main = async () => {
    try {
        let client = new PaymentClient(config)
        let request: types.PaymentServiceAuthorizeRequest = {
            merchantTransactionId: "authorize_123",
            amount: {
                minorAmount: 1000, // $10.00
                currency: types.Currency.USD,
            },
            captureMethod: types.CaptureMethod.AUTOMATIC,
            paymentMethod: {
                card: {
                    cardNumber: { value: "4111111111111111" },
                    cardExpMonth: { value: "12" },
                    cardExpYear: { value: "2050" },
                    cardCvc: { value: "123" },
                    cardHolderName: { value: "Test User" },
                },
            },
            authType: types.AuthenticationType.NO_THREE_DS,
            address: {},
            orderDetails: [],
        }
        let response: types.PaymentServiceAuthorizeResponse = await client.authorize(request);
        switch (response.status) {
            case types.PaymentStatus.CHARGED:
                console.log("success");
                break;
            default:
                console.error("failed");
        }
    } catch (e: any) {
        switch (true) {
            case (e instanceof IntegrationError): {
                console.error("Error", e);
                break;
            }
            case (e instanceof ConnectorResponseTransformationError): {
                console.error("Error", e);
                break;
            }
            default: {
                console.error("Error", e);
            }
        }
    }
}

main()
```

---

## 🔄 Routing between Payment Providers

Once the basic plumbing is implemented you can leverage Prism's core benefit - **switch payment providers by changing one line**.


```typescript
  // Routing rule: EUR -> Adyen, USD -> Stripe
  const currency = types.Currency.USD;

  let stripeConfig: types.ConnectorConfig = {
      connectorConfig: {
          stripe: {
              apiKey: { value: process.env.STRIPE_API_KEY! }
          }
      }
  }

  let adyenConfig: types.ConnectorConfig = {
      connectorConfig: {
          adyen: {
              apiKey: { value: process.env.ADYEN_API_KEY! },
              merchantAccount: process.env.ADYEN_MERCHANT_ACCOUNT!
          }
      }
  }

  const config = currency === types.Currency.EUR ? adyenConfig : stripeConfig;
  const client = new PaymentClient(config);

  const request: types.PaymentServiceAuthorizeRequest = {
      merchantTransactionId: "order_123",
      amount: {
          minorAmount: 1000,
          currency: currency
      },
      captureMethod: types.CaptureMethod.AUTOMATIC,
      paymentMethod: {
          card: {
              cardNumber: { value: "4111111111111111" },
              cardExpMonth: { value: "12" },
              cardExpYear: { value: "2050" },
              cardCvc: { value: "123" },
              cardHolderName: { value: "Test User" },
          },
      },
      authType: types.AuthenticationType.NO_THREE_DS,
      address: {},
      orderDetails: [],
  };

  const response = await client.authorize(request);
  console.log(`Payment authorized with ${currency === types.Currency.EUR ? 'Adyen' : 'Stripe'}`);
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
