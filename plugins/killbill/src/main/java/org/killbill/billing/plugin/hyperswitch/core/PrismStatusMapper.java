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

import org.killbill.billing.payment.api.TransactionType;
import org.killbill.billing.payment.plugin.api.PaymentPluginStatus;

/**
 * Maps Prism's payment/refund status (proto enum, referenced by NAME so this is decoupled from the SDK's
 * enum package) onto KillBill's {@link PaymentPluginStatus}. Operation-aware, because the same Prism status
 * means different things per KillBill {@link TransactionType} (e.g. {@code VOIDED} is success for a VOID but a
 * cancellation for an AUTHORIZE).
 *
 * <p>Java port of the Medusa plugin's {@code mapPrismStatus}/{@code mapRefundStatus}
 * ({@code plugins/medusa/medusa-custom-payments/src/providers/utils/index.ts}). Status names come from
 * {@code crates/types-traits/grpc-api-types/proto/payment.proto} — {@code PaymentStatus} (L184),
 * {@code RefundStatus} (L254).
 */
public final class PrismStatusMapper {

    private PrismStatusMapper() {
    }

    /**
     * @param statusName the Prism status enum name (e.g. {@code "CHARGED"}, {@code "REFUND_SUCCESS"}); typically
     *                   {@code response.getStatus().name()}.
     * @param transactionType the KillBill transaction this status is being interpreted for.
     */
    public static PaymentPluginStatus map(final String statusName, final TransactionType transactionType) {
        if (statusName == null) {
            return PaymentPluginStatus.UNDEFINED;
        }
        if (transactionType == TransactionType.REFUND || transactionType == TransactionType.CREDIT) {
            return mapRefund(statusName);
        }
        return mapPayment(statusName, transactionType);
    }

    private static PaymentPluginStatus mapPayment(final String s, final TransactionType txnType) {
        switch (s) {
            // Success
            case "AUTHORIZED":
            case "PARTIALLY_AUTHORIZED":
            case "CHARGED":
            case "PARTIAL_CHARGED":
            case "PARTIAL_CHARGED_AND_CHARGEABLE":
                return PaymentPluginStatus.PROCESSED;

            // VOIDED is success only when we asked to void; otherwise the payment was cancelled → error/canceled
            case "VOIDED":
            case "VOIDED_POST_CAPTURE":
                return txnType == TransactionType.VOID ? PaymentPluginStatus.PROCESSED : PaymentPluginStatus.CANCELED;

            // In-flight
            case "STARTED":
            case "AUTHORIZING":
            case "AUTHENTICATION_PENDING":
            case "AUTHENTICATION_SUCCESSFUL":
            case "CONFIRMATION_AWAITED":
            case "PAYMENT_METHOD_AWAITED":
            case "DEVICE_DATA_COLLECTION_PENDING":
            case "CAPTURE_INITIATED":
            case "VOID_INITIATED":
            case "PENDING":
            case "COD_INITIATED":
                return PaymentPluginStatus.PENDING;

            // Terminal failure
            case "AUTHORIZATION_FAILED":
            case "AUTHENTICATION_FAILED":
            case "CAPTURE_FAILED":
            case "VOID_FAILED":
            case "EXPIRED":
            case "FAILURE":
            case "ROUTER_DECLINED":
                return PaymentPluginStatus.ERROR;

            case "UNRESOLVED":
            case "PAYMENT_STATUS_UNSPECIFIED":
            default:
                return PaymentPluginStatus.UNDEFINED;
        }
    }

    private static PaymentPluginStatus mapRefund(final String s) {
        switch (s) {
            case "REFUND_SUCCESS":
                return PaymentPluginStatus.PROCESSED;
            case "REFUND_PENDING":
            case "REFUND_MANUAL_REVIEW":
                return PaymentPluginStatus.PENDING;
            case "REFUND_FAILURE":
            case "REFUND_TRANSACTION_FAILURE":
                return PaymentPluginStatus.ERROR;
            case "REFUND_STATUS_UNSPECIFIED":
            default:
                return PaymentPluginStatus.UNDEFINED;
        }
    }
}
