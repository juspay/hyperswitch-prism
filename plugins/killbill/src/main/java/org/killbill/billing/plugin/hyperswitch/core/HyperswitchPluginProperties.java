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
package org.killbill.billing.plugin.hyperswitch.core;

import java.util.LinkedHashMap;
import java.util.Map;

import javax.annotation.Nullable;

import org.killbill.billing.payment.api.PluginProperty;
import org.killbill.billing.plugin.api.PluginProperties;
import org.killbill.billing.plugin.hyperswitch.model.HyperswitchPaymentTransactionInfoPlugin;

/**
 * Plugin-property keys and helpers: raw-card extraction (KillBill passes card data as plugin properties, using
 * the framework's standard {@code cc*} names) and construction of the {@code additional_data} JSON persisted on
 * each response row.
 */
public final class HyperswitchPluginProperties {

    // Standard KillBill card plugin-property keys.
    public static final String PROPERTY_CC_NUMBER = "ccNumber";
    public static final String PROPERTY_CC_EXPIRATION_MONTH = "ccExpirationMonth";
    public static final String PROPERTY_CC_EXPIRATION_YEAR = "ccExpirationYear";
    public static final String PROPERTY_CC_VERIFICATION_VALUE = "ccVerificationValue";
    public static final String PROPERTY_CC_FIRST_NAME = "ccFirstName";
    public static final String PROPERTY_CC_LAST_NAME = "ccLastName";

    // Signals that a payment method / first purchase should be stored for future (off-session) reuse — triggers
    // Prism setup_recurring / tokenize (Phase A of the recurring flow).
    public static final String PROPERTY_STORE_FOR_FUTURE_USE = "setupFutureUsage";

    // additional_data keys.
    public static final String DATA_PRISM_STATUS = "prismStatus";
    public static final String DATA_CONNECTOR_TRANSACTION_ID = "connectorTransactionId";
    public static final String DATA_MANDATE_ID = "connectorRecurringPaymentId";
    public static final String DATA_TOKEN = "connectorToken";

    private HyperswitchPluginProperties() {
    }

    @Nullable
    public static String get(final Iterable<PluginProperty> properties, final String key) {
        final Object value = PluginProperties.findPluginPropertyValue(key, properties);
        return value == null ? null : value.toString();
    }

    public static boolean isTrue(final Iterable<PluginProperty> properties, final String key) {
        return Boolean.parseBoolean(get(properties, key));
    }

    /** Extract raw card data from plugin properties, or {@code null} when the payment is card-not-present. */
    @Nullable
    public static Card extractCard(final Iterable<PluginProperty> properties) {
        final String number = get(properties, PROPERTY_CC_NUMBER);
        if (number == null || number.isEmpty()) {
            return null;
        }
        final String first = get(properties, PROPERTY_CC_FIRST_NAME);
        final String last = get(properties, PROPERTY_CC_LAST_NAME);
        final String holder = ((first == null ? "" : first) + " " + (last == null ? "" : last)).trim();
        return new Card(number,
                        get(properties, PROPERTY_CC_EXPIRATION_MONTH),
                        get(properties, PROPERTY_CC_EXPIRATION_YEAR),
                        get(properties, PROPERTY_CC_VERIFICATION_VALUE),
                        holder.isEmpty() ? null : holder);
    }

    /**
     * Build the additional_data map persisted on a response row. {@code pluginStatus} is the already-mapped
     * {@link org.killbill.billing.payment.plugin.api.PaymentPluginStatus} name, read back by
     * {@link HyperswitchPaymentTransactionInfoPlugin}.
     */
    public static Map<String, Object> responseData(final String pluginStatus,
                                                   @Nullable final String prismStatus,
                                                   @Nullable final String connectorTransactionId,
                                                   @Nullable final String gatewayError,
                                                   @Nullable final String gatewayErrorCode) {
        final Map<String, Object> data = new LinkedHashMap<>();
        data.put(HyperswitchPaymentTransactionInfoPlugin.PROPERTY_PLUGIN_STATUS, pluginStatus);
        if (prismStatus != null) {
            data.put(DATA_PRISM_STATUS, prismStatus);
        }
        if (connectorTransactionId != null) {
            data.put(DATA_CONNECTOR_TRANSACTION_ID, connectorTransactionId);
        }
        if (gatewayError != null) {
            data.put(HyperswitchPaymentTransactionInfoPlugin.PROPERTY_GATEWAY_ERROR, gatewayError);
        }
        if (gatewayErrorCode != null) {
            data.put(HyperswitchPaymentTransactionInfoPlugin.PROPERTY_GATEWAY_ERROR_CODE, gatewayErrorCode);
        }
        return data;
    }

    /** Raw card data (secrets — never logged or persisted). */
    public static final class Card {
        public final String number;
        public final String expMonth;
        public final String expYear;
        public final String cvc;
        public final String holderName;

        public Card(final String number, final String expMonth, final String expYear, final String cvc, @Nullable final String holderName) {
            this.number = number;
            this.expMonth = expMonth;
            this.expYear = expYear;
            this.cvc = cvc;
            this.holderName = holderName;
        }
    }
}
