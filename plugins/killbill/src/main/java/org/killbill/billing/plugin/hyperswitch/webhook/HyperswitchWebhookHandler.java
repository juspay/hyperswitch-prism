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
package org.killbill.billing.plugin.hyperswitch.webhook;

import java.nio.charset.StandardCharsets;
import java.util.Collections;
import java.util.Map;
import java.util.UUID;
import java.util.function.BooleanSupplier;
import java.util.function.Supplier;

import org.killbill.billing.account.api.Account;
import org.killbill.billing.osgi.libs.killbill.OSGIKillbillAPI;
import org.killbill.billing.payment.api.Payment;
import org.killbill.billing.payment.api.PluginProperty;
import org.killbill.billing.plugin.hyperswitch.HyperswitchConfigProperties;
import org.killbill.billing.plugin.hyperswitch.HyperswitchConfigPropertiesConfigurationHandler;
import org.killbill.billing.plugin.hyperswitch.client.PrismClient;
import org.killbill.billing.plugin.hyperswitch.client.PrismClientException;
import org.killbill.billing.plugin.hyperswitch.core.PrismRequestBuilder;
import org.killbill.billing.plugin.hyperswitch.model.HyperswitchGatewayNotification;
import org.killbill.billing.plugin.hyperswitch.store.HyperswitchStateStore;
import org.killbill.billing.util.callcontext.CallContext;
import org.killbill.clock.Clock;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import com.google.common.collect.ImmutableList;

// NOTE (P0 verification): SDK webhook getters preserved from the E2E-verified DB-based version.
import types.Payment.EventServiceHandleRequest;
import types.Payment.EventServiceHandleResponse;

/**
 * Processes inbound connector webhooks via Prism {@code EventService.handle_event}, DB-less: correlates the
 * connector transaction id back to a KillBill transaction through {@link HyperswitchStateStore} (custom-field
 * search) and finalizes it via {@code notifyPendingTransactionOfStateChanged}.
 */
public class HyperswitchWebhookHandler {

    private static final Logger logger = LoggerFactory.getLogger(HyperswitchWebhookHandler.class);
    private static final Iterable<PluginProperty> NO_PROPS = ImmutableList.of();

    private final HyperswitchConfigPropertiesConfigurationHandler configHandler;
    private final PrismClient prismClient;
    private final HyperswitchStateStore store;
    private final OSGIKillbillAPI killbillAPI;
    private final Clock clock;

    public HyperswitchWebhookHandler(final HyperswitchConfigPropertiesConfigurationHandler configHandler,
                                     final PrismClient prismClient,
                                     final HyperswitchStateStore store,
                                     final OSGIKillbillAPI killbillAPI,
                                     final Clock clock) {
        this.configHandler = configHandler;
        this.prismClient = prismClient;
        this.store = store;
        this.killbillAPI = killbillAPI;
        this.clock = clock;
    }

    /** Framework path ({@code processNotification}) — only the raw body is available. */
    public HyperswitchGatewayNotification processNotification(final String notification, final CallContext context) {
        final byte[] body = notification == null ? new byte[0] : notification.getBytes(StandardCharsets.UTF_8);
        return handle(context.getTenantId(), "", Collections.emptyMap(), body, context);
    }

    /** Servlet path — full HTTP context (uri + headers + raw body). */
    public HyperswitchGatewayNotification handle(final UUID tenantId, final String uri, final Map<String, String> headers,
                                                 final byte[] body, final CallContext context) {
        final HyperswitchConfigProperties config = configHandler.getConfigurable(tenantId);
        final EventServiceHandleRequest req = PrismRequestBuilder.handleEvent(UUID.randomUUID().toString(),
                                                                             uri == null ? "" : uri,
                                                                             headers == null ? Collections.emptyMap() : headers,
                                                                             body,
                                                                             config.getWebhookSecret());
        try {
            final EventServiceHandleResponse resp = prismClient.handleEvent(tenantId, req);

            if (!safeBool(() -> resp.getSourceVerified())) {
                logger.warn("Rejecting Hyperswitch webhook: source not verified");
                return HyperswitchGatewayNotification.ack("{\"status\":\"rejected\",\"reason\":\"source_not_verified\"}");
            }

            final String eventType = safe(() -> resp.getEventType().name());
            // The handle response has no top-level transaction id — it rides inside the typed event content
            // (payments_response / refunds_response mirror PaymentService.Get / RefundService.Get).
            final String connectorTxnId = safe(() -> resp.getEventContent().hasPaymentsResponse()
                    ? resp.getEventContent().getPaymentsResponse().getConnectorTransactionId()
                    : resp.getEventContent().getRefundsResponse().getConnectorTransactionId());
            applyEvent(eventType, connectorTxnId, context);
            return HyperswitchGatewayNotification.ack("{\"status\":\"ok\"}");
        } catch (final PrismClientException e) {
            logger.warn("Hyperswitch webhook processing failed: {}", e.getMessage());
            return HyperswitchGatewayNotification.ack("{\"status\":\"error\"}");
        }
    }

    private void applyEvent(final String eventType, final String connectorTxnId, final CallContext context) {
        final Boolean success = outcome(eventType);
        if (success == null || connectorTxnId == null) {
            logger.info("Hyperswitch webhook {} → no state change (connectorTransactionId={})", eventType, connectorTxnId);
            return;
        }
        try {
            final UUID kbTransactionId = store.findKbTransactionByConnectorTxnId(connectorTxnId, context);
            if (kbTransactionId == null) {
                logger.info("No local transaction for connectorTransactionId {}", connectorTxnId);
                return;
            }
            final Payment payment = killbillAPI.getPaymentApi().getPaymentByTransactionId(kbTransactionId, false, false, NO_PROPS, context);
            final Account account = killbillAPI.getAccountUserApi().getAccountById(payment.getAccountId(), context);
            killbillAPI.getPaymentApi().notifyPendingTransactionOfStateChanged(account, kbTransactionId, success, context);
        } catch (final Exception e) {
            logger.warn("Failed to apply Hyperswitch webhook for {}: {}", connectorTxnId, e.getMessage());
        }
    }

    /** WebhookEventType name → success(TRUE) / failure(FALSE) / pending or unknown(null). */
    private static Boolean outcome(final String eventType) {
        if (eventType == null) {
            return null;
        }
        switch (eventType) {
            case "PAYMENT_INTENT_SUCCESS":
            case "PAYMENT_INTENT_CAPTURE_SUCCESS":
            case "PAYMENT_INTENT_AUTHORIZATION_SUCCESS":
            case "WEBHOOK_REFUND_SUCCESS":
                return Boolean.TRUE;
            case "PAYMENT_INTENT_FAILURE":
            case "PAYMENT_INTENT_AUTHORIZATION_FAILURE":
            case "PAYMENT_INTENT_CAPTURE_FAILURE":
            case "PAYMENT_INTENT_CANCELLED":
            case "WEBHOOK_REFUND_FAILURE":
                return Boolean.FALSE;
            default:
                return null;
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

    private static boolean safeBool(final BooleanSupplier supplier) {
        try {
            return supplier.getAsBoolean();
        } catch (final RuntimeException e) {
            return false;
        }
    }
}
