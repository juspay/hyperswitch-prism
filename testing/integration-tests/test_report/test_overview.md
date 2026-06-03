# UCS Connector Test Report

> Generated: epoch 1776156341

**Summary**: 81 connectors discovered, 5 tested | 87 passed, 58 failed across 145 scenarios

## Connector Flow Matrix

Legend: percentage = tested (links to details), `—` = supported but not yet tested, `-` = not supported

| Connector | Authorize | Capture | Void | Refund | Payment Sync | Refund Sync | Complete Auth | Setup Mandate | Mandate Pay | Revoke Mandate | Create Token | Customer | Pre Auth | Auth | Post Auth | SDK Session | Tokenize PM | Create Order | EventService/HandleEvent | Session Token | Incremental Auth | PaymentService/ProxyAuthorize | PaymentService/ProxySetupRecurring | PaymentService/TokenAuthorize | PaymentService/TokenSetupRecurring |
|:----------|:------:|:------:|:------:|:------:|:------:|:------:|:------:|:------:|:------:|:------:|:------:|:------:|:------:|:------:|:------:|:------:|:------:|:------:|:------:|:------:|:------:|:------:|:------:|:------:|:------:|
| `aci` | — | — | — | — | — | - | - | — | — | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - |
| `adyen` | — | — | — | — | — | - | - | — | — | - | - | - | - | - | - | — | - | — | [0.0%](./connectors/adyen/eventservice-handleevent.md) | - | - | - | - | - | - |
| `airwallex` | — | — | — | — | — | — | - | - | - | - | — | - | - | - | - | - | - | — | - | - | - | - | - | - | - |
| `authipay` | — | — | — | — | — | — | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - |
| `authorizedotnet` | — | — | — | — | — | — | - | — | — | - | - | — | - | - | - | - | - | - | — | - | - | - | - | - | - |
| `bambora` | — | — | — | — | — | — | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - |
| `bamboraapac` | — | — | - | — | — | — | - | — | — | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - |
| `bankofamerica` | — | — | — | — | — | — | - | — | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - |
| `barclaycard` | — | — | — | — | — | — | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - |
| `billwerk` | — | — | — | — | — | — | - | — | — | - | - | - | - | - | - | — | — | - | - | - | - | - | - | - | - |
| `bluesnap` | — | — | — | — | — | — | - | - | - | - | - | - | - | - | - | — | - | - | - | - | - | - | - | - | - |
| `braintree` | - | — | — | — | — | — | - | - | — | - | - | - | - | - | - | — | — | - | - | - | - | - | - | - | - |
| `calida` | — | - | - | - | — | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - |
| `cashfree` | — | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | — | - | - | - | - | - | - | - |
| `cashtocode` | — | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - |
| `celero` | — | — | — | — | — | — | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - |
| `checkout` | — | — | — | — | — | — | - | — | — | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - |
| `cryptopay` | — | - | - | - | — | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - |
| `cybersource` | — | — | — | — | — | — | - | — | — | - | - | - | — | — | — | — | - | - | - | - | - | - | - | - | - |
| `datatrans` | — | — | — | — | — | — | - | - | - | - | - | - | - | - | - | — | - | - | - | - | - | - | - | - | - |
| `dlocal` | — | — | — | — | — | — | - | - | — | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - |
| `elavon` | — | — | - | — | — | — | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - |
| `finix` | — | — | — | — | — | — | - | - | - | - | - | — | - | - | - | - | — | - | - | - | - | - | - | - | - |
| `fiserv` | — | — | — | — | — | — | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - |
| `fiservcommercehub` | — | - | — | — | — | — | - | - | - | - | — | - | - | - | - | - | - | - | - | - | - | - | - | - | - |
| `fiservemea` | — | — | — | — | — | — | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - |
| `fiuu` | — | — | — | — | — | — | - | - | — | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - |
| `forte` | — | — | — | — | — | — | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - |
| `getnet` | — | — | — | — | — | — | - | - | - | - | — | - | - | - | - | - | - | - | - | - | - | - | - | - | - |
| `gigadat` | — | - | - | — | — | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - |
| `globalpay` | — | — | — | — | — | — | - | - | - | - | - | - | - | - | - | — | - | - | - | - | - | - | - | - | - |
| `helcim` | — | — | — | — | — | — | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - |
| `hipay` | — | — | — | — | — | - | - | - | - | - | - | - | - | - | - | - | — | - | - | - | - | - | - | - | - |
| `hyperpg` | — | - | - | — | — | — | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - |
| `iatapay` | — | - | - | — | — | — | - | - | - | - | — | - | - | - | - | - | - | - | - | - | - | - | - | - | - |
| `itaubank` | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - |
| `jpmorgan` | — | — | — | — | — | — | - | - | - | - | — | - | - | - | - | — | - | - | - | - | - | - | - | - | - |
| `loonio` | — | - | - | - | — | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - |
| `mifinity` | — | - | - | - | — | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - |
| `mollie` | — | — | — | — | — | — | - | - | - | - | - | - | - | - | - | - | — | - | - | - | - | - | - | - | - |
| `multisafepay` | — | - | - | — | — | — | - | - | - | - | - | - | - | - | - | — | - | - | - | - | - | - | - | - | - |
| `nexinets` | — | — | — | — | — | — | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - |
| `nexixpay` | — | — | — | — | — | — | — | - | - | - | - | - | — | - | — | — | - | - | - | - | - | - | - | - | - |
| `nmi` | — | — | — | — | — | — | - | - | - | - | - | - | — | - | - | - | - | - | - | - | - | - | - | - | - |
| `noon` | — | — | — | — | — | — | - | — | — | — | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - |
| `novalnet` | — | — | — | — | — | — | - | — | — | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - |
| `nuvei` | [26.3%](./connectors/nuvei/paymentservice-authorize.md) | [100.0%](./connectors/nuvei/paymentservice-capture.md) | [100.0%](./connectors/nuvei/paymentservice-void.md) | [100.0%](./connectors/nuvei/paymentservice-refund.md) | [0.0%](./connectors/nuvei/paymentservice-get.md) | [0.0%](./connectors/nuvei/refundservice-get.md) | - | - | - | - | - | - | - | - | - | [75.0%](./connectors/nuvei/merchantauthenticationservice-createclientauthenticationtoken.md) | - | — | - | [66.7%](./connectors/nuvei/merchantauthenticationservice-createserversessionauthenticationtoken.md) | - | - | - | - | - |
| `paybox` | — | — | — | — | — | — | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - |
| `payload` | — | — | — | — | — | — | - | — | — | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - |
| `payme` | — | — | — | — | — | — | - | - | - | - | - | - | - | - | - | - | - | — | - | - | - | - | - | - | - |
| `paypal` | — | — | — | — | — | — | — | — | — | - | — | - | - | - | - | - | - | — | [50.0%](./connectors/paypal/eventservice-handleevent.md) | - | - | - | - | - | - |
| `paysafe` | — | — | — | — | — | — | - | - | — | - | - | - | - | - | - | - | — | - | - | - | - | - | - | - | - |
| `paytm` | — | - | - | - | — | - | - | - | - | - | - | - | - | - | - | - | - | - | - | — | - | - | - | - | - |
| `payu` | — | - | - | - | — | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - |
| `peachpayments` | — | — | — | — | — | — | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - |
| `phonepe` | — | - | - | - | — | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - |
| `placetopay` | — | — | — | — | — | — | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - |
| `powertranz` | — | — | — | — | — | — | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - |
| `ppro` | — | — | — | — | — | — | - | — | — | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - |
| `rapyd` | [26.3%](./connectors/rapyd/paymentservice-authorize.md) | [100.0%](./connectors/rapyd/paymentservice-capture.md) | [0.0%](./connectors/rapyd/paymentservice-void.md) | [100.0%](./connectors/rapyd/paymentservice-refund.md) | [100.0%](./connectors/rapyd/paymentservice-get.md) | [100.0%](./connectors/rapyd/refundservice-get.md) | - | - | - | - | - | - | - | - | - | [100.0%](./connectors/rapyd/merchantauthenticationservice-createclientauthenticationtoken.md) | - | - | - | - | - | - | - | - | - |
| `razorpay` | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - |
| `razorpayv2` | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - |
| `redsys` | — | — | — | — | - | - | - | - | - | - | - | - | — | — | - | - | - | - | - | - | - | - | - | - | - |
| `revolut` | — | — | - | — | — | — | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - |
| `revolv3` | — | — | — | — | — | — | - | — | — | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - |
| `shift4` | — | — | - | — | — | — | - | - | — | - | - | — | - | - | - | - | - | - | - | - | - | - | - | - | - |
| `silverflow` | — | — | — | — | — | — | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - |
| `stax` | — | — | — | — | — | — | - | - | - | - | - | — | - | - | - | - | — | - | - | - | - | - | - | - | - |
| `stripe` | [89.5%](./connectors/stripe/paymentservice-authorize.md) | [100.0%](./connectors/stripe/paymentservice-capture.md) | [100.0%](./connectors/stripe/paymentservice-void.md) | [100.0%](./connectors/stripe/paymentservice-refund.md) | [100.0%](./connectors/stripe/paymentservice-get.md) | [100.0%](./connectors/stripe/refundservice-get.md) | [0.0%](./connectors/stripe/paymentservice-completeauthorize.md) | [100.0%](./connectors/stripe/paymentservice-setuprecurring.md) | [100.0%](./connectors/stripe/recurringpaymentservice-charge.md) | - | - | [100.0%](./connectors/stripe/customerservice-create.md) | - | - | - | [100.0%](./connectors/stripe/merchantauthenticationservice-createclientauthenticationtoken.md) | [60.0%](./connectors/stripe/paymentmethodservice-tokenize.md) | - | [0.0%](./connectors/stripe/eventservice-handleevent.md) | - | — | [0.0%](./connectors/stripe/paymentservice-proxyauthorize.md) | [0.0%](./connectors/stripe/paymentservice-proxysetuprecurring.md) | [0.0%](./connectors/stripe/paymentservice-tokenauthorize.md) | [0.0%](./connectors/stripe/paymentservice-tokensetuprecurring.md) |
| `truelayer` | — | - | — | — | — | — | - | - | - | - | — | - | - | - | - | - | - | - | - | - | - | - | - | - | - |
| `trustly` | — | - | - | — | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - |
| `trustpay` | — | - | - | — | — | — | - | - | - | - | — | - | - | - | - | - | - | — | - | - | - | - | - | - | - |
| `trustpayments` | — | — | — | — | — | — | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - |
| `tsys` | — | — | — | — | — | — | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - |
| `volt` | — | - | - | — | — | - | - | - | - | - | — | - | - | - | - | - | - | - | - | - | - | - | - | - | - |
| `wellsfargo` | — | — | — | — | — | — | - | — | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - |
| `worldpay` | — | — | — | — | — | — | - | - | — | - | - | - | — | - | — | - | - | - | - | - | - | - | - | - | - |
| `worldpayvantiv` | — | - | - | - | — | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - |
| `worldpayxml` | — | — | — | — | — | — | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - |
| `xendit` | — | — | - | — | — | — | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - |
| `zift` | — | — | — | — | — | - | - | — | — | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - | - |

> Each percentage links to connector-specific suite results.
