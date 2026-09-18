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
package org.killbill.billing.plugin.hyperswitch.model;

import java.math.BigDecimal;
import java.time.ZoneOffset;
import java.util.List;
import java.util.Map;
import java.util.UUID;

import javax.annotation.Nullable;

import org.joda.time.DateTime;
import org.joda.time.DateTimeZone;
import org.killbill.billing.catalog.api.Currency;
import org.killbill.billing.payment.api.PluginProperty;
import org.killbill.billing.payment.api.TransactionType;
import org.killbill.billing.payment.plugin.api.PaymentPluginStatus;
import org.killbill.billing.plugin.api.PluginProperties;
import org.killbill.billing.plugin.api.payment.PluginPaymentTransactionInfoPlugin;
import org.killbill.billing.plugin.hyperswitch.dao.HyperswitchDao;
import org.killbill.billing.plugin.hyperswitch.dao.gen.tables.records.HyperswitchResponsesRecord;

import com.google.common.base.Strings;

/**
 * Rebuilds a KillBill {@link PaymentTransactionInfoPlugin} from a persisted {@code hyperswitch_responses} row.
 * The plugin status (already mapped from Prism's status by {@code PrismStatusMapper}) and gateway error live in
 * {@code additional_data}; {@code hyperswitch_id} is the connector transaction id, surfaced as the first
 * payment reference id. Mirrors {@code StripePaymentTransactionInfoPlugin}.
 */
public class HyperswitchPaymentTransactionInfoPlugin extends PluginPaymentTransactionInfoPlugin {

    private static final int ERROR_CODE_MAX_LENGTH = 32;

    /** additional_data key holding the already-mapped {@link PaymentPluginStatus} name (written by PrismService). */
    public static final String PROPERTY_PLUGIN_STATUS = "pluginStatus";
    public static final String PROPERTY_GATEWAY_ERROR = "gatewayError";
    public static final String PROPERTY_GATEWAY_ERROR_CODE = "gatewayErrorCode";

    private final HyperswitchResponsesRecord record;

    public static HyperswitchPaymentTransactionInfoPlugin build(final HyperswitchResponsesRecord record) {
        final Map<?, ?> additionalData = HyperswitchDao.fromAdditionalData(record.getAdditionalData());

        final DateTime responseDate = new DateTime(record.getCreatedDate().atZone(ZoneOffset.UTC).toInstant().toEpochMilli(), DateTimeZone.UTC);

        return new HyperswitchPaymentTransactionInfoPlugin(record,
                                                           UUID.fromString(record.getKbPaymentId()),
                                                           UUID.fromString(record.getKbPaymentTransactionId()),
                                                           TransactionType.valueOf(record.getTransactionType()),
                                                           record.getAmount(),
                                                           Strings.isNullOrEmpty(record.getCurrency()) ? null : Currency.valueOf(record.getCurrency()),
                                                           pluginStatus(additionalData),
                                                           (String) additionalData.get(PROPERTY_GATEWAY_ERROR),
                                                           truncate((String) additionalData.get(PROPERTY_GATEWAY_ERROR_CODE)),
                                                           record.getHyperswitchId(),
                                                           null,
                                                           responseDate,
                                                           responseDate,
                                                           PluginProperties.buildPluginProperties(additionalData));
    }

    private static PaymentPluginStatus pluginStatus(final Map<?, ?> additionalData) {
        final Object status = additionalData.get(PROPERTY_PLUGIN_STATUS);
        if (status == null) {
            return PaymentPluginStatus.UNDEFINED;
        }
        try {
            return PaymentPluginStatus.valueOf(status.toString());
        } catch (final IllegalArgumentException e) {
            return PaymentPluginStatus.UNDEFINED;
        }
    }

    private static String truncate(@Nullable final String s) {
        if (s == null) {
            return null;
        }
        return s.length() <= ERROR_CODE_MAX_LENGTH ? s : s.substring(0, ERROR_CODE_MAX_LENGTH);
    }

    public HyperswitchPaymentTransactionInfoPlugin(final HyperswitchResponsesRecord record,
                                                   final UUID kbPaymentId,
                                                   final UUID kbTransactionId,
                                                   final TransactionType transactionType,
                                                   final BigDecimal amount,
                                                   final Currency currency,
                                                   final PaymentPluginStatus pluginStatus,
                                                   final String gatewayError,
                                                   final String gatewayErrorCode,
                                                   final String firstPaymentReferenceId,
                                                   final String secondPaymentReferenceId,
                                                   final DateTime createdDate,
                                                   final DateTime effectiveDate,
                                                   final List<PluginProperty> properties) {
        super(kbPaymentId,
              kbTransactionId,
              transactionType,
              amount,
              currency,
              pluginStatus,
              gatewayError,
              gatewayErrorCode,
              firstPaymentReferenceId,
              secondPaymentReferenceId,
              createdDate,
              effectiveDate,
              properties);
        this.record = record;
    }

    public HyperswitchResponsesRecord getRecord() {
        return record;
    }
}
