# Frequently Asked Questions

## General

### How is Prism different from existing open source payment integration libraries - Omnipay/ActiveMerchant?

While Prism were inspired by previous initiatives on similar lines (active-merchant for Ruby, omnipay for PHP), Prism brings several unique advantages:

1. **Polyglot Ready**: The protobuf spec serves as the interface, allowing for extensions across many programming languages.

2. **Flexible to Use**: Dual deployment mode (embedded and gRPC from the same core). PCI compliance can be managed by the user, or outsourced to a PCI compliant third party.

3. **Hardened with Real Traffic**: We extracted the integrations from a production-grade payment orchestrator at scale, rather than designing from scratch. We continue to use it in production with regular updates, similar to how we have been consistently delivering for the last 4 years on Juspay Hyperswitch.

### Why can't payment integration be done with LLMs? Why should I use Prism library?

Every payment processor has diverse APIs, error codes, authentication methods, PDF documents to read, and behavioral differences between the actual environment and documented specs.

A small mistake or oversight can create a huge financial impact for businesses accepting payments. Thousands of enterprises around the world have gone through this learning curve, iterating and fixing payment systems over many years. The learnings are not published or documented in a single place to provide instructions to LLMs.

All such fixes, improvements, and iterations are locked-in as tribal knowledge into Enterprise Payment Platforms and SaaS Payment Orchestration solutions.

### What is meant by hardening?

Hardening involves:

1. Identifying non-documented edge cases and fixing them in the integration code
2. Actively running integrations on payment processors' test/production environments to stay in sync

This is where Prism plays a very important role in democratizing the tribal knowledge which exists outside payment processor documentation or, baked into the integration certification process.

Prism was extracted from Juspay Hyperswitch and is an actively used component in production, powering payments for large enterprises.


## Security & Compliance

### How does Prism handle PCI compliance?

Prism is PCI-Compliant by design. It operates as a stateless library with no data storage and no logging—meaning no PCI data is ever persisted.

Prism provides you with the flexibility to:
- Outsource your PCI compliance to third-party vaults/payment processors of your choice
- Insource the compliance fully

Learn more from the [PCI compliance section](./architecture/compliance/compliance.md).

### How is sensitive data protected when using Prism?

No data is logged to any server. The library is credential-free and can be embedded into your app without any risks or compliance involved.

Just like an optical prism, it takes an input (beam of light), transforms (refracts), and sends it to the target payment processor endpoint of your choice.


## Support & Maintenance

### Who develops and maintains the Prism library?

Prism is actively maintained by the Juspay Hyperswitch team with regular updates for:
- New integrations
- Feature enhancements
- Security patches
- Bug fixes
- Framework improvements

It was extracted from Juspay Hyperswitch and is an actively used component in production, powering payments for large enterprises.

### How many payment processors and methods does Prism support?

Prism supports supports multiple connectors and numerous payment methods (beyond cards). An elaborate list is available in the [Supported Connectors List](https://github.com/juspay/hyperswitch-prism/blob/main/docs-generated/all_connector.md).

A quick summary of the payment methods as below.
- **Cards**: Visa, Mastercard, Amex, etc.
- **Digital Wallets**: Apple Pay, Google Pay, PayPal
- **Online Banking**: iDEAL, Sofort
- **Bank Transfers**: SEPA, ACH
- **Buy Now Pay Later**: Klarna, Affirm
- **Crypto**
- **Regional Methods**: UPI, PIX, Boleto
