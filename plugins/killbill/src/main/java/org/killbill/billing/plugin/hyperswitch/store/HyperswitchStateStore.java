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
package org.killbill.billing.plugin.hyperswitch.store;

import java.math.BigDecimal;
import java.util.ArrayList;
import java.util.List;
import java.util.UUID;

import javax.annotation.Nullable;

import org.killbill.billing.ObjectType;
import org.killbill.billing.catalog.api.Currency;
import org.killbill.billing.osgi.libs.killbill.OSGIKillbillAPI;
import org.killbill.billing.payment.api.Payment;
import org.killbill.billing.payment.api.PaymentApiException;
import org.killbill.billing.payment.api.PaymentTransaction;
import org.killbill.billing.payment.api.PluginProperty;
import org.killbill.billing.payment.api.TransactionType;
import org.killbill.billing.plugin.api.core.PluginCustomField;
import org.killbill.billing.util.api.CustomFieldApiException;
import org.killbill.billing.util.callcontext.CallContext;
import org.killbill.billing.util.callcontext.TenantContext;
import org.killbill.billing.util.customfield.CustomField;
import org.killbill.billing.util.entity.Pagination;
import org.killbill.clock.Clock;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import com.google.common.collect.ImmutableList;

/**
 * DB-less state store: replaces the jOOQ {@code HyperswitchDao} by persisting the plugin's connector state in
 * KillBill's own storage — Custom Fields (key/value, ≤255 chars) on the TRANSACTION and PAYMENT_METHOD objects,
 * plus reads from the core Payment API. KillBill has no opaque replayed blob, so the plugin remains the source
 * of truth for gateway reference ids and re-serves them via {@code getPaymentInfo}.
 *
 * <p>NOTE: custom-field writes are separate transactions from the core payment state machine (best-effort), so
 * callers must tolerate a missing field. Values are capped at 255 chars (fine for ids/tokens/mandates).
 */
public class HyperswitchStateStore {

    private static final Logger logger = LoggerFactory.getLogger(HyperswitchStateStore.class);
    private static final Iterable<PluginProperty> NO_PROPS = ImmutableList.of();

    public static final String CF_CONNECTOR_TXN_ID = "HS_CONNECTOR_TXN_ID"; // on TRANSACTION
    public static final String CF_PRISM_STATUS = "HS_PRISM_STATUS";         // on TRANSACTION (raw status, audit)
    public static final String CF_MANDATE_ID = "HS_MANDATE_ID";             // on PAYMENT_METHOD
    public static final String CF_TOKEN = "HS_TOKEN";                       // on PAYMENT_METHOD
    public static final String CF_CUSTOMER_ID = "HS_CUSTOMER_ID";           // on PAYMENT_METHOD

    private final OSGIKillbillAPI killbillAPI;
    private final Clock clock;

    public HyperswitchStateStore(final OSGIKillbillAPI killbillAPI, final Clock clock) {
        this.killbillAPI = killbillAPI;
        this.clock = clock;
    }

    // ------------------------------------------------------------------ transactions

    public void saveConnectorTransactionId(final UUID kbTransactionId, @Nullable final String connectorTxnId,
                                           @Nullable final String prismStatus, final CallContext context) {
        final List<CustomField> fields = new ArrayList<>();
        if (connectorTxnId != null) {
            fields.add(new PluginCustomField(kbTransactionId, ObjectType.TRANSACTION, CF_CONNECTOR_TXN_ID, connectorTxnId, clock.getUTCNow()));
        }
        if (prismStatus != null) {
            fields.add(new PluginCustomField(kbTransactionId, ObjectType.TRANSACTION, CF_PRISM_STATUS, prismStatus, clock.getUTCNow()));
        }
        addQuietly(fields, context);
    }

    @Nullable
    public String getConnectorTransactionId(final UUID kbTransactionId, final TenantContext context) {
        return fieldValue(killbillAPI.getCustomFieldUserApi().getCustomFieldsForObject(kbTransactionId, ObjectType.TRANSACTION, context), CF_CONNECTOR_TXN_ID);
    }

    /** The successful AUTHORIZE/PURCHASE of a payment: its gateway ref + original amount (capture/void/refund). */
    @Nullable
    public AuthInfo findAuthorization(final UUID kbPaymentId, final TenantContext context) {
        try {
            final Payment payment = killbillAPI.getPaymentApi().getPayment(kbPaymentId, false, false, NO_PROPS, context);
            final List<PaymentTransaction> txns = payment.getTransactions();
            for (int i = txns.size() - 1; i >= 0; i--) {
                final PaymentTransaction t = txns.get(i);
                if (t.getTransactionType() == TransactionType.AUTHORIZE || t.getTransactionType() == TransactionType.PURCHASE) {
                    final String connectorTxnId = getConnectorTransactionId(t.getId(), context);
                    if (connectorTxnId != null) {
                        return new AuthInfo(connectorTxnId, t.getAmount(), t.getCurrency());
                    }
                }
            }
        } catch (final PaymentApiException e) {
            logger.warn("findAuthorization failed for payment {}: {}", kbPaymentId, e.getMessage());
        }
        return null;
    }

    /** Webhook correlation: connector transaction id → KillBill transaction id (via the indexed cf search). */
    @Nullable
    public UUID findKbTransactionByConnectorTxnId(final String connectorTxnId, final TenantContext context) {
        final Pagination<CustomField> matches = killbillAPI.getCustomFieldUserApi()
                .searchCustomFields(CF_CONNECTOR_TXN_ID, connectorTxnId, ObjectType.TRANSACTION, 0L, 10L, context);
        for (final CustomField cf : matches) {
            if (connectorTxnId.equals(cf.getFieldValue())) {
                return cf.getObjectId();
            }
        }
        return null;
    }

    // ------------------------------------------------------------------ payment methods

    public void savePaymentMethodCredential(final UUID kbPaymentMethodId, @Nullable final String mandateId,
                                            @Nullable final String token, @Nullable final String customerId, final CallContext context) {
        final List<CustomField> fields = new ArrayList<>();
        if (mandateId != null) {
            fields.add(new PluginCustomField(kbPaymentMethodId, ObjectType.PAYMENT_METHOD, CF_MANDATE_ID, mandateId, clock.getUTCNow()));
        }
        if (token != null) {
            fields.add(new PluginCustomField(kbPaymentMethodId, ObjectType.PAYMENT_METHOD, CF_TOKEN, token, clock.getUTCNow()));
        }
        if (customerId != null) {
            fields.add(new PluginCustomField(kbPaymentMethodId, ObjectType.PAYMENT_METHOD, CF_CUSTOMER_ID, customerId, clock.getUTCNow()));
        }
        addQuietly(fields, context);
    }

    public StoredCredential getStoredCredential(final UUID kbPaymentMethodId, final TenantContext context) {
        final List<CustomField> cfs = killbillAPI.getCustomFieldUserApi().getCustomFieldsForObject(kbPaymentMethodId, ObjectType.PAYMENT_METHOD, context);
        return new StoredCredential(fieldValue(cfs, CF_MANDATE_ID), fieldValue(cfs, CF_TOKEN), fieldValue(cfs, CF_CUSTOMER_ID));
    }

    public void deletePaymentMethodCredential(final UUID kbPaymentMethodId, final CallContext context) {
        final List<CustomField> cfs = killbillAPI.getCustomFieldUserApi().getCustomFieldsForObject(kbPaymentMethodId, ObjectType.PAYMENT_METHOD, context);
        final List<CustomField> ours = new ArrayList<>();
        for (final CustomField cf : cfs) {
            final String n = cf.getFieldName();
            if (CF_MANDATE_ID.equals(n) || CF_TOKEN.equals(n) || CF_CUSTOMER_ID.equals(n)) {
                ours.add(cf);
            }
        }
        if (!ours.isEmpty()) {
            try {
                killbillAPI.getCustomFieldUserApi().removeCustomFields(ours, context);
            } catch (final CustomFieldApiException e) {
                logger.warn("Failed to remove custom fields for payment method {}: {}", kbPaymentMethodId, e.getMessage());
            }
        }
    }

    // ------------------------------------------------------------------ helpers

    private void addQuietly(final List<CustomField> fields, final CallContext context) {
        if (fields.isEmpty()) {
            return;
        }
        try {
            killbillAPI.getCustomFieldUserApi().addCustomFields(fields, context);
        } catch (final CustomFieldApiException e) {
            logger.warn("Failed to persist custom fields {}: {}", fields, e.getMessage());
        }
    }

    @Nullable
    private static String fieldValue(final List<CustomField> fields, final String name) {
        for (final CustomField cf : fields) {
            if (name.equals(cf.getFieldName())) {
                return cf.getFieldValue();
            }
        }
        return null;
    }

    /** Gateway ref + original amount of the authorizing transaction. */
    public static final class AuthInfo {
        public final String connectorTransactionId;
        public final BigDecimal amount;
        public final Currency currency;

        AuthInfo(final String connectorTransactionId, final BigDecimal amount, final Currency currency) {
            this.connectorTransactionId = connectorTransactionId;
            this.amount = amount;
            this.currency = currency;
        }
    }

    /** A reusable off-session credential (mandate reference, connector token, connector customer id). */
    public static final class StoredCredential {
        public final String mandateId;
        public final String token;
        public final String customerId;

        StoredCredential(@Nullable final String mandateId, @Nullable final String token, @Nullable final String customerId) {
            this.mandateId = mandateId;
            this.token = token;
            this.customerId = customerId;
        }
    }
}
