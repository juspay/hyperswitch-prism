/*
 * Copyright 2024 Juspay
 *
 * Licensed under the Apache License, version 2.0 (the "License"); you may not
 * use this file except in compliance with the License. You may obtain a copy
 * of the License at http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS, WITHOUT
 * WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied. See the
 * License for the specific language governing permissions and limitations
 * under the License.
 */
package org.killbill.billing.plugin.hyperswitch.client;

import java.util.UUID;
import java.util.concurrent.ConcurrentHashMap;

import org.killbill.billing.plugin.hyperswitch.HyperswitchConfigProperties;
import org.killbill.billing.plugin.hyperswitch.HyperswitchConfigPropertiesConfigurationHandler;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

// NOTE (P0 verification): SDK client/config classes use the in-repo Gradle package layout — `payments.*` for
// clients + config wrappers (per examples/stripe/stripe.kt) and `types.Payment.*` for proto messages. Reconcile
// against the published io.hyperswitch:prism:0.0.6 artifact (README advertises `com.hyperswitch.payments.*`)
// when first compiling during the P0 spike, and adjust the imports below accordingly.
import payments.ConnectorConfig;
import payments.ConnectorSpecificConfig;
import payments.Environment;
import payments.EventClient;
import payments.MerchantAuthenticationClient;
import payments.PaymentClient;
import payments.PaymentMethodClient;
import payments.RecurringPaymentClient;
import payments.RefundClient;
import payments.SdkOptions;
import payments.SecretString;
import types.Payment.AdyenConfig;
import types.Payment.BraintreeConfig;
import types.Payment.CybersourceConfig;
import types.Payment.EventServiceHandleRequest;
import types.Payment.EventServiceHandleResponse;
import types.Payment.ForteConfig;
import types.Payment.PaymentMethodServiceTokenizeRequest;
import types.Payment.PaymentMethodServiceTokenizeResponse;
import types.Payment.PaymentServiceAuthorizeRequest;
import types.Payment.PaymentServiceAuthorizeResponse;
import types.Payment.PaymentServiceCaptureRequest;
import types.Payment.PaymentServiceCaptureResponse;
import types.Payment.PaymentServiceGetRequest;
import types.Payment.PaymentServiceGetResponse;
import types.Payment.PaymentServiceRefundRequest;
import types.Payment.PaymentServiceSetupRecurringRequest;
import types.Payment.PaymentServiceSetupRecurringResponse;
import types.Payment.PaymentServiceTokenAuthorizeRequest;
import types.Payment.PaymentServiceTokenAuthorizeResponse;
import types.Payment.PaymentServiceVoidRequest;
import types.Payment.PaymentServiceVoidResponse;
import types.Payment.PaypalConfig;
import types.Payment.RecurringPaymentServiceChargeRequest;
import types.Payment.RecurringPaymentServiceChargeResponse;
import types.Payment.RefundResponse;
import types.Payment.RefundServiceGetRequest;
import types.Payment.StripeConfig;

/**
 * Primary {@link PrismClient} implementation: embeds the {@code io.hyperswitch:prism} Java SDK in-process
 * (JNA → native FFI), the Java analog of how {@code plugins/medusa} embeds the Node SDK.
 *
 * <p>Builds one {@link ConnectorConfig} + SDK client bundle per tenant from the resolved
 * {@link HyperswitchConfigProperties} (single connector per tenant, like Medusa's {@code {[connector]: cfg}}),
 * and caches it (SDK clients hold OkHttp connection pools, so they should be reused).
 */
public class SdkPrismClient implements PrismClient {

    private static final Logger logger = LoggerFactory.getLogger(SdkPrismClient.class);
    private static final UUID DEFAULT_TENANT = new UUID(0L, 0L);

    private final HyperswitchConfigPropertiesConfigurationHandler configHandler;
    private final ConcurrentHashMap<UUID, ClientBundle> bundlesByTenant = new ConcurrentHashMap<>();

    public SdkPrismClient(final HyperswitchConfigPropertiesConfigurationHandler configHandler) {
        this.configHandler = configHandler;
    }

    /**
     * Drop the cached client bundle for a tenant so the next call rebuilds it from fresh config. Wire this to the
     * plugin config-change event so credential/connector edits take effect without a restart.
     */
    public void invalidate(final UUID kbTenantId) {
        bundlesByTenant.remove(kbTenantId == null ? DEFAULT_TENANT : kbTenantId);
    }

    private ClientBundle bundle(final UUID kbTenantId) throws PrismClientException {
        final UUID key = kbTenantId == null ? DEFAULT_TENANT : kbTenantId;
        // computeIfAbsent can't throw checked exceptions, so build then cache.
        ClientBundle existing = bundlesByTenant.get(key);
        if (existing != null) {
            return existing;
        }
        final HyperswitchConfigProperties config = configHandler.getConfigurable(kbTenantId);
        final ConnectorConfig connectorConfig = buildConnectorConfig(config);
        final ClientBundle created = new ClientBundle(connectorConfig);
        existing = bundlesByTenant.putIfAbsent(key, created);
        return existing != null ? existing : created;
    }

    /**
     * Translate the KillBill config into a Prism {@link ConnectorConfig}, switching on the configured connector.
     * Faithful port of {@code examples/stripe/stripe.kt}'s {@code _defaultConfig} builder. Add connector cases as
     * they are onboarded.
     */
    ConnectorConfig buildConnectorConfig(final HyperswitchConfigProperties config) throws PrismClientException {
        final String connector = config.getConnector();
        if (connector == null) {
            throw new PrismClientException("No connector configured (org.killbill.billing.plugin.hyperswitch.connector)",
                                           "INVALID_CONFIGURATION", false, null);
        }

        final Environment environment = "PRODUCTION".equalsIgnoreCase(config.getEnvironment())
                                        ? Environment.PRODUCTION
                                        : Environment.SANDBOX;
        final ConnectorSpecificConfig.Builder specific = ConnectorSpecificConfig.newBuilder();

        switch (connector.toLowerCase()) {
            case "stripe": {
                final StripeConfig.Builder stripe = StripeConfig.newBuilder()
                        .setApiKey(secret(config.getConnectorProperty("apiKey")));
                final String baseUrl = config.getConnectorProperty("baseUrl");
                if (baseUrl != null) {
                    stripe.setBaseUrl(baseUrl);
                }
                specific.setStripe(stripe.build());
                break;
            }
            case "adyen": {
                final AdyenConfig.Builder adyen = AdyenConfig.newBuilder()
                        .setApiKey(secret(config.getConnectorProperty("apiKey")))
                        .setMerchantAccount(secret(config.getConnectorProperty("merchantAccount")));
                final String baseUrl = config.getConnectorProperty("baseUrl");
                if (baseUrl != null) {
                    adyen.setBaseUrl(baseUrl);
                }
                specific.setAdyen(adyen.build());
                break;
            }
            case "cybersource": {
                final CybersourceConfig.Builder cs = CybersourceConfig.newBuilder()
                        .setApiKey(secret(config.getConnectorProperty("apiKey")))
                        .setMerchantAccount(secret(config.getConnectorProperty("merchantAccount")))
                        .setApiSecret(secret(config.getConnectorProperty("apiSecret")));
                final String baseUrl = config.getConnectorProperty("baseUrl");
                if (baseUrl != null) {
                    cs.setBaseUrl(baseUrl);
                }
                specific.setCybersource(cs.build());
                break;
            }
            case "braintree": {
                // Server-side card processing only — the storefront Apple/Google-Pay array fields are not set.
                final BraintreeConfig.Builder bt = BraintreeConfig.newBuilder()
                        .setPublicKey(secret(config.getConnectorProperty("publicKey")))
                        .setPrivateKey(secret(config.getConnectorProperty("privateKey")));
                final String merchantAccountId = config.getConnectorProperty("merchantAccountId");
                if (merchantAccountId != null) {
                    bt.setMerchantAccountId(secret(merchantAccountId));
                }
                final String baseUrl = config.getConnectorProperty("baseUrl");
                if (baseUrl != null) {
                    bt.setBaseUrl(baseUrl);
                }
                specific.setBraintree(bt.build());
                break;
            }
            case "paypal": {
                final PaypalConfig.Builder pp = PaypalConfig.newBuilder()
                        .setClientId(secret(config.getConnectorProperty("clientId")))
                        .setClientSecret(secret(config.getConnectorProperty("clientSecret")));
                final String payerId = config.getConnectorProperty("payerId");
                if (payerId != null) {
                    pp.setPayerId(secret(payerId));
                }
                final String baseUrl = config.getConnectorProperty("baseUrl");
                if (baseUrl != null) {
                    pp.setBaseUrl(baseUrl);
                }
                specific.setPaypal(pp.build());
                break;
            }
            case "forte": {
                final ForteConfig.Builder forte = ForteConfig.newBuilder()
                        .setApiAccessId(secret(config.getConnectorProperty("apiAccessId")))
                        .setOrganizationId(secret(config.getConnectorProperty("organizationId")))
                        .setLocationId(secret(config.getConnectorProperty("locationId")))
                        .setApiSecretKey(secret(config.getConnectorProperty("apiSecretKey")));
                final String baseUrl = config.getConnectorProperty("baseUrl");
                if (baseUrl != null) {
                    forte.setBaseUrl(baseUrl);
                }
                specific.setForte(forte.build());
                break;
            }
            default:
                throw new PrismClientException("Unsupported connector: " + connector, "INVALID_CONFIGURATION", false, null);
        }

        return ConnectorConfig.newBuilder()
                .setOptions(SdkOptions.newBuilder().setEnvironment(environment).build())
                .setConnectorConfig(specific.build())
                .build();
    }

    private static SecretString secret(final String value) {
        return SecretString.newBuilder().setValue(value == null ? "" : value).build();
    }

    // ---------------------------------------------------------------------------------------------
    // PrismClient — delegate to the embedded SDK client bundle, wrapping SDK errors.
    // ---------------------------------------------------------------------------------------------

    @Override
    public PaymentServiceAuthorizeResponse authorize(final UUID t, final PaymentServiceAuthorizeRequest r) throws PrismClientException {
        try {
            return bundle(t).payment.authorize(r);
        } catch (final PrismClientException e) {
            throw e;
        } catch (final Exception e) {
            throw wrap("authorize", e);
        }
    }

    @Override
    public PaymentServiceCaptureResponse capture(final UUID t, final PaymentServiceCaptureRequest r) throws PrismClientException {
        try {
            return bundle(t).payment.capture(r);
        } catch (final PrismClientException e) {
            throw e;
        } catch (final Exception e) {
            throw wrap("capture", e);
        }
    }

    @Override
    public PaymentServiceVoidResponse voidPayment(final UUID t, final PaymentServiceVoidRequest r) throws PrismClientException {
        try {
            return bundle(t).payment.void_(r);
        } catch (final PrismClientException e) {
            throw e;
        } catch (final Exception e) {
            throw wrap("void", e);
        }
    }

    @Override
    public PaymentServiceGetResponse get(final UUID t, final PaymentServiceGetRequest r) throws PrismClientException {
        try {
            return bundle(t).payment.get(r);
        } catch (final PrismClientException e) {
            throw e;
        } catch (final Exception e) {
            throw wrap("get", e);
        }
    }

    @Override
    public RefundResponse refund(final UUID t, final PaymentServiceRefundRequest r) throws PrismClientException {
        try {
            return bundle(t).payment.refund(r);
        } catch (final PrismClientException e) {
            throw e;
        } catch (final Exception e) {
            throw wrap("refund", e);
        }
    }

    @Override
    public RefundResponse refundGet(final UUID t, final RefundServiceGetRequest r) throws PrismClientException {
        try {
            return bundle(t).refund.refund_get(r);
        } catch (final PrismClientException e) {
            throw e;
        } catch (final Exception e) {
            throw wrap("refundGet", e);
        }
    }

    @Override
    public PaymentServiceSetupRecurringResponse setupRecurring(final UUID t, final PaymentServiceSetupRecurringRequest r) throws PrismClientException {
        try {
            return bundle(t).payment.setup_recurring(r);
        } catch (final PrismClientException e) {
            throw e;
        } catch (final Exception e) {
            throw wrap("setupRecurring", e);
        }
    }

    @Override
    public RecurringPaymentServiceChargeResponse charge(final UUID t, final RecurringPaymentServiceChargeRequest r) throws PrismClientException {
        try {
            return bundle(t).recurring.charge(r);
        } catch (final PrismClientException e) {
            throw e;
        } catch (final Exception e) {
            throw wrap("charge", e);
        }
    }

    @Override
    public PaymentServiceTokenAuthorizeResponse tokenAuthorize(final UUID t, final PaymentServiceTokenAuthorizeRequest r) throws PrismClientException {
        try {
            return bundle(t).payment.token_authorize(r);
        } catch (final PrismClientException e) {
            throw e;
        } catch (final Exception e) {
            throw wrap("tokenAuthorize", e);
        }
    }

    @Override
    public PaymentMethodServiceTokenizeResponse tokenize(final UUID t, final PaymentMethodServiceTokenizeRequest r) throws PrismClientException {
        try {
            return bundle(t).paymentMethod.tokenize(r);
        } catch (final PrismClientException e) {
            throw e;
        } catch (final Exception e) {
            throw wrap("tokenize", e);
        }
    }

    @Override
    public EventServiceHandleResponse handleEvent(final UUID t, final EventServiceHandleRequest r) throws PrismClientException {
        try {
            return bundle(t).event.handle_event(r);
        } catch (final PrismClientException e) {
            throw e;
        } catch (final Exception e) {
            throw wrap("handleEvent", e);
        }
    }

    private PrismClientException wrap(final String op, final Exception e) {
        // TODO(P0): distinguish the SDK's IntegrationError / ConnectorError (do NOT retry) from NetworkError
        // (safe to retry with backoff) — see the demo-integration skill's error-handling contract.
        logger.warn("Prism {} failed: {}", op, e.getMessage());
        return new PrismClientException("Prism " + op + " failed: " + e.getMessage(), "PRISM_ERROR", false, e);
    }

    /** The set of SDK clients for one tenant's connector config (clients hold OkHttp pools → reuse). */
    private static final class ClientBundle {
        final PaymentClient payment;
        final RefundClient refund;
        final RecurringPaymentClient recurring;
        final PaymentMethodClient paymentMethod;
        final EventClient event;
        final MerchantAuthenticationClient merchantAuth;

        ClientBundle(final ConnectorConfig config) {
            this.payment = new PaymentClient(config);
            this.refund = new RefundClient(config);
            this.recurring = new RecurringPaymentClient(config);
            this.paymentMethod = new PaymentMethodClient(config);
            this.event = new EventClient(config);
            this.merchantAuth = new MerchantAuthenticationClient(config);
        }
    }
}
