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
package org.killbill.billing.plugin.hyperswitch;

import java.math.BigDecimal;
import java.util.ArrayList;
import java.util.Collections;
import java.util.Iterator;
import java.util.List;
import java.util.Map;
import java.util.UUID;
import java.util.function.Supplier;

import org.killbill.billing.account.api.Account;
import org.killbill.billing.account.api.AccountApiException;
import org.killbill.billing.catalog.api.Currency;
import org.killbill.billing.osgi.libs.killbill.OSGIKillbillAPI;
import org.killbill.billing.payment.api.Payment;
import org.killbill.billing.payment.api.PaymentApiException;
import org.killbill.billing.payment.api.PaymentMethod;
import org.killbill.billing.payment.api.PaymentMethodPlugin;
import org.killbill.billing.payment.api.PaymentTransaction;
import org.killbill.billing.payment.api.PluginProperty;
import org.killbill.billing.payment.api.TransactionStatus;
import org.killbill.billing.payment.api.TransactionType;
import org.killbill.billing.payment.plugin.api.GatewayNotification;
import org.killbill.billing.payment.plugin.api.HostedPaymentPageFormDescriptor;
import org.killbill.billing.payment.plugin.api.PaymentMethodInfoPlugin;
import org.killbill.billing.payment.plugin.api.PaymentPluginApi;
import org.killbill.billing.payment.plugin.api.PaymentPluginApiException;
import org.killbill.billing.payment.plugin.api.PaymentPluginStatus;
import org.killbill.billing.payment.plugin.api.PaymentTransactionInfoPlugin;
import org.killbill.billing.plugin.api.PluginProperties;
import org.killbill.billing.plugin.api.payment.PluginHostedPaymentPageFormDescriptor;
import org.killbill.billing.plugin.api.payment.PluginPaymentMethodInfoPlugin;
import org.killbill.billing.plugin.hyperswitch.client.PrismClient;
import org.killbill.billing.plugin.hyperswitch.client.PrismClientException;
import org.killbill.billing.plugin.hyperswitch.core.AmountConverter;
import org.killbill.billing.plugin.hyperswitch.core.HyperswitchPluginProperties;
import org.killbill.billing.plugin.hyperswitch.core.PrismRequestBuilder;
import org.killbill.billing.plugin.hyperswitch.core.PrismStatusMapper;
import org.killbill.billing.plugin.hyperswitch.model.HyperswitchGatewayNotification;
import org.killbill.billing.plugin.hyperswitch.model.HyperswitchPaymentMethodPlugin;
import org.killbill.billing.plugin.hyperswitch.model.HyperswitchPaymentTransactionInfoPlugin;
import org.killbill.billing.plugin.hyperswitch.store.HyperswitchStateStore;
import org.killbill.billing.plugin.hyperswitch.store.HyperswitchStateStore.AuthInfo;
import org.killbill.billing.plugin.hyperswitch.store.HyperswitchStateStore.StoredCredential;
import org.killbill.billing.plugin.hyperswitch.webhook.HyperswitchWebhookHandler;
import org.killbill.billing.util.callcontext.CallContext;
import org.killbill.billing.util.callcontext.TenantContext;
import org.killbill.billing.util.entity.Pagination;
import org.killbill.clock.Clock;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import com.google.common.collect.ImmutableList;

// NOTE (P0 verification): SDK getters/request signatures preserved from the E2E-verified DB-based version.
import types.Payment.CustomerServiceCreateResponse;
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
import types.Payment.PaymentServiceVoidRequest;
import types.Payment.PaymentServiceVoidResponse;
import types.Payment.RecurringPaymentServiceChargeRequest;
import types.Payment.RecurringPaymentServiceChargeResponse;
import types.Payment.RefundResponse;
import types.Payment.RefundServiceGetRequest;

/**
 * KillBill payment plugin backed by Hyperswitch Prism — DB-less variant. Implements {@link PaymentPluginApi}
 * directly (no DAO/jOOQ) and persists connector state in KillBill's own storage via {@link HyperswitchStateStore}
 * (custom fields + the core Payment API). All connector flow logic (customer leg, mandate reference extraction,
 * metadata/bill-to, MIT charge) is preserved from the DB-based version; only the persistence layer changed.
 */
public class HyperswitchPaymentPluginApi implements PaymentPluginApi {

    private static final Logger logger = LoggerFactory.getLogger(HyperswitchPaymentPluginApi.class);
    private static final Iterable<PluginProperty> NO_PROPS = ImmutableList.of();

    private final HyperswitchConfigPropertiesConfigurationHandler configHandler;
    private final PrismClient prismClient;
    private final HyperswitchStateStore store;
    private final OSGIKillbillAPI killbillAPI;
    private final Clock clock;
    private final HyperswitchWebhookHandler webhookHandler;

    public HyperswitchPaymentPluginApi(final HyperswitchConfigPropertiesConfigurationHandler configHandler,
                                       final PrismClient prismClient,
                                       final HyperswitchStateStore store,
                                       final OSGIKillbillAPI killbillAPI,
                                       final Clock clock) {
        this.configHandler = configHandler;
        this.prismClient = prismClient;
        this.store = store;
        this.killbillAPI = killbillAPI;
        this.clock = clock;
        this.webhookHandler = new HyperswitchWebhookHandler(configHandler, prismClient, store, killbillAPI, clock);
    }

    // ------------------------------------------------------------------ payment flows

    @Override
    public PaymentTransactionInfoPlugin authorizePayment(final UUID kbAccountId, final UUID kbPaymentId, final UUID kbTransactionId,
                                                         final UUID kbPaymentMethodId, final BigDecimal amount, final Currency currency,
                                                         final Iterable<PluginProperty> properties, final CallContext context) throws PaymentPluginApiException {
        return pay(kbPaymentId, kbTransactionId, kbPaymentMethodId, amount, currency, properties, context, TransactionType.AUTHORIZE, "MANUAL");
    }

    @Override
    public PaymentTransactionInfoPlugin purchasePayment(final UUID kbAccountId, final UUID kbPaymentId, final UUID kbTransactionId,
                                                        final UUID kbPaymentMethodId, final BigDecimal amount, final Currency currency,
                                                        final Iterable<PluginProperty> properties, final CallContext context) throws PaymentPluginApiException {
        final HyperswitchConfigProperties config = configHandler.getConfigurable(context.getTenantId());
        final String captureMethod = "MANUAL".equalsIgnoreCase(config.getCaptureMethod()) ? "MANUAL" : "AUTOMATIC";
        return pay(kbPaymentId, kbTransactionId, kbPaymentMethodId, amount, currency, properties, context, TransactionType.PURCHASE, captureMethod);
    }

    private PaymentTransactionInfoPlugin pay(final UUID kbPaymentId, final UUID kbTransactionId, final UUID kbPaymentMethodId,
                                             final BigDecimal amount, final Currency currency, final Iterable<PluginProperty> properties,
                                             final CallContext context, final TransactionType txnType, final String captureMethod) throws PaymentPluginApiException {
        final UUID tenantId = context.getTenantId();
        final HyperswitchConfigProperties config = configHandler.getConfigurable(tenantId);
        final long minor = AmountConverter.toMinor(amount, currency);
        final String cc = currency.name();
        final HyperswitchPluginProperties.Card card = HyperswitchPluginProperties.extractCard(properties);

        final String statusName;
        final String connectorTxnId;
        final String gatewayError;
        try {
            if (card != null) {
                final PaymentServiceAuthorizeRequest req = PrismRequestBuilder.authorize(kbTransactionId.toString(), minor, cc, card, captureMethod, false, config.getReturnUrl());
                final PaymentServiceAuthorizeResponse resp = prismClient.authorize(tenantId, req);
                statusName = safe(() -> resp.getStatus().name());
                connectorTxnId = safe(() -> resp.getConnectorTransactionId());
                gatewayError = safe(() -> resp.getError().getUnifiedDetails().getMessage());
            } else {
                final StoredCredential cred = store.getStoredCredential(kbPaymentMethodId, context);
                if (cred.mandateId == null && cred.token == null) {
                    return fail(kbPaymentId, kbTransactionId, txnType, amount, currency, PaymentPluginStatus.CANCELED,
                                "No stored mandate/token for payment method " + kbPaymentMethodId, "NO_REUSABLE_CREDENTIAL");
                }
                final boolean autoCapture = "AUTOMATIC".equalsIgnoreCase(captureMethod);
                if (cred.token != null && (!autoCapture || cred.mandateId == null)) {
                    // token_authorize honours the capture method — required for AUTHORIZE (auth-only, MANUAL).
                    final PaymentServiceTokenAuthorizeRequest req = PrismRequestBuilder.tokenAuthorize(kbTransactionId.toString(), minor, cc, cred.token, captureMethod, config.getReturnUrl());
                    final PaymentServiceAuthorizeResponse resp = prismClient.tokenAuthorize(tenantId, req);
                    statusName = safe(() -> resp.getStatus().name());
                    connectorTxnId = safe(() -> resp.getConnectorTransactionId());
                    gatewayError = safe(() -> resp.getError().getUnifiedDetails().getMessage());
                } else {
                    // MIT charge against the stored mandate (captures immediately) — passes the connector customer id.
                    final RecurringPaymentServiceChargeRequest req = PrismRequestBuilder.charge(kbTransactionId.toString(), cred.mandateId, minor, cc, cred.customerId, connectorMetadata(config), config.getReturnUrl());
                    final RecurringPaymentServiceChargeResponse resp = prismClient.charge(tenantId, req);
                    statusName = safe(() -> resp.getStatus().name());
                    connectorTxnId = safe(() -> resp.getConnectorTransactionId());
                    gatewayError = safe(() -> resp.getError().getUnifiedDetails().getMessage());
                }
            }
        } catch (final PrismClientException e) {
            return recordException(kbPaymentId, kbTransactionId, txnType, amount, currency, e);
        }
        return record(kbPaymentId, kbTransactionId, txnType, amount, currency, connectorTxnId, statusName, gatewayError, context);
    }

    @Override
    public PaymentTransactionInfoPlugin capturePayment(final UUID kbAccountId, final UUID kbPaymentId, final UUID kbTransactionId,
                                                       final UUID kbPaymentMethodId, final BigDecimal amount, final Currency currency,
                                                       final Iterable<PluginProperty> properties, final CallContext context) throws PaymentPluginApiException {
        final AuthInfo auth = store.findAuthorization(kbPaymentId, context);
        if (auth == null) {
            return fail(kbPaymentId, kbTransactionId, TransactionType.CAPTURE, amount, currency, PaymentPluginStatus.CANCELED, "No prior authorization to capture", "NO_AUTHORIZATION");
        }
        try {
            final PaymentServiceCaptureRequest req = PrismRequestBuilder.capture(kbTransactionId.toString(), auth.connectorTransactionId, AmountConverter.toMinor(amount, currency), currency.name());
            final PaymentServiceCaptureResponse resp = prismClient.capture(context.getTenantId(), req);
            return record(kbPaymentId, kbTransactionId, TransactionType.CAPTURE, amount, currency, auth.connectorTransactionId,
                          safe(() -> resp.getStatus().name()), safe(() -> resp.getError().getUnifiedDetails().getMessage()), context);
        } catch (final PrismClientException e) {
            return recordException(kbPaymentId, kbTransactionId, TransactionType.CAPTURE, amount, currency, e);
        }
    }

    @Override
    public PaymentTransactionInfoPlugin voidPayment(final UUID kbAccountId, final UUID kbPaymentId, final UUID kbTransactionId,
                                                    final UUID kbPaymentMethodId, final Iterable<PluginProperty> properties,
                                                    final CallContext context) throws PaymentPluginApiException {
        final AuthInfo auth = store.findAuthorization(kbPaymentId, context);
        if (auth == null) {
            return fail(kbPaymentId, kbTransactionId, TransactionType.VOID, null, null, PaymentPluginStatus.CANCELED, "No prior authorization to void", "NO_AUTHORIZATION");
        }
        try {
            final PaymentServiceVoidRequest req = PrismRequestBuilder.voidPayment(kbTransactionId.toString(), auth.connectorTransactionId);
            final PaymentServiceVoidResponse resp = prismClient.voidPayment(context.getTenantId(), req);
            return record(kbPaymentId, kbTransactionId, TransactionType.VOID, null, null, auth.connectorTransactionId,
                          safe(() -> resp.getStatus().name()), safe(() -> resp.getError().getUnifiedDetails().getMessage()), context);
        } catch (final PrismClientException e) {
            return recordException(kbPaymentId, kbTransactionId, TransactionType.VOID, null, null, e);
        }
    }

    @Override
    public PaymentTransactionInfoPlugin refundPayment(final UUID kbAccountId, final UUID kbPaymentId, final UUID kbTransactionId,
                                                      final UUID kbPaymentMethodId, final BigDecimal amount, final Currency currency,
                                                      final Iterable<PluginProperty> properties, final CallContext context) throws PaymentPluginApiException {
        final AuthInfo auth = store.findAuthorization(kbPaymentId, context);
        if (auth == null) {
            return fail(kbPaymentId, kbTransactionId, TransactionType.REFUND, amount, currency, PaymentPluginStatus.CANCELED, "No captured payment to refund", "NO_CAPTURE");
        }
        final long paymentMinor = auth.amount != null ? AmountConverter.toMinor(auth.amount, currency) : AmountConverter.toMinor(amount, currency);
        try {
            // No refund reason: the field is optional and reason vocabularies are connector-specific (e.g. Adyen).
            final PaymentServiceRefundRequest req = PrismRequestBuilder.refund(kbTransactionId.toString(), auth.connectorTransactionId, paymentMinor, AmountConverter.toMinor(amount, currency), currency.name(), null);
            final RefundResponse resp = prismClient.refund(context.getTenantId(), req);
            return record(kbPaymentId, kbTransactionId, TransactionType.REFUND, amount, currency, auth.connectorTransactionId,
                          safe(() -> resp.getStatus().name()), safe(() -> resp.getError().getUnifiedDetails().getMessage()), context);
        } catch (final PrismClientException e) {
            return recordException(kbPaymentId, kbTransactionId, TransactionType.REFUND, amount, currency, e);
        }
    }

    @Override
    public PaymentTransactionInfoPlugin creditPayment(final UUID kbAccountId, final UUID kbPaymentId, final UUID kbTransactionId,
                                                      final UUID kbPaymentMethodId, final BigDecimal amount, final Currency currency,
                                                      final Iterable<PluginProperty> properties, final CallContext context) throws PaymentPluginApiException {
        throw new PaymentPluginApiException("UNSUPPORTED", "creditPayment is not supported by the Hyperswitch Prism plugin");
    }

    // ------------------------------------------------------------------ reads (rebuilt from core + custom fields)

    @Override
    public List<PaymentTransactionInfoPlugin> getPaymentInfo(final UUID kbAccountId, final UUID kbPaymentId,
                                                             final Iterable<PluginProperty> properties, final TenantContext context) throws PaymentPluginApiException {
        final List<PaymentTransactionInfoPlugin> result = new ArrayList<>();
        try {
            final Payment payment = killbillAPI.getPaymentApi().getPayment(kbPaymentId, false, false, NO_PROPS, context);
            final UUID tenantId = context.getTenantId();
            for (final PaymentTransaction t : payment.getTransactions()) {
                final String connectorTxnId = store.getConnectorTransactionId(t.getId(), context);
                final TransactionType txnType = t.getTransactionType();
                PaymentPluginStatus status = fromCoreStatus(t.getTransactionStatus());
                if (status == PaymentPluginStatus.PENDING && connectorTxnId != null) {
                    final PaymentPluginStatus refreshed = trySync(tenantId, connectorTxnId, t, txnType);
                    if (refreshed != null) {
                        status = refreshed;
                    }
                }
                result.add(HyperswitchPaymentTransactionInfoPlugin.build(kbPaymentId, t.getId(), txnType, t.getAmount(), t.getCurrency(),
                                                                         status, connectorTxnId, t.getGatewayErrorMsg(), t.getGatewayErrorCode(),
                                                                         t.getEffectiveDate() != null ? t.getEffectiveDate() : clock.getUTCNow()));
            }
        } catch (final PaymentApiException e) {
            throw new PaymentPluginApiException("Unable to load payment " + kbPaymentId, e);
        }
        return result;
    }

    private PaymentPluginStatus trySync(final UUID tenantId, final String connectorTxnId, final PaymentTransaction t, final TransactionType txnType) {
        try {
            if (txnType == TransactionType.REFUND || txnType == TransactionType.CREDIT) {
                final RefundServiceGetRequest req = PrismRequestBuilder.refundGet(t.getId().toString(), connectorTxnId);
                final RefundResponse resp = prismClient.refundGet(tenantId, req);
                return PrismStatusMapper.map(safe(() -> resp.getStatus().name()), TransactionType.REFUND);
            }
            if (t.getCurrency() == null) {
                return null;
            }
            final long minor = t.getAmount() != null ? AmountConverter.toMinor(t.getAmount(), t.getCurrency()) : 0L;
            final PaymentServiceGetRequest req = PrismRequestBuilder.get(t.getId().toString(), connectorTxnId, minor, t.getCurrency().name());
            final PaymentServiceGetResponse resp = prismClient.get(tenantId, req);
            return PrismStatusMapper.map(safe(() -> resp.getStatus().name()), txnType);
        } catch (final Exception e) {
            logger.warn("Sync for transaction {} failed: {}", t.getId(), e.getMessage());
            return null;
        }
    }

    @Override
    public Pagination<PaymentTransactionInfoPlugin> searchPayments(final String searchKey, final Long offset, final Long limit,
                                                                   final Iterable<PluginProperty> properties, final TenantContext context) throws PaymentPluginApiException {
        logger.info("searchPayments is not supported by the DB-less Hyperswitch plugin (searchKey={})", searchKey);
        return emptyPagination(offset, limit);
    }

    // ------------------------------------------------------------------ payment methods (custom-field backed)

    @Override
    public void addPaymentMethod(final UUID kbAccountId, final UUID kbPaymentMethodId, final PaymentMethodPlugin paymentMethodProps,
                                 final boolean setDefault, final Iterable<PluginProperty> properties, final CallContext context) throws PaymentPluginApiException {
        final UUID tenantId = context.getTenantId();
        final HyperswitchConfigProperties config = configHandler.getConfigurable(tenantId);
        final Iterable<PluginProperty> allProps = PluginProperties.merge(paymentMethodProps.getProperties(), properties);
        final HyperswitchPluginProperties.Card card = HyperswitchPluginProperties.extractCard(allProps);

        String token = paymentMethodProps.getExternalPaymentMethodId();
        String mandateId = null;
        String connectorCustomerId = null;

        if (card != null) {
            try {
                final String cc = accountCurrency(kbAccountId, context);
                // Create the connector-side customer first: Stripe (and others) only allow reusing a stored
                // payment method off-session when it is attached to a customer.
                HyperswitchPluginProperties.Billing billing = null;
                try {
                    final Account account = killbillAPI.getAccountUserApi().getAccountById(kbAccountId, context);
                    billing = toBilling(account);
                    final CustomerServiceCreateResponse custResp = prismClient.customerCreate(tenantId,
                            PrismRequestBuilder.customerCreate(kbAccountId.toString(), account.getName(), account.getEmail()));
                    connectorCustomerId = safe(custResp::getConnectorCustomerId);
                } catch (final AccountApiException | PrismClientException e) {
                    logger.warn("Connector customer creation failed (continuing without): {}", e.getMessage());
                }
                if (connectorCustomerId == null && "adyen".equalsIgnoreCase(config.getConnector())) {
                    // Adyen has no customer-create API; Prism derives the shopperReference at mandate setup as
                    // "{merchantId}_{customer.id}". The MIT charge must present the same reference.
                    connectorCustomerId = "DefaultMerchantId_" + kbAccountId;
                }
                final PaymentServiceSetupRecurringRequest req = PrismRequestBuilder.setupRecurring(kbPaymentMethodId.toString(), cc, card, billing, kbAccountId.toString(), connectorCustomerId, connectorMetadata(config), config.getReturnUrl());
                final PaymentServiceSetupRecurringResponse resp = prismClient.setupRecurring(tenantId, req);
                // Prefer mandate_reference_details.connector_mandate_id — the connector's REUSABLE reference
                // (Stripe: the pm_… payment method). connector_recurring_payment_id is the seti_… registration id.
                mandateId = safe(() -> {
                    if (resp.hasMandateReferenceDetails() && resp.getMandateReferenceDetails().hasConnectorMandateId()) {
                        return resp.getMandateReferenceDetails().getConnectorMandateId();
                    }
                    return resp.hasConnectorRecurringPaymentId() ? resp.getConnectorRecurringPaymentId() : null;
                });
                if (mandateId != null && token == null) {
                    token = mandateId;
                }
            } catch (final PrismClientException e) {
                throw new PaymentPluginApiException("Unable to set up mandate for payment method", e);
            }
        }

        store.savePaymentMethodCredential(kbPaymentMethodId, mandateId, token, connectorCustomerId, context);
    }

    @Override
    public void deletePaymentMethod(final UUID kbAccountId, final UUID kbPaymentMethodId, final Iterable<PluginProperty> properties,
                                    final CallContext context) throws PaymentPluginApiException {
        store.deletePaymentMethodCredential(kbPaymentMethodId, context);
    }

    @Override
    public PaymentMethodPlugin getPaymentMethodDetail(final UUID kbAccountId, final UUID kbPaymentMethodId,
                                                      final Iterable<PluginProperty> properties, final TenantContext context) throws PaymentPluginApiException {
        final StoredCredential cred = store.getStoredCredential(kbPaymentMethodId, context);
        final List<PluginProperty> props = new ArrayList<>();
        if (cred.mandateId != null) {
            props.add(new PluginProperty(HyperswitchStateStore.CF_MANDATE_ID, cred.mandateId, false));
        }
        if (cred.customerId != null) {
            props.add(new PluginProperty(HyperswitchStateStore.CF_CUSTOMER_ID, cred.customerId, false));
        }
        return new HyperswitchPaymentMethodPlugin(kbPaymentMethodId, cred.token, isDefault(kbAccountId, kbPaymentMethodId, context), props);
    }

    @Override
    public void setDefaultPaymentMethod(final UUID kbAccountId, final UUID kbPaymentMethodId, final Iterable<PluginProperty> properties,
                                        final CallContext context) throws PaymentPluginApiException {
        // KillBill core manages the default payment method; the plugin stores nothing about it.
    }

    @Override
    public List<PaymentMethodInfoPlugin> getPaymentMethods(final UUID kbAccountId, final boolean refreshFromGateway,
                                                           final Iterable<PluginProperty> properties, final CallContext context) throws PaymentPluginApiException {
        final List<PaymentMethodInfoPlugin> result = new ArrayList<>();
        try {
            final UUID defaultPm = defaultPaymentMethodId(kbAccountId, context);
            for (final PaymentMethod pm : killbillAPI.getPaymentApi().getAccountPaymentMethods(kbAccountId, false, false, NO_PROPS, context)) {
                if (!HyperswitchActivator.PLUGIN_NAME.equals(pm.getPluginName())) {
                    continue;
                }
                result.add(new PluginPaymentMethodInfoPlugin(kbAccountId, pm.getId(), pm.getId().equals(defaultPm), pm.getExternalKey()));
            }
        } catch (final PaymentApiException e) {
            throw new PaymentPluginApiException("Unable to list payment methods for account " + kbAccountId, e);
        }
        return result;
    }

    @Override
    public Pagination<PaymentMethodPlugin> searchPaymentMethods(final String searchKey, final Long offset, final Long limit,
                                                                final Iterable<PluginProperty> properties, final TenantContext context) throws PaymentPluginApiException {
        logger.info("searchPaymentMethods is not supported by the DB-less Hyperswitch plugin (searchKey={})", searchKey);
        return emptyPagination(offset, limit);
    }

    @Override
    public void resetPaymentMethods(final UUID kbAccountId, final List<PaymentMethodInfoPlugin> paymentMethods,
                                    final Iterable<PluginProperty> properties, final CallContext context) throws PaymentPluginApiException {
        // No plugin-side payment-method table to rewrite on account migration; nothing to do.
    }

    // ------------------------------------------------------------------ webhooks + HPP

    @Override
    public GatewayNotification processNotification(final String notification, final Iterable<PluginProperty> properties,
                                                   final CallContext context) throws PaymentPluginApiException {
        return webhookHandler.processNotification(notification, context);
    }

    /** Servlet entrypoint — full HTTP context. */
    public HyperswitchGatewayNotification handleWebhook(final UUID tenantId, final String uri, final Map<String, String> headers,
                                                        final byte[] body, final CallContext context) {
        return webhookHandler.handle(tenantId, uri, headers, body, context);
    }

    @Override
    public HostedPaymentPageFormDescriptor buildFormDescriptor(final UUID kbAccountId, final Iterable<PluginProperty> customFields,
                                                               final Iterable<PluginProperty> properties, final CallContext context) throws PaymentPluginApiException {
        return new PluginHostedPaymentPageFormDescriptor(kbAccountId, "");
    }

    // ------------------------------------------------------------------ helpers

    private PaymentTransactionInfoPlugin record(final UUID kbPaymentId, final UUID kbTransactionId, final TransactionType txnType,
                                                final BigDecimal amount, final Currency currency, final String connectorTxnId,
                                                final String statusName, final String gatewayError, final CallContext context) {
        store.saveConnectorTransactionId(kbTransactionId, connectorTxnId, statusName, context);
        final PaymentPluginStatus status = PrismStatusMapper.map(statusName, txnType);
        return HyperswitchPaymentTransactionInfoPlugin.build(kbPaymentId, kbTransactionId, txnType, amount, currency, status, connectorTxnId, gatewayError, null, clock.getUTCNow());
    }

    private PaymentTransactionInfoPlugin recordException(final UUID kbPaymentId, final UUID kbTransactionId, final TransactionType txnType,
                                                         final BigDecimal amount, final Currency currency, final PrismClientException e) {
        final PaymentPluginStatus status = e.isRetryable() ? PaymentPluginStatus.ERROR : PaymentPluginStatus.CANCELED;
        return fail(kbPaymentId, kbTransactionId, txnType, amount, currency, status, e.getMessage(), e.getErrorCode());
    }

    private PaymentTransactionInfoPlugin fail(final UUID kbPaymentId, final UUID kbTransactionId, final TransactionType txnType,
                                              final BigDecimal amount, final Currency currency, final PaymentPluginStatus status,
                                              final String message, final String code) {
        return HyperswitchPaymentTransactionInfoPlugin.build(kbPaymentId, kbTransactionId, txnType, amount, currency, status, null, message, code, clock.getUTCNow());
    }

    /** Connector-specific request metadata JSON (config key {@code <connector>.metadata}); defaults to "{}". */
    private static String connectorMetadata(final HyperswitchConfigProperties config) {
        final String configured = config.getConnectorProperty("metadata");
        return configured != null ? configured : "{}";
    }

    /** Map KillBill account fields to a connector bill-to (name split on the first space, as is conventional). */
    private static HyperswitchPluginProperties.Billing toBilling(final Account account) {
        final String name = account.getName();
        String firstName = name;
        String lastName = null;
        if (name != null && name.contains(" ")) {
            firstName = name.substring(0, name.indexOf(' '));
            lastName = name.substring(name.indexOf(' ') + 1);
        }
        return new HyperswitchPluginProperties.Billing(firstName, lastName, account.getEmail(),
                                                       account.getAddress1(), account.getCity(),
                                                       account.getStateOrProvince(), account.getPostalCode(),
                                                       account.getCountry());
    }

    private String accountCurrency(final UUID kbAccountId, final CallContext context) throws PaymentPluginApiException {
        try {
            final Account account = killbillAPI.getAccountUserApi().getAccountById(kbAccountId, context);
            return account.getCurrency().name();
        } catch (final AccountApiException e) {
            throw new PaymentPluginApiException("Unable to resolve account currency for mandate setup", e);
        }
    }

    private boolean isDefault(final UUID kbAccountId, final UUID kbPaymentMethodId, final TenantContext context) {
        return kbPaymentMethodId.equals(defaultPaymentMethodId(kbAccountId, context));
    }

    private UUID defaultPaymentMethodId(final UUID kbAccountId, final TenantContext context) {
        try {
            return killbillAPI.getAccountUserApi().getAccountById(kbAccountId, context).getPaymentMethodId();
        } catch (final AccountApiException e) {
            return null;
        }
    }

    private static PaymentPluginStatus fromCoreStatus(final TransactionStatus status) {
        if (status == null) {
            return PaymentPluginStatus.UNDEFINED;
        }
        switch (status) {
            case SUCCESS:
                return PaymentPluginStatus.PROCESSED;
            case PENDING:
                return PaymentPluginStatus.PENDING;
            case PAYMENT_FAILURE:
            case PLUGIN_FAILURE:
                return PaymentPluginStatus.ERROR;
            default:
                return PaymentPluginStatus.UNDEFINED;
        }
    }

    private static String safe(final Supplier<String> supplier) {
        try {
            final String value = supplier.get();
            return value == null || value.isEmpty() ? null : value;
        } catch (final RuntimeException e) {
            return null;
        }
    }

    private static <T> Pagination<T> emptyPagination(final Long offset, final Long limit) {
        return new Pagination<T>() {
            @Override
            public Long getCurrentOffset() {
                return offset == null ? 0L : offset;
            }

            @Override
            public Long getNextOffset() {
                return null;
            }

            @Override
            public Long getMaxNbRecords() {
                return 0L;
            }

            @Override
            public Long getTotalNbRecords() {
                return 0L;
            }

            @Override
            public Iterator<T> iterator() {
                return Collections.<T>emptyIterator();
            }
        };
    }
}
