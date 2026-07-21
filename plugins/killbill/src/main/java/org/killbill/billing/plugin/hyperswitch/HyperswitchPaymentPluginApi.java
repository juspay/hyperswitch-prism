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
import java.sql.SQLException;
import java.util.HashMap;
import java.util.List;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.UUID;
import java.util.function.Supplier;

import org.killbill.billing.account.api.Account;
import org.killbill.billing.account.api.AccountApiException;
import org.killbill.billing.catalog.api.Currency;
import org.killbill.billing.osgi.libs.killbill.OSGIConfigPropertiesService;
import org.killbill.billing.osgi.libs.killbill.OSGIKillbillAPI;
import org.killbill.billing.payment.api.PluginProperty;
import org.killbill.billing.payment.api.TransactionType;
import org.killbill.billing.payment.plugin.api.GatewayNotification;
import org.killbill.billing.payment.plugin.api.HostedPaymentPageFormDescriptor;
import org.killbill.billing.payment.plugin.api.PaymentMethodInfoPlugin;
import org.killbill.billing.payment.plugin.api.PaymentMethodPlugin;
import org.killbill.billing.payment.plugin.api.PaymentPluginApiException;
import org.killbill.billing.payment.plugin.api.PaymentPluginStatus;
import org.killbill.billing.payment.plugin.api.PaymentTransactionInfoPlugin;
import org.killbill.billing.plugin.api.PluginProperties;
import org.killbill.billing.plugin.api.payment.PluginHostedPaymentPageFormDescriptor;
import org.killbill.billing.plugin.api.payment.PluginPaymentMethodInfoPlugin;
import org.killbill.billing.plugin.api.payment.PluginPaymentPluginApi;
import org.killbill.billing.plugin.hyperswitch.client.PrismClient;
import org.killbill.billing.plugin.hyperswitch.client.PrismClientException;
import org.killbill.billing.plugin.hyperswitch.core.AmountConverter;
import org.killbill.billing.plugin.hyperswitch.core.HyperswitchPluginProperties;
import org.killbill.billing.plugin.hyperswitch.core.PrismRequestBuilder;
import org.killbill.billing.plugin.hyperswitch.core.PrismStatusMapper;
import org.killbill.billing.plugin.hyperswitch.dao.HyperswitchDao;
import org.killbill.billing.plugin.hyperswitch.dao.gen.tables.HyperswitchPaymentMethods;
import org.killbill.billing.plugin.hyperswitch.dao.gen.tables.HyperswitchResponses;
import org.killbill.billing.plugin.hyperswitch.dao.gen.tables.records.HyperswitchPaymentMethodsRecord;
import org.killbill.billing.plugin.hyperswitch.dao.gen.tables.records.HyperswitchResponsesRecord;
import org.killbill.billing.plugin.hyperswitch.model.HyperswitchGatewayNotification;
import org.killbill.billing.plugin.hyperswitch.model.HyperswitchPaymentMethodPlugin;
import org.killbill.billing.plugin.hyperswitch.model.HyperswitchPaymentTransactionInfoPlugin;
import org.killbill.billing.plugin.hyperswitch.webhook.HyperswitchWebhookHandler;
import org.killbill.billing.util.callcontext.CallContext;
import org.killbill.clock.Clock;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

// NOTE (P0 verification): SDK response getters (getStatus().name(), getConnectorTransactionId(),
// getError().getUnifiedDetails().getMessage(), getConnectorRecurringPaymentId(), getPaymentMethodToken())
// follow examples/stripe/stripe.kt. Verify against io.hyperswitch:prism:0.0.6 at first compile.
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
import types.Payment.RecurringPaymentServiceChargeRequest;
import types.Payment.RecurringPaymentServiceChargeResponse;
import types.Payment.RefundResponse;
import types.Payment.RefundServiceGetRequest;

/**
 * KillBill payment plugin backed by Hyperswitch Prism (embedded SDK, in-process — same model as Medusa).
 * Thin host adapter: the framework base handles DAO-backed reads; the flow methods build Prism requests
 * (see {@link PrismRequestBuilder}), call the embedded SDK via {@link PrismClient}, map the returned status
 * ({@link PrismStatusMapper}) and persist to {@link HyperswitchDao}.
 */
public class HyperswitchPaymentPluginApi extends PluginPaymentPluginApi<HyperswitchResponsesRecord, HyperswitchResponses, HyperswitchPaymentMethodsRecord, HyperswitchPaymentMethods> {

    private static final Logger logger = LoggerFactory.getLogger(HyperswitchPaymentPluginApi.class);

    private final HyperswitchConfigPropertiesConfigurationHandler configHandler;
    private final PrismClient prismClient;
    private final HyperswitchDao dao;
    private final HyperswitchWebhookHandler webhookHandler;

    public HyperswitchPaymentPluginApi(final HyperswitchConfigPropertiesConfigurationHandler configHandler,
                                       final PrismClient prismClient,
                                       final OSGIKillbillAPI killbillAPI,
                                       final OSGIConfigPropertiesService configProperties,
                                       final Clock clock,
                                       final HyperswitchDao dao) {
        super(killbillAPI, configProperties, clock, dao);
        this.configHandler = configHandler;
        this.prismClient = prismClient;
        this.dao = dao;
        this.webhookHandler = new HyperswitchWebhookHandler(configHandler, prismClient, dao, killbillAPI, clock);
    }

    // ------------------------------------------------------------------ record → model builders

    @Override
    protected PaymentTransactionInfoPlugin buildPaymentTransactionInfoPlugin(final HyperswitchResponsesRecord record) {
        return HyperswitchPaymentTransactionInfoPlugin.build(record);
    }

    @Override
    protected PaymentMethodPlugin buildPaymentMethodPlugin(final HyperswitchPaymentMethodsRecord record) {
        return HyperswitchPaymentMethodPlugin.build(record);
    }

    @Override
    protected PaymentMethodInfoPlugin buildPaymentMethodInfoPlugin(final HyperswitchPaymentMethodsRecord record) {
        return new PluginPaymentMethodInfoPlugin(UUID.fromString(record.getKbAccountId()),
                                                 UUID.fromString(record.getKbPaymentMethodId()),
                                                 record.getIsDefault() != null && record.getIsDefault() == HyperswitchDao.DB_TRUE,
                                                 record.getHyperswitchId());
    }

    @Override
    protected String getPaymentMethodId(final HyperswitchPaymentMethodsRecord record) {
        return record.getKbPaymentMethodId();
    }

    // ------------------------------------------------------------------ payment flows (P1 / P4)

    @Override
    public PaymentTransactionInfoPlugin authorizePayment(final UUID kbAccountId, final UUID kbPaymentId, final UUID kbTransactionId,
                                                         final UUID kbPaymentMethodId, final BigDecimal amount, final Currency currency,
                                                         final Iterable<PluginProperty> properties, final CallContext context) throws PaymentPluginApiException {
        return pay(kbAccountId, kbPaymentId, kbTransactionId, kbPaymentMethodId, amount, currency, properties, context, TransactionType.AUTHORIZE, "MANUAL");
    }

    @Override
    public PaymentTransactionInfoPlugin purchasePayment(final UUID kbAccountId, final UUID kbPaymentId, final UUID kbTransactionId,
                                                        final UUID kbPaymentMethodId, final BigDecimal amount, final Currency currency,
                                                        final Iterable<PluginProperty> properties, final CallContext context) throws PaymentPluginApiException {
        final HyperswitchConfigProperties config = configHandler.getConfigurable(context.getTenantId());
        final String captureMethod = "MANUAL".equalsIgnoreCase(config.getCaptureMethod()) ? "MANUAL" : "AUTOMATIC";
        return pay(kbAccountId, kbPaymentId, kbTransactionId, kbPaymentMethodId, amount, currency, properties, context, TransactionType.PURCHASE, captureMethod);
    }

    /**
     * Card-present → {@code authorize}. Card-not-present (recurring invoice, customer not present) → a
     * merchant-initiated {@code charge} against the stored mandate, else {@code token_authorize} against the
     * stored token (Phase B of the recurring flow).
     */
    private PaymentTransactionInfoPlugin pay(final UUID kbAccountId, final UUID kbPaymentId, final UUID kbTransactionId,
                                             final UUID kbPaymentMethodId, final BigDecimal amount, final Currency currency,
                                             final Iterable<PluginProperty> properties, final CallContext context,
                                             final TransactionType txnType, final String captureMethod) throws PaymentPluginApiException {
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
                final StoredCredential cred = loadStoredCredential(kbPaymentMethodId, tenantId);
                if (cred == null || (cred.mandateId == null && cred.token == null)) {
                    return persistFailure(kbAccountId, kbPaymentId, kbTransactionId, txnType, amount, currency,
                                          PaymentPluginStatus.CANCELED, "No stored mandate/token for payment method " + kbPaymentMethodId,
                                          "NO_REUSABLE_CREDENTIAL", tenantId);
                }
                final boolean autoCapture = "AUTOMATIC".equalsIgnoreCase(captureMethod);
                if (cred.token != null && (!autoCapture || cred.mandateId == null)) {
                    // token_authorize honours the capture method — required for AUTHORIZE (auth-only, MANUAL).
                    final PaymentServiceTokenAuthorizeRequest req = PrismRequestBuilder.tokenAuthorize(kbTransactionId.toString(), minor, cc, cred.token, captureMethod, config.getReturnUrl());
                    final PaymentServiceTokenAuthorizeResponse resp = prismClient.tokenAuthorize(tenantId, req);
                    statusName = safe(() -> resp.getStatus().name());
                    connectorTxnId = safe(() -> resp.getConnectorTransactionId());
                    gatewayError = safe(() -> resp.getError().getUnifiedDetails().getMessage());
                } else {
                    // MIT charge against the stored mandate. NB: charge captures immediately, so this is the
                    // path for AUTOMATIC/PURCHASE (and, unavoidably, for a MANUAL auth backed only by a mandate).
                    final RecurringPaymentServiceChargeRequest req = PrismRequestBuilder.charge(cred.mandateId, minor, cc, cred.token, config.getReturnUrl());
                    final RecurringPaymentServiceChargeResponse resp = prismClient.charge(tenantId, req);
                    statusName = safe(() -> resp.getStatus().name());
                    connectorTxnId = safe(() -> resp.getConnectorTransactionId());
                    gatewayError = safe(() -> resp.getError().getUnifiedDetails().getMessage());
                }
            }
        } catch (final PrismClientException e) {
            return recordException(kbAccountId, kbPaymentId, kbTransactionId, txnType, amount, currency, e, tenantId);
        }
        return record(kbAccountId, kbPaymentId, kbTransactionId, txnType, amount, currency, connectorTxnId, statusName, gatewayError, tenantId);
    }

    @Override
    public PaymentTransactionInfoPlugin capturePayment(final UUID kbAccountId, final UUID kbPaymentId, final UUID kbTransactionId,
                                                       final UUID kbPaymentMethodId, final BigDecimal amount, final Currency currency,
                                                       final Iterable<PluginProperty> properties, final CallContext context) throws PaymentPluginApiException {
        final UUID tenantId = context.getTenantId();
        final String connectorTxnId = connectorTransactionIdForPayment(kbPaymentId, tenantId);
        if (connectorTxnId == null) {
            return persistFailure(kbAccountId, kbPaymentId, kbTransactionId, TransactionType.CAPTURE, amount, currency,
                                  PaymentPluginStatus.CANCELED, "No prior authorization to capture", "NO_AUTHORIZATION", tenantId);
        }
        try {
            final PaymentServiceCaptureRequest req = PrismRequestBuilder.capture(kbTransactionId.toString(), connectorTxnId, AmountConverter.toMinor(amount, currency), currency.name());
            final PaymentServiceCaptureResponse resp = prismClient.capture(tenantId, req);
            return record(kbAccountId, kbPaymentId, kbTransactionId, TransactionType.CAPTURE, amount, currency,
                          connectorTxnId, safe(() -> resp.getStatus().name()), safe(() -> resp.getError().getUnifiedDetails().getMessage()), tenantId);
        } catch (final PrismClientException e) {
            return recordException(kbAccountId, kbPaymentId, kbTransactionId, TransactionType.CAPTURE, amount, currency, e, tenantId);
        }
    }

    @Override
    public PaymentTransactionInfoPlugin voidPayment(final UUID kbAccountId, final UUID kbPaymentId, final UUID kbTransactionId,
                                                    final UUID kbPaymentMethodId, final Iterable<PluginProperty> properties,
                                                    final CallContext context) throws PaymentPluginApiException {
        final UUID tenantId = context.getTenantId();
        final String connectorTxnId = connectorTransactionIdForPayment(kbPaymentId, tenantId);
        if (connectorTxnId == null) {
            return persistFailure(kbAccountId, kbPaymentId, kbTransactionId, TransactionType.VOID, null, null,
                                  PaymentPluginStatus.CANCELED, "No prior authorization to void", "NO_AUTHORIZATION", tenantId);
        }
        try {
            final PaymentServiceVoidRequest req = PrismRequestBuilder.voidPayment(kbTransactionId.toString(), connectorTxnId);
            final PaymentServiceVoidResponse resp = prismClient.voidPayment(tenantId, req);
            return record(kbAccountId, kbPaymentId, kbTransactionId, TransactionType.VOID, null, null,
                          connectorTxnId, safe(() -> resp.getStatus().name()), safe(() -> resp.getError().getUnifiedDetails().getMessage()), tenantId);
        } catch (final PrismClientException e) {
            return recordException(kbAccountId, kbPaymentId, kbTransactionId, TransactionType.VOID, null, null, e, tenantId);
        }
    }

    @Override
    public PaymentTransactionInfoPlugin refundPayment(final UUID kbAccountId, final UUID kbPaymentId, final UUID kbTransactionId,
                                                      final UUID kbPaymentMethodId, final BigDecimal amount, final Currency currency,
                                                      final Iterable<PluginProperty> properties, final CallContext context) throws PaymentPluginApiException {
        final UUID tenantId = context.getTenantId();
        final HyperswitchResponsesRecord charged = safeGetAuthorization(kbPaymentId, tenantId);
        final String connectorTxnId = charged == null ? null : charged.getHyperswitchId();
        if (connectorTxnId == null) {
            return persistFailure(kbAccountId, kbPaymentId, kbTransactionId, TransactionType.REFUND, amount, currency,
                                  PaymentPluginStatus.CANCELED, "No captured payment to refund", "NO_CAPTURE", tenantId);
        }
        final long paymentMinor = charged.getAmount() != null ? AmountConverter.toMinor(charged.getAmount(), currency) : AmountConverter.toMinor(amount, currency);
        try {
            final PaymentServiceRefundRequest req = PrismRequestBuilder.refund(kbTransactionId.toString(), connectorTxnId, paymentMinor, AmountConverter.toMinor(amount, currency), currency.name(), "customer_request");
            final RefundResponse resp = prismClient.refund(tenantId, req);
            return record(kbAccountId, kbPaymentId, kbTransactionId, TransactionType.REFUND, amount, currency,
                          connectorTxnId, safe(() -> resp.getStatus().name()), safe(() -> resp.getError().getUnifiedDetails().getMessage()), tenantId);
        } catch (final PrismClientException e) {
            return recordException(kbAccountId, kbPaymentId, kbTransactionId, TransactionType.REFUND, amount, currency, e, tenantId);
        }
    }

    @Override
    public PaymentTransactionInfoPlugin creditPayment(final UUID kbAccountId, final UUID kbPaymentId, final UUID kbTransactionId,
                                                      final UUID kbPaymentMethodId, final BigDecimal amount, final Currency currency,
                                                      final Iterable<PluginProperty> properties, final CallContext context) throws PaymentPluginApiException {
        // Prism's default client set has no standalone money-out; unsupported in v1 (see plan risks / PayoutService).
        throw new PaymentPluginApiException("UNSUPPORTED", "creditPayment is not supported by the Hyperswitch Prism plugin");
    }

    @Override
    public List<PaymentTransactionInfoPlugin> getPaymentInfo(final UUID kbAccountId, final UUID kbPaymentId,
                                                             final Iterable<PluginProperty> properties, final org.killbill.billing.util.callcontext.TenantContext context) throws PaymentPluginApiException {
        // Best-effort: PSync any non-terminal payment row before the framework rebuilds the list from the DAO.
        final UUID tenantId = context.getTenantId();
        try {
            for (final HyperswitchResponsesRecord r : dao.getResponses(kbPaymentId, tenantId)) {
                if (r.getHyperswitchId() == null) {
                    continue;
                }
                final Object status = HyperswitchDao.fromAdditionalData(r.getAdditionalData()).get(HyperswitchPaymentTransactionInfoPlugin.PROPERTY_PLUGIN_STATUS);
                if (status == null || !PaymentPluginStatus.PENDING.name().equals(status.toString())) {
                    continue;
                }
                final TransactionType txnType = TransactionType.valueOf(r.getTransactionType());
                if (txnType == TransactionType.REFUND) {
                    rsync(r, tenantId);
                } else if (txnType != TransactionType.CREDIT && r.getCurrency() != null) {
                    psync(r, txnType, tenantId);
                }
            }
        } catch (final SQLException e) {
            logger.warn("Sync refresh failed for payment {}: {}", kbPaymentId, e.getMessage());
        }
        return super.getPaymentInfo(kbAccountId, kbPaymentId, properties, context);
    }

    private void rsync(final HyperswitchResponsesRecord r, final UUID tenantId) {
        try {
            final RefundServiceGetRequest req = PrismRequestBuilder.refundGet(r.getKbPaymentTransactionId(), r.getHyperswitchId());
            final RefundResponse resp = prismClient.refundGet(tenantId, req);
            final String statusName = safe(() -> resp.getStatus().name());
            final PaymentPluginStatus mapped = PrismStatusMapper.map(statusName, TransactionType.REFUND);
            final Map<String, Object> extra = new HashMap<>();
            extra.put(HyperswitchPaymentTransactionInfoPlugin.PROPERTY_PLUGIN_STATUS, mapped.name());
            extra.put(HyperswitchPluginProperties.DATA_PRISM_STATUS, statusName);
            dao.updateResponseAdditionalData(r, extra);
        } catch (final Exception e) {
            logger.warn("RSync for transaction {} failed: {}", r.getKbPaymentTransactionId(), e.getMessage());
        }
    }

    private void psync(final HyperswitchResponsesRecord r, final TransactionType txnType, final UUID tenantId) {
        try {
            final long minor = r.getAmount() == null ? 0L : AmountConverter.toMinor(r.getAmount(), Currency.valueOf(r.getCurrency()));
            final PaymentServiceGetRequest req = PrismRequestBuilder.get(r.getKbPaymentTransactionId(), r.getHyperswitchId(), minor, r.getCurrency());
            final PaymentServiceGetResponse resp = prismClient.get(tenantId, req);
            final String statusName = safe(() -> resp.getStatus().name());
            final PaymentPluginStatus mapped = PrismStatusMapper.map(statusName, txnType);
            final Map<String, Object> extra = new HashMap<>();
            extra.put(HyperswitchPaymentTransactionInfoPlugin.PROPERTY_PLUGIN_STATUS, mapped.name());
            extra.put(HyperswitchPluginProperties.DATA_PRISM_STATUS, statusName);
            dao.updateResponseAdditionalData(r, extra);
        } catch (final Exception e) {
            logger.warn("PSync for transaction {} failed: {}", r.getKbPaymentTransactionId(), e.getMessage());
        }
    }

    // ------------------------------------------------------------------ payment methods (P2 / P4 Phase A)

    @Override
    public void addPaymentMethod(final UUID kbAccountId, final UUID kbPaymentMethodId, final PaymentMethodPlugin paymentMethodProps,
                                 final boolean setDefault, final Iterable<PluginProperty> properties, final CallContext context) throws PaymentPluginApiException {
        final UUID tenantId = context.getTenantId();
        final HyperswitchConfigProperties config = configHandler.getConfigurable(tenantId);
        final Iterable<PluginProperty> allProps = PluginProperties.merge(paymentMethodProps.getProperties(), properties);
        final HyperswitchPluginProperties.Card card = HyperswitchPluginProperties.extractCard(allProps);

        String hyperswitchId = paymentMethodProps.getExternalPaymentMethodId();
        final Map<String, Object> data = new LinkedHashMap<>();

        if (card != null) {
            // Phase A: set up a reusable mandate (customer present) so recurring invoices can charge off-session.
            try {
                final String cc = accountCurrency(kbAccountId, context);
                final PaymentServiceSetupRecurringRequest req = PrismRequestBuilder.setupRecurring(kbPaymentMethodId.toString(), cc, card, config.getReturnUrl());
                final PaymentServiceSetupRecurringResponse resp = prismClient.setupRecurring(tenantId, req);
                final String mandateId = safe(() -> resp.getConnectorRecurringPaymentId());
                if (mandateId != null) {
                    data.put(HyperswitchPluginProperties.DATA_MANDATE_ID, mandateId);
                    if (hyperswitchId == null) {
                        hyperswitchId = mandateId;
                    }
                }
            } catch (final PrismClientException e) {
                throw new PaymentPluginApiException("Unable to set up mandate for payment method", e);
            }
        }

        try {
            dao.addPaymentMethod(kbAccountId, kbPaymentMethodId, hyperswitchId, setDefault, data, clock.getUTCNow(), tenantId);
        } catch (final SQLException e) {
            throw new PaymentPluginApiException("Unable to persist payment method", e);
        }
    }

    // ------------------------------------------------------------------ webhooks (P3)

    @Override
    public GatewayNotification processNotification(final String notification, final Iterable<PluginProperty> properties,
                                                   final CallContext context) throws PaymentPluginApiException {
        return webhookHandler.processNotification(notification, context);
    }

    /** Servlet entrypoint — full HTTP context. Delegates to the same handler as {@link #processNotification}. */
    public HyperswitchGatewayNotification handleWebhook(final UUID tenantId, final String uri, final Map<String, String> headers,
                                                        final byte[] body, final CallContext context) {
        return webhookHandler.handle(tenantId, uri, headers, body, context);
    }

    // ------------------------------------------------------------------ HPP (unused by server-to-server flow)

    @Override
    public HostedPaymentPageFormDescriptor buildFormDescriptor(final UUID kbAccountId, final Iterable<PluginProperty> customFields,
                                                               final Iterable<PluginProperty> properties, final CallContext context) throws PaymentPluginApiException {
        // The server-to-server flow does not use a hosted payment page; return an empty descriptor.
        return new PluginHostedPaymentPageFormDescriptor(kbAccountId, "");
    }

    // ------------------------------------------------------------------ helpers

    private PaymentTransactionInfoPlugin record(final UUID kbAccountId, final UUID kbPaymentId, final UUID kbTransactionId,
                                                final TransactionType txnType, final BigDecimal amount, final Currency currency,
                                                final String connectorTxnId, final String statusName, final String gatewayError,
                                                final UUID tenantId) throws PaymentPluginApiException {
        final PaymentPluginStatus pluginStatus = PrismStatusMapper.map(statusName, txnType);
        final Map<String, Object> data = HyperswitchPluginProperties.responseData(pluginStatus.name(), statusName, connectorTxnId, gatewayError, null);
        return persist(kbAccountId, kbPaymentId, kbTransactionId, txnType, amount, currency, connectorTxnId, data, tenantId);
    }

    private PaymentTransactionInfoPlugin recordException(final UUID kbAccountId, final UUID kbPaymentId, final UUID kbTransactionId,
                                                         final TransactionType txnType, final BigDecimal amount, final Currency currency,
                                                         final PrismClientException e, final UUID tenantId) throws PaymentPluginApiException {
        final PaymentPluginStatus st = e.isRetryable() ? PaymentPluginStatus.ERROR : PaymentPluginStatus.CANCELED;
        return persistFailure(kbAccountId, kbPaymentId, kbTransactionId, txnType, amount, currency, st, e.getMessage(), e.getErrorCode(), tenantId);
    }

    private PaymentTransactionInfoPlugin persistFailure(final UUID kbAccountId, final UUID kbPaymentId, final UUID kbTransactionId,
                                                        final TransactionType txnType, final BigDecimal amount, final Currency currency,
                                                        final PaymentPluginStatus status, final String message, final String code,
                                                        final UUID tenantId) throws PaymentPluginApiException {
        final Map<String, Object> data = HyperswitchPluginProperties.responseData(status.name(), null, null, message, code);
        return persist(kbAccountId, kbPaymentId, kbTransactionId, txnType, amount, currency, null, data, tenantId);
    }

    private PaymentTransactionInfoPlugin persist(final UUID kbAccountId, final UUID kbPaymentId, final UUID kbTransactionId,
                                                 final TransactionType txnType, final BigDecimal amount, final Currency currency,
                                                 final String connectorTxnId, final Map<String, Object> data, final UUID tenantId) throws PaymentPluginApiException {
        try {
            final HyperswitchResponsesRecord r = dao.addResponse(kbAccountId, kbPaymentId, kbTransactionId, txnType, amount, currency, connectorTxnId, data, clock.getUTCNow(), tenantId);
            return HyperswitchPaymentTransactionInfoPlugin.build(r);
        } catch (final SQLException e) {
            throw new PaymentPluginApiException("Unable to persist Hyperswitch response for " + txnType, e);
        }
    }

    private String connectorTransactionIdForPayment(final UUID kbPaymentId, final UUID tenantId) {
        final HyperswitchResponsesRecord auth = safeGetAuthorization(kbPaymentId, tenantId);
        return auth == null ? null : auth.getHyperswitchId();
    }

    private HyperswitchResponsesRecord safeGetAuthorization(final UUID kbPaymentId, final UUID tenantId) {
        try {
            return dao.getSuccessfulAuthorizationResponse(kbPaymentId, tenantId);
        } catch (final SQLException e) {
            logger.warn("Failed to load authorization for payment {}: {}", kbPaymentId, e.getMessage());
            return null;
        }
    }

    private StoredCredential loadStoredCredential(final UUID kbPaymentMethodId, final UUID tenantId) {
        if (kbPaymentMethodId == null) {
            return null;
        }
        try {
            final HyperswitchPaymentMethodsRecord pm = dao.getPaymentMethod(kbPaymentMethodId, tenantId);
            if (pm == null) {
                return null;
            }
            final Object mandate = HyperswitchDao.fromAdditionalData(pm.getAdditionalData()).get(HyperswitchPluginProperties.DATA_MANDATE_ID);
            return new StoredCredential(mandate == null ? null : mandate.toString(), pm.getHyperswitchId());
        } catch (final SQLException e) {
            logger.warn("Failed to load payment method {}: {}", kbPaymentMethodId, e.getMessage());
            return null;
        }
    }

    private String accountCurrency(final UUID kbAccountId, final CallContext context) throws PaymentPluginApiException {
        try {
            final Account account = killbillAPI.getAccountUserApi().getAccountById(kbAccountId, context);
            return account.getCurrency().name();
        } catch (final AccountApiException e) {
            throw new PaymentPluginApiException("Unable to resolve account currency for mandate setup", e);
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

    /** A reusable off-session credential for merchant-initiated charges. */
    private static final class StoredCredential {
        final String mandateId;
        final String token;

        StoredCredential(final String mandateId, final String token) {
            this.mandateId = mandateId;
            this.token = token;
        }
    }
}
