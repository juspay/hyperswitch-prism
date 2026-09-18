# Kill Bill Hyperswitch Prism plugin

A [Kill Bill](https://killbill.io) payment plugin that routes payments through
**[Hyperswitch Prism](https://github.com/juspay/hyperswitch-prism)** — the unified connector library. One
plugin gives a Kill Bill deployment access to Prism's entire connector catalogue (Stripe, Adyen, Cybersource,
100+) through a single integration and one request schema.

This is the **Java/OSGi analog of [`plugins/medusa`](../medusa)**: it implements Kill Bill's `PaymentPluginApi`
once and delegates to Prism. Like the Medusa plugin, it **embeds the Prism SDK in-process** — here the
`io.hyperswitch:prism` Java SDK (JNA → native FFI), the Java equivalent of Medusa's `koffi`-based Node SDK.
There is **no Prism server/sidecar**; the plugin calls the SDK directly.

## Architecture

```
Kill Bill core ─▶ HyperswitchPaymentPluginApi (extends PluginPaymentPluginApi)   host adapter (thin)
                     │  delegates every flow
                     ▼
                  PrismClient (interface) ──▶ SdkPrismClient   embeds io.hyperswitch:prism, IN-PROCESS
                     │                          PaymentClient / RefundClient / RecurringPaymentClient /
                     │                          PaymentMethodClient / EventClient / MerchantAuthenticationClient
                     ▼  JNA → native Rust FFI (transform) → OkHttp
                  Stripe / Adyen / Cybersource / …
```

Local state (Kill Bill ids ↔ Prism `connectorTransactionId` / mandate / token / status) is persisted in two
jOOQ tables so `getPaymentInfo`/`searchPayments` work without a live connector call.

`PrismClient` is an interface so a sidecar transport (Prism `grpc-server` with `[server] type="http"`) can be
dropped in behind it if embedding the native SDK in Kill Bill's Felix/OSGi container ever proves unworkable —
without touching the payment-plugin logic.

## Requirements

- **JDK 17+** (required by `io.hyperswitch:prism`). Confirm the Kill Bill runtime is on JDK 17+.
- A platform-native Prism FFI library bundled in the SDK jar (or built from source — see risks below).
- Kill Bill `0.24.x` (`killbill-oss-parent:0.146.63`).

## Build

```bash
# 1. Generate the jOOQ DAO sources (once, or whenever ddl.sql changes):
#    apply src/main/resources/ddl.sql to a local MySQL/MariaDB named `killbill`, then
mvn -Pgenerate-jooq generate-sources     # writes src/main/java/.../dao/gen/**  → commit it

# 2. Build the OSGi bundle:
mvn clean install
```

The bundle embeds the Prism SDK and its runtime stack (`Embed-Dependency` in `pom.xml`); the native FFI lib
rides inside the SDK jar and JNA extracts it at runtime.

## Install & configure

```bash
kpm install_java_plugin killbill-hyperswitch --from-source-file=target/hyperswitch-plugin-*.jar
```

Configure per tenant (see `src/main/resources/hyperswitch.properties` for all keys), e.g. via
`POST /1.0/kb/tenants/uploadPluginConfig/killbill-hyperswitch`:

```
org.killbill.billing.plugin.hyperswitch.connector=stripe
org.killbill.billing.plugin.hyperswitch.environment=SANDBOX
org.killbill.billing.plugin.hyperswitch.stripe.apiKey=sk_test_xxx
org.killbill.billing.plugin.hyperswitch.webhookSecret=whsec_xxx
```

Single connector per tenant (same model as the Medusa plugin).

## Supported connectors

Scoped for **parity with KillBill's own payment plugins** — the gateways a real KillBill deployment uses. Set
`…hyperswitch.connector=<name>` and the connector's namespaced credential keys (see `hyperswitch.properties`);
switching connector is a config change only.

| Connector | KillBill plugin it replaces | Required credential keys (prefix `…hyperswitch.<connector>.`) |
|---|---|---|
| `stripe` | killbill-stripe-plugin | `apiKey` |
| `adyen` | killbill-adyen-plugin | `apiKey`, `merchantAccount` |
| `braintree` | killbill-braintree-blue-plugin | `publicKey`, `privateKey` |
| `cybersource` | killbill-cybersource-plugin | `apiKey`, `merchantAccount`, `apiSecret` |
| `paypal` | killbill-paypal-express-plugin | `clientId`, `clientSecret` |
| `forte` | killbill-forte-plugin | `apiAccessId`, `organizationId`, `locationId`, `apiSecretKey` |

All also accept an optional `baseUrl` (plus a few optional keys — see `hyperswitch.properties`).

**Not available via Prism** (KillBill has these, Prism has no equivalent connector — configuring them throws
`Unsupported connector`): **GoCardless, Qualpay, Dwolla, Orbital, SecureNet** (direct-debit / ACH / regional).
Supporting them would require adding the connector to Prism upstream (Rust side).

## Testing end-to-end

Once the plugin is installed in a running KillBill, drive the full flow (tenant → account → payment method →
purchase → refund → webhook) with the API test harness in [`test-harness/`](./test-harness):

```bash
cd test-harness && STRIPE_API_KEY=sk_test_xxx ./run.sh
```

It's the server-to-server analog of the Medusa demo app (no browser — KillBill has no storefront). Also
includes `killbill-flow.http` for the VS Code REST Client / IntelliJ HTTP Client. See
[`test-harness/README.md`](./test-harness/README.md).

## P0 feasibility spike (do this first)

Before implementing the flows, prove the embedded native SDK loads inside Kill Bill's Felix container:

1. `mvn clean install` produces the OSGi bundle.
2. Install it into a running Kill Bill; confirm the plugin appears in the plugin list and config upload works.
3. Make **one live `PaymentClient.authorize`** to a Stripe sandbox key (a smoke-test route or a throwaway main)
   and confirm the JNA native lib loads and a status comes back.

If step 3 hits an intractable JNA/classloader wall, add an `HttpPrismClient` (sidecar) implementation behind
`PrismClient` — the rest of the plugin is unaffected.

## Status: work in progress

| Phase | Scope | State |
|------|-------|-------|
| P0 | Scaffold + build + OSGi feasibility spike | **this scaffold** |
| P1 | authorize / capture / purchase / void / refund / getPaymentInfo | flows stubbed (`NOT_IMPLEMENTED`) |
| P2 | Payment-method storage / tokenization | pending |
| P3 | Webhooks (`processNotification` + servlet → `EventService.handle_event`) | servlet stub |
| P4 | Recurring / mandates (`setup_recurring` + MIT charge) | pending |

## Must-verify before relying on this (see the plan's risk list)

1. **SDK package names.** The Java files import the in-repo Gradle layout (`payments.*`, `types.Payment.*`).
   The published `io.hyperswitch:prism:0.0.6` may expose `com.hyperswitch.payments.*` — reconcile at first
   compile (P0). Every SDK-facing file carries a `NOTE (P0 verification)` banner.
2. **Native lib packaging.** Confirm the Maven Central jar bundles a native FFI for your platform (linux
   x64/arm64); if not, build the FFI from source in CI and place it on the JNA path. The `Embed-Dependency`
   artifact list in `pom.xml` must match the SDK's actual transitive deps.
3. **PCI scope.** Embedding the SDK keeps card data and credentials inside the Kill Bill JVM (as Medusa keeps
   them in its Node process). Tokenize/create mandates early to minimize raw-card handling.

## License

Apache-2.0
