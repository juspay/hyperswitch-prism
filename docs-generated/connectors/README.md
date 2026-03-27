# Connectors Overview

This document provides a comprehensive overview of all supported payment connectors and their integration status across various operations.

## Status Legend

| Status | Badge | Description |
|--------|-------|-------------|
| **integrated** | ![Integrated](https://img.shields.io/badge/-integrated-blue) | Code and mapping transformers are available in `/connectors` folder |
| **tested** | ![Tested](https://img.shields.io/badge/-tested-green) | Integrated with tests available in the connector's `test.rs` file |
| **not integrated** | ![Not Integrated](https://img.shields.io/badge/-not%20integrated-lightgrey) | No code or mapping available for this operation |

---

## PaymentService

The PaymentService handles all payment-related operations including authorization, capture, void, and sync operations.

### Core Payment Operations

| Connector | Authorize | Capture | Void | PSync (Get) | Setup Mandate | Create Order | Create Customer | Payment Token |
|-----------|:---------:|:-------:|:----:|:-----------:|:-------------:|:------------:|:---------------:|:-------------:|
| **Stripe** | ![Tested](https://img.shields.io/badge/-tested-green) | ![Tested](https://img.shields.io/badge/-tested-green) | ![Tested](https://img.shields.io/badge/-tested-green) | ![Tested](https://img.shields.io/badge/-tested-green) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | - | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) |
| **Adyen** | ![Tested](https://img.shields.io/badge/-tested-green) | ![Tested](https://img.shields.io/badge/-tested-green) | ![Tested](https://img.shields.io/badge/-tested-green) | ![Tested](https://img.shields.io/badge/-tested-green) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | - | - | - |
| **Authorize.Net** | ![Tested](https://img.shields.io/badge/-tested-green) | ![Tested](https://img.shields.io/badge/-tested-green) | ![Tested](https://img.shields.io/badge/-tested-green) | ![Tested](https://img.shields.io/badge/-tested-green) | ![Tested](https://img.shields.io/badge/-tested-green) | - | ![Tested](https://img.shields.io/badge/-tested-green) | - |
| **Checkout.com** | ![Tested](https://img.shields.io/badge/-tested-green) | ![Tested](https://img.shields.io/badge/-tested-green) | ![Tested](https://img.shields.io/badge/-tested-green) | ![Tested](https://img.shields.io/badge/-tested-green) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | - | - | - |
| **Cybersource** | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | - | - | - |
| **Bank of America** | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | - | - | - |
| **Wells Fargo** | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | - | - | - |
| **Braintree** | ![Tested](https://img.shields.io/badge/-tested-green) | ![Tested](https://img.shields.io/badge/-tested-green) | ![Tested](https://img.shields.io/badge/-tested-green) | ![Tested](https://img.shields.io/badge/-tested-green) | - | - | - | ![Integrated](https://img.shields.io/badge/-integrated-blue) |
| **PayPal** | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | - | - | - |
| **ACI** | ![Tested](https://img.shields.io/badge/-tested-green) | ![Tested](https://img.shields.io/badge/-tested-green) | ![Tested](https://img.shields.io/badge/-tested-green) | ![Tested](https://img.shields.io/badge/-tested-green) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | - | - | - |
| **Airwallex** | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | - | ![Integrated](https://img.shields.io/badge/-integrated-blue) | - | - |
| **Shift4** | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | - | ![Integrated](https://img.shields.io/badge/-integrated-blue) | - | - | - | - |
| **Trustpayments** | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | - | - | - | - |
| **TSYS** | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | - | - | - | - |
| **Worldpay** | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | - | - | - | - |
| **Worldpay XML** | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | - | - | - | - |
| **Worldpay Vantiv** | ![Integrated](https://img.shields.io/badge/-integrated-blue) | - | - | ![Integrated](https://img.shields.io/badge/-integrated-blue) | - | - | - | - |
| **Fiserv** | ![Tested](https://img.shields.io/badge/-tested-green) | ![Tested](https://img.shields.io/badge/-tested-green) | ![Tested](https://img.shields.io/badge/-tested-green) | ![Tested](https://img.shields.io/badge/-tested-green) | - | - | - | - |
| **Fiserv MEA** | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | - | - | - | - |
| **Nexinets** | ![Tested](https://img.shields.io/badge/-tested-green) | ![Tested](https://img.shields.io/badge/-tested-green) | ![Tested](https://img.shields.io/badge/-tested-green) | ![Tested](https://img.shields.io/badge/-tested-green) | - | - | - | - |
| **Nexixpay** | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | - | - | - | - |
| **Noon** | ![Tested](https://img.shields.io/badge/-tested-green) | ![Tested](https://img.shields.io/badge/-tested-green) | ![Tested](https://img.shields.io/badge/-tested-green) | ![Tested](https://img.shields.io/badge/-tested-green) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | - | - | - |
| **Payme** | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | - | ![Integrated](https://img.shields.io/badge/-integrated-blue) | - | - |
| **Trustpay** | ![Integrated](https://img.shields.io/badge/-integrated-blue) | - | - | ![Integrated](https://img.shields.io/badge/-integrated-blue) | - | ![Integrated](https://img.shields.io/badge/-integrated-blue) | - | - |
| **Rapyd** | ![Tested](https://img.shields.io/badge/-tested-green) | ![Tested](https://img.shields.io/badge/-tested-green) | ![Tested](https://img.shields.io/badge/-tested-green) | ![Tested](https://img.shields.io/badge/-tested-green) | - | - | - | - |
| **Revolut** | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | - | ![Integrated](https://img.shields.io/badge/-integrated-blue) | - | - | - | - |
| **Revolv3** | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | - | - | - |
| **Bambora** | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | - | - | - | - |
| **Bambora APAC** | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | - | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | - | - | - |
| **Bluesnap** | ![Tested](https://img.shields.io/badge/-tested-green) | ![Tested](https://img.shields.io/badge/-tested-green) | ![Tested](https://img.shields.io/badge/-tested-green) | ![Tested](https://img.shields.io/badge/-tested-green) | - | - | - | - |
| **Authipay** | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | - | - | - | - |
| **Barclaycard** | ![Tested](https://img.shields.io/badge/-tested-green) | ![Tested](https://img.shields.io/badge/-tested-green) | ![Tested](https://img.shields.io/badge/-tested-green) | ![Tested](https://img.shields.io/badge/-tested-green) | - | - | - | - |
| **Billwerk** | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | - | - | - | ![Integrated](https://img.shields.io/badge/-integrated-blue) |
| **Calida** | ![Integrated](https://img.shields.io/badge/-integrated-blue) | - | - | ![Integrated](https://img.shields.io/badge/-integrated-blue) | - | - | - | - |
| **Celero** | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | - | - | - | - |
| **Cryptopay** | ![Tested](https://img.shields.io/badge/-tested-green) | - | - | ![Tested](https://img.shields.io/badge/-tested-green) | - | - | - | - |
| **Datatrans** | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | - | - | - | - |
| **DLocal** | ![Tested](https://img.shields.io/badge/-tested-green) | ![Tested](https://img.shields.io/badge/-tested-green) | ![Tested](https://img.shields.io/badge/-tested-green) | ![Tested](https://img.shields.io/badge/-tested-green) | - | - | - | - |
| **Elavon** | ![Tested](https://img.shields.io/badge/-tested-green) | ![Tested](https://img.shields.io/badge/-tested-green) | - | ![Tested](https://img.shields.io/badge/-tested-green) | - | - | - | - |
| **Forte** | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | - | - | - | - |
| **Getnet** | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | - | - | - | - |
| **GlobalPay** | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | - | - | - | - |
| **Helcim** | ![Tested](https://img.shields.io/badge/-tested-green) | ![Tested](https://img.shields.io/badge/-tested-green) | ![Tested](https://img.shields.io/badge/-tested-green) | ![Tested](https://img.shields.io/badge/-tested-green) | - | - | - | - |
| **HiPay** | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | - | - | - | ![Integrated](https://img.shields.io/badge/-integrated-blue) |
| **HyperPG** | ![Integrated](https://img.shields.io/badge/-integrated-blue) | - | - | ![Integrated](https://img.shields.io/badge/-integrated-blue) | - | - | - | - |
| **IataPay** | ![Integrated](https://img.shields.io/badge/-integrated-blue) | - | - | ![Integrated](https://img.shields.io/badge/-integrated-blue) | - | - | - | - |
| **JP Morgan** | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | - | - | - | - |
| **Loonio** | ![Integrated](https://img.shields.io/badge/-integrated-blue) | - | - | ![Integrated](https://img.shields.io/badge/-integrated-blue) | - | - | - | - |
| **Mifinity** | ![Tested](https://img.shields.io/badge/-tested-green) | - | - | ![Tested](https://img.shields.io/badge/-tested-green) | - | - | - | - |
| **Mollie** | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | - | - | - | ![Integrated](https://img.shields.io/badge/-integrated-blue) |
| **Multisafepay** | ![Integrated](https://img.shields.io/badge/-integrated-blue) | - | - | ![Integrated](https://img.shields.io/badge/-integrated-blue) | - | - | - | - |
| **NMI** | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | - | - | - | - |
| **Novalnet** | ![Tested](https://img.shields.io/badge/-tested-green) | ![Tested](https://img.shields.io/badge/-tested-green) | ![Tested](https://img.shields.io/badge/-tested-green) | ![Tested](https://img.shields.io/badge/-tested-green) | ![Tested](https://img.shields.io/badge/-tested-green) | - | - | - |
| **Nuvei** | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | - | - | - | - |
| **Paybox** | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | - | - | - | - |
| **Payeezy** | ![Integrated](https://img.shields.io/badge/-integrated-blue) | - | - | - | - | - | - | - |
| **Payme** | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | - | - | - | - |
| **PaySafe** | ![Tested](https://img.shields.io/badge/-tested-green) | ![Tested](https://img.shields.io/badge/-tested-green) | ![Tested](https://img.shields.io/badge/-tested-green) | ![Tested](https://img.shields.io/badge/-tested-green) | - | - | - | ![Tested](https://img.shields.io/badge/-tested-green) |
| **Paytm** | ![Integrated](https://img.shields.io/badge/-integrated-blue) | - | - | ![Integrated](https://img.shields.io/badge/-integrated-blue) | - | - | - | - |
| **PayU** | ![Integrated](https://img.shields.io/badge/-integrated-blue) | - | - | ![Integrated](https://img.shields.io/badge/-integrated-blue) | - | - | - | - |
| **PhonePe** | ![Integrated](https://img.shields.io/badge/-integrated-blue) | - | - | ![Integrated](https://img.shields.io/badge/-integrated-blue) | - | - | - | - |
| **Placetopay** | ![Tested](https://img.shields.io/badge/-tested-green) | ![Tested](https://img.shields.io/badge/-tested-green) | ![Tested](https://img.shields.io/badge/-tested-green) | ![Tested](https://img.shields.io/badge/-tested-green) | - | - | - | - |
| **Powertranz** | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | - | - | - | - |
| **Razorpay** | ![Tested](https://img.shields.io/badge/-tested-green) | ![Tested](https://img.shields.io/badge/-tested-green) | ![Tested](https://img.shields.io/badge/-tested-green) | ![Tested](https://img.shields.io/badge/-tested-green) | - | - | - | - |
| **Redsys** | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | - | - | - | - | - |
| **Silverflow** | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | - | - | - | - |
| **Stax** | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | - | - | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) |
| **Volt** | ![Integrated](https://img.shields.io/badge/-integrated-blue) | - | - | ![Integrated](https://img.shields.io/badge/-integrated-blue) | - | - | - | - |
| **Xendit** | ![Tested](https://img.shields.io/badge/-tested-green) | ![Tested](https://img.shields.io/badge/-tested-green) | - | ![Tested](https://img.shields.io/badge/-tested-green) | - | - | - | - |
| **Zift** | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | - | - | - |
| **Cashfree** | ![Integrated](https://img.shields.io/badge/-integrated-blue) | - | - | - | - | ![Integrated](https://img.shields.io/badge/-integrated-blue) | - | - |
| **CashToCode** | ![Tested](https://img.shields.io/badge/-tested-green) | - | - | - | - | - | - | - |
| **Fiuu** | ![Tested](https://img.shields.io/badge/-tested-green) | ![Tested](https://img.shields.io/badge/-tested-green) | ![Tested](https://img.shields.io/badge/-tested-green) | ![Tested](https://img.shields.io/badge/-tested-green) | - | - | - | - |
| **Gigadat** | ![Integrated](https://img.shields.io/badge/-integrated-blue) | - | - | ![Integrated](https://img.shields.io/badge/-integrated-blue) | - | - | - | - |
| **Payload** | ![Tested](https://img.shields.io/badge/-tested-green) | ![Tested](https://img.shields.io/badge/-tested-green) | ![Tested](https://img.shields.io/badge/-tested-green) | ![Tested](https://img.shields.io/badge/-tested-green) | ![Tested](https://img.shields.io/badge/-tested-green) | - | - | - |

### Additional Payment Features

| Connector | Incremental Auth | Void Post Capture | Repeat Payment | Verify 3DS |
|-----------|:----------------:|:-----------------:|:--------------:|:----------:|
| **Stripe** | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | - |

---

## RefundService

The RefundService handles refund operations for processed payments.

| Connector | Refund | Refund Sync (RSync) |
|-----------|:------:|:-------------------:|
| **Stripe** | ![Tested](https://img.shields.io/badge/-tested-green) | ![Tested](https://img.shields.io/badge/-tested-green) |
| **Adyen** | ![Integrated](https://img.shields.io/badge/-integrated-blue) | - |
| **Authorize.Net** | ![Tested](https://img.shields.io/badge/-tested-green) | ![Tested](https://img.shields.io/badge/-tested-green) |
| **Checkout.com** | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) |
| **Cybersource** | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) |
| **Bank of America** | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) |
| **Wells Fargo** | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) |
| **Braintree** | ![Tested](https://img.shields.io/badge/-tested-green) | ![Tested](https://img.shields.io/badge/-tested-green) |
| **PayPal** | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) |
| **ACI** | ![Tested](https://img.shields.io/badge/-tested-green) | ![Tested](https://img.shields.io/badge/-tested-green) |
| **Airwallex** | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) |
| **Shift4** | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) |
| **Trustpayments** | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) |
| **TSYS** | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) |
| **Worldpay** | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) |
| **Worldpay XML** | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) |
| **Fiserv** | ![Tested](https://img.shields.io/badge/-tested-green) | ![Tested](https://img.shields.io/badge/-tested-green) |
| **Fiserv MEA** | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) |
| **Nexinets** | ![Tested](https://img.shields.io/badge/-tested-green) | ![Tested](https://img.shields.io/badge/-tested-green) |
| **Nexixpay** | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) |
| **Noon** | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) |
| **Payme** | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) |
| **Trustpay** | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) |
| **Rapyd** | ![Tested](https://img.shields.io/badge/-tested-green) | ![Tested](https://img.shields.io/badge/-tested-green) |
| **Revolut** | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) |
| **Revolv3** | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) |
| **Bambora** | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) |
| **Bambora APAC** | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) |
| **Bluesnap** | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) |
| **Authipay** | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) |
| **Barclaycard** | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) |
| **Billwerk** | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) |
| **Celero** | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) |
| **Datatrans** | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) |
| **DLocal** | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) |
| **Elavon** | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) |
| **Forte** | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) |
| **Getnet** | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) |
| **GlobalPay** | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) |
| **Helcim** | ![Tested](https://img.shields.io/badge/-tested-green) | ![Tested](https://img.shields.io/badge/-tested-green) |
| **HiPay** | ![Integrated](https://img.shields.io/badge/-integrated-blue) | - |
| **HyperPG** | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) |
| **IataPay** | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) |
| **JP Morgan** | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) |
| **Mollie** | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) |
| **Multisafepay** | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) |
| **NMI** | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) |
| **Novalnet** | ![Tested](https://img.shields.io/badge/-tested-green) | ![Tested](https://img.shields.io/badge/-tested-green) |
| **Nuvei** | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) |
| **Paybox** | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) |
| **PaySafe** | ![Tested](https://img.shields.io/badge/-tested-green) | ![Tested](https://img.shields.io/badge/-tested-green) |
| **Placetopay** | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) |
| **Powertranz** | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) |
| **Silverflow** | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) |
| **Stax** | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) |
| **Xendit** | ![Tested](https://img.shields.io/badge/-tested-green) | ![Tested](https://img.shields.io/badge/-tested-green) |
| **Zift** | ![Integrated](https://img.shields.io/badge/-integrated-blue) | - |
| **Fiuu** | ![Tested](https://img.shields.io/badge/-tested-green) | ![Tested](https://img.shields.io/badge/-tested-green) |
| **Gigadat** | ![Integrated](https://img.shields.io/badge/-integrated-blue) | - |
| **Payload** | ![Tested](https://img.shields.io/badge/-tested-green) | ![Tested](https://img.shields.io/badge/-tested-green) |

---

## DisputeService

The DisputeService handles chargeback and dispute management operations.

| Connector | Accept Dispute | Defend Dispute | Submit Evidence |
|-----------|:--------------:|:--------------:|:---------------:|
| **Adyen** | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) |
| **Stripe** | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) | ![Integrated](https://img.shields.io/badge/-integrated-blue) |

---

## Connector Details

### Stripe
- **Location**: `backend/connector-integration/src/connectors/stripe.rs`
- **Transformers**: `backend/connector-integration/src/connectors/stripe/transformers.rs`
- **Tests**: `backend/grpc-server/tests/stripe_payment_flows_test.rs`
- **Supported Operations**: Authorize, Capture, Void, PSync, Refund, RSync, SetupMandate, CreateConnectorCustomer, PaymentMethodToken, IncrementalAuthorization, VoidPostCapture, RepeatPayment, AcceptDispute, DefendDispute, SubmitEvidence

### Adyen
- **Location**: `backend/connector-integration/src/connectors/adyen.rs`
- **Transformers**: `backend/connector-integration/src/connectors/adyen/transformers.rs`
- **Tests**: `backend/grpc-server/tests/beta_tests/adyen_dispute_webhook_test.rs`
- **Unit Tests**: `backend/connector-integration/src/connectors/adyen/test.rs`
- **Supported Operations**: Authorize, Capture, Void, PSync, Refund, SetupMandate, AcceptDispute, DefendDispute, SubmitEvidence

### Authorize.Net
- **Location**: `backend/connector-integration/src/connectors/authorizedotnet.rs`
- **Transformers**: `backend/connector-integration/src/connectors/authorizedotnet/transformers.rs`
- **Tests**: `backend/grpc-server/tests/authorizedotnet_payment_flows_test.rs`, `backend/grpc-server/tests/beta_tests/authorizedotnet_webhook_test.rs`
- **Supported Operations**: Authorize, Capture, Void, PSync, Refund, RSync, SetupMandate, CreateConnectorCustomer

### ACI
- **Location**: `backend/connector-integration/src/connectors/aci.rs`
- **Transformers**: `backend/connector-integration/src/connectors/aci/transformers.rs`
- **Tests**: `backend/grpc-server/tests/beta_tests/aci_payment_flows_test.rs`
- **Supported Operations**: Authorize, Capture, Void, PSync, Refund, SetupMandate, RepeatPayment

### Braintree
- **Location**: `backend/connector-integration/src/connectors/braintree.rs`
- **Transformers**: `backend/connector-integration/src/connectors/braintree/transformers.rs`
- **Tests**: `backend/grpc-server/tests/beta_tests/braintree_payment_flows_test.rs`
- **Supported Operations**: Authorize, Capture, Void, PSync, Refund, RSync, PaymentMethodToken

### Checkout.com
- **Location**: `backend/connector-integration/src/connectors/checkout.rs`
- **Transformers**: `backend/connector-integration/src/connectors/checkout/transformers.rs`
- **Tests**: `backend/grpc-server/tests/beta_tests/checkout_payment_flows_test.rs`
- **Supported Operations**: Authorize, Capture, Void, PSync, Refund, RSync

### PaySafe
- **Location**: `backend/connector-integration/src/connectors/paysafe.rs`
- **Transformers**: `backend/connector-integration/src/connectors/paysafe/transformers.rs`
- **Tests**: `backend/grpc-server/tests/paysafe_payment_flows_test.rs`
- **Supported Operations**: Authorize, Capture, Void, PSync, Refund, RSync, PaymentMethodToken

### Bluesnap
- **Location**: `backend/connector-integration/src/connectors/bluesnap.rs`
- **Transformers**: `backend/connector-integration/src/connectors/bluesnap/transformers.rs`
- **Tests**: `backend/grpc-server/tests/beta_tests/bluesnap_payment_flows_test.rs`
- **Supported Operations**: Authorize, Capture, Void, PSync, Refund, RSync

### Bluecode
- **Location**: `backend/connector-integration/src/connectors/bluecode.rs`
- **Transformers**: `backend/connector-integration/src/connectors/bluecode/transformers.rs`
- **Tests**: `backend/grpc-server/tests/beta_tests/bluecode_payment_flows_test.rs`
- **Supported Operations**: Authorize, PSync

### Rapyd
- **Location**: `backend/connector-integration/src/connectors/rapyd.rs`
- **Transformers**: `backend/connector-integration/src/connectors/rapyd/transformers.rs`
- **Tests**: `backend/grpc-server/tests/beta_tests/rapyd_payment_flows_test.rs`
- **Supported Operations**: Authorize, Capture, Void, PSync, Refund, RSync

### Fiserv
- **Location**: `backend/connector-integration/src/connectors/fiserv.rs`
- **Transformers**: `backend/connector-integration/src/connectors/fiserv/transformers.rs`
- **Tests**: `backend/grpc-server/tests/beta_tests/fiserv_payment_flows_test.rs`
- **Supported Operations**: Authorize, Capture, Void, PSync, Refund, RSync

### DLocal
- **Location**: `backend/connector-integration/src/connectors/dlocal.rs`
- **Transformers**: `backend/connector-integration/src/connectors/dlocal/transformers.rs`
- **Tests**: `backend/grpc-server/tests/beta_tests/dlocal_payment_flows_test.rs`
- **Supported Operations**: Authorize, Capture, Void, PSync, Refund, RSync

### Nexinets
- **Location**: `backend/connector-integration/src/connectors/nexinets.rs`
- **Transformers**: `backend/connector-integration/src/connectors/nexinets/transformers.rs`
- **Tests**: `backend/grpc-server/tests/beta_tests/nexinets_payment_flows_test.rs`
- **Supported Operations**: Authorize, Capture, Void, PSync, Refund, RSync

### Elavon
- **Location**: `backend/connector-integration/src/connectors/elavon.rs`
- **Transformers**: `backend/connector-integration/src/connectors/elavon/transformers.rs`
- **Tests**: `backend/grpc-server/tests/beta_tests/elavon_payment_flows_test.rs`
- **Supported Operations**: Authorize, Capture, PSync, Refund, RSync

### Noon
- **Location**: `backend/connector-integration/src/connectors/noon.rs`
- **Transformers**: `backend/connector-integration/src/connectors/noon/transformers.rs`
- **Tests**: `backend/grpc-server/tests/beta_tests/noon_payment_flows_test.rs`
- **Supported Operations**: Authorize, Capture, Void, PSync, Refund, RSync

### Placetopay
- **Location**: `backend/connector-integration/src/connectors/placetopay.rs`
- **Transformers**: `backend/connector-integration/src/connectors/placetopay/transformers.rs`
- **Tests**: `backend/grpc-server/tests/beta_tests/placetopay_payment_flows_test.rs`
- **Supported Operations**: Authorize, Capture, Void, PSync, Refund, RSync

### MiFinity
- **Location**: `backend/connector-integration/src/connectors/mifinity.rs`
- **Transformers**: `backend/connector-integration/src/connectors/mifinity/transformers.rs`
- **Tests**: `backend/grpc-server/tests/beta_tests/mifinity_payment_flows_test.rs`
- **Supported Operations**: Authorize, PSync

### Cryptopay
- **Location**: `backend/connector-integration/src/connectors/cryptopay.rs`
- **Transformers**: `backend/connector-integration/src/connectors/cryptopay/transformers.rs`
- **Tests**: `backend/grpc-server/tests/beta_tests/cryptopay_payment_flows_test.rs`
- **Supported Operations**: Authorize, PSync

### Xendit
- **Location**: `backend/connector-integration/src/connectors/xendit.rs`
- **Transformers**: `backend/connector-integration/src/connectors/xendit/transformers.rs`
- **Tests**: `backend/grpc-server/tests/xendit_payment_flows_test.rs`
- **Supported Operations**: Authorize, Capture, PSync, Refund, RSync

### Helcim
- **Location**: `backend/connector-integration/src/connectors/helcim.rs`
- **Transformers**: `backend/connector-integration/src/connectors/helcim/transformers.rs`
- **Tests**: `backend/grpc-server/tests/helcim_payment_flows_test.rs`
- **Supported Operations**: Authorize, Capture, Void, PSync, Refund, RSync

### Novalnet
- **Location**: `backend/connector-integration/src/connectors/novalnet.rs`
- **Transformers**: `backend/connector-integration/src/connectors/novalnet/transformers.rs`
- **Tests**: `backend/grpc-server/tests/novalnet_payment_flows_test.rs`
- **Supported Operations**: Authorize, Capture, Void, PSync, Refund, RSync, SetupMandate, RepeatPayment

### Fiuu
- **Location**: `backend/connector-integration/src/connectors/fiuu.rs`
- **Transformers**: `backend/connector-integration/src/connectors/fiuu/transformers.rs`
- **Tests**: `backend/grpc-server/tests/fiuu_payment_flows_test.rs`
- **Supported Operations**: Authorize, Capture, Void, PSync, Refund, RSync

### CashToCode
- **Location**: `backend/connector-integration/src/connectors/cashtocode.rs`
- **Transformers**: `backend/connector-integration/src/connectors/cashtocode/transformers.rs`
- **Tests**: `backend/grpc-server/tests/cashtocode_payment_flows_test.rs`
- **Supported Operations**: Authorize

### Payload
- **Location**: `backend/connector-integration/src/connectors/payload.rs`
- **Transformers**: `backend/connector-integration/src/connectors/payload/transformers.rs`
- **Tests**: `backend/grpc-server/tests/payload_payment_flows_test.rs`
- **Supported Operations**: Authorize, Capture, Void, PSync, Refund, RSync

### Razorpay
- **Location**: `backend/connector-integration/src/connectors/razorpay.rs`
- **Transformers**: `backend/connector-integration/src/connectors/razorpay/transformers.rs`
- **Unit Tests**: `backend/connector-integration/src/connectors/razorpay/test.rs`
- **Supported Operations**: Authorize, Capture, Void, PSync, Refund, RSync, CreateOrder, CreateAccessToken, CreateConnectorCustomer

### Razorpay V2
- **Location**: `backend/connector-integration/src/connectors/razorpayv2.rs`
- **Transformers**: `backend/connector-integration/src/connectors/razorpayv2/transformers.rs`
- **Unit Tests**: `backend/connector-integration/src/connectors/razorpayv2/test.rs`
- **Supported Operations**: Authorize, Capture, Void, PSync, Refund, RSync, CreateOrder

### Cashfree
- **Location**: `backend/connector-integration/src/connectors/cashfree.rs`
- **Transformers**: `backend/connector-integration/src/connectors/cashfree/transformers.rs`
- **Unit Tests**: `backend/connector-integration/src/connectors/cashfree/test.rs`
- **Supported Operations**: Authorize, CreateOrder

### Calida
- **Location**: `backend/connector-integration/src/connectors/calida.rs`
- **Transformers**: `backend/connector-integration/src/connectors/calida/transformers.rs`
- **Unit Tests**: `backend/connector-integration/src/connectors/calida/test.rs`
- **Supported Operations**: Authorize, PSync

### Barclaycard
- **Location**: `backend/connector-integration/src/connectors/barclaycard.rs`
- **Transformers**: `backend/connector-integration/src/connectors/barclaycard/transformers.rs`
- **Tests**: `backend/grpc-server/tests/beta_tests/barclaycard_payment_flows_test.rs`
- **Supported Operations**: Authorize, Capture, Void, PSync, Refund, RSync

---

## Contributing

To add a new connector:

1. Create a new file in `backend/connector-integration/src/connectors/{connector_name}.rs`
2. Implement the required traits for each operation
3. Add corresponding transformers in `backend/connector-integration/src/connectors/{connector_name}/transformers.rs`
4. Add tests in `backend/grpc-server/tests/{connector_name}_payment_flows_test.rs`
5. Update this documentation with the new connector's status
