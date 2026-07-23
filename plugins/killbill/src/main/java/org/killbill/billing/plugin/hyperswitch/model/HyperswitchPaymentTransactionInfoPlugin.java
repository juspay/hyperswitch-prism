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
import java.util.List;
import java.util.UUID;

import javax.annotation.Nullable;

import org.joda.time.DateTime;
import org.killbill.billing.catalog.api.Currency;
import org.killbill.billing.payment.api.PluginProperty;
import org.killbill.billing.payment.api.TransactionType;
import org.killbill.billing.payment.plugin.api.PaymentPluginStatus;
import org.killbill.billing.plugin.api.payment.PluginPaymentTransactionInfoPlugin;

import com.google.common.collect.ImmutableList;

/**
 * A KillBill {@code PaymentTransactionInfoPlugin} built from plain fields (DB-less). The connector transaction id
 * is surfaced as {@code firstPaymentReferenceId}; the plugin status is already mapped by {@code PrismStatusMapper}.
 */
public class HyperswitchPaymentTransactionInfoPlugin extends PluginPaymentTransactionInfoPlugin {

    private static final int ERROR_CODE_MAX_LENGTH = 32;

    public static HyperswitchPaymentTransactionInfoPlugin build(final UUID kbPaymentId,
                                                                final UUID kbTransactionId,
                                                                final TransactionType transactionType,
                                                                final BigDecimal amount,
                                                                final Currency currency,
                                                                final PaymentPluginStatus pluginStatus,
                                                                @Nullable final String connectorTransactionId,
                                                                @Nullable final String gatewayError,
                                                                @Nullable final String gatewayErrorCode,
                                                                final DateTime effectiveDate) {
        return new HyperswitchPaymentTransactionInfoPlugin(kbPaymentId, kbTransactionId, transactionType, amount, currency,
                                                           pluginStatus, gatewayError, truncate(gatewayErrorCode),
                                                           connectorTransactionId, null, effectiveDate, effectiveDate,
                                                           ImmutableList.<PluginProperty>of());
    }

    @Nullable
    private static String truncate(@Nullable final String s) {
        if (s == null) {
            return null;
        }
        return s.length() <= ERROR_CODE_MAX_LENGTH ? s : s.substring(0, ERROR_CODE_MAX_LENGTH);
    }

    public HyperswitchPaymentTransactionInfoPlugin(final UUID kbPaymentId,
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
        super(kbPaymentId, kbTransactionId, transactionType, amount, currency, pluginStatus, gatewayError,
              gatewayErrorCode, firstPaymentReferenceId, secondPaymentReferenceId, createdDate, effectiveDate, properties);
    }
}
