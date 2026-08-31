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

import java.util.Map;

import javax.annotation.Nullable;

import org.killbill.billing.plugin.hyperswitch.core.HyperswitchPluginProperties.Card;

import com.google.protobuf.ByteString;

// NOTE (P0 verification): SDK proto message/enum names + builder methods below follow the in-repo Gradle layout
// and examples/stripe/stripe.kt. Verify against io.hyperswitch:prism:0.0.6 at first compile (enum values, whether
// card fields are SecretString sub-builders, Money vs Amount, webhook_secrets field name).
import types.Payment.AcceptanceType;
import types.Payment.AuthenticationType;
import types.Payment.CaptureMethod;
import types.Payment.Currency;
import types.Payment.FutureUsage;
import types.Payment.HttpMethod;
import types.Payment.EventServiceHandleRequest;
import types.Payment.PaymentServiceAuthorizeRequest;
import types.Payment.PaymentServiceCaptureRequest;
import types.Payment.PaymentServiceGetRequest;
import types.Payment.PaymentServiceRefundRequest;
import types.Payment.PaymentServiceSetupRecurringRequest;
import types.Payment.PaymentServiceTokenAuthorizeRequest;
import types.Payment.PaymentServiceVoidRequest;
import types.Payment.RecurringPaymentServiceChargeRequest;
import types.Payment.RefundServiceGetRequest;

/**
 * Builds the Prism SDK proto requests from KillBill inputs. All amounts are minor units (see
 * {@link AmountConverter}); {@code merchant*Id} carries the KillBill transaction id for connector-side
 * idempotency across retries. Faithful port of the request builders in {@code examples/stripe/stripe.kt}.
 */
public final class PrismRequestBuilder {

    private PrismRequestBuilder() {
    }

    // ---- one-off card flows (P1) -------------------------------------------------------------

    public static PaymentServiceAuthorizeRequest authorize(final String merchantTransactionId,
                                                           final long minorAmount,
                                                           final String currencyCode,
                                                           final Card card,
                                                           final String captureMethod,
                                                           final boolean threeDs,
                                                           @Nullable final String returnUrl) {
        final PaymentServiceAuthorizeRequest.Builder b = PaymentServiceAuthorizeRequest.newBuilder();
        b.setMerchantTransactionId(merchantTransactionId);
        b.getAmountBuilder().setMinorAmount(minorAmount).setCurrency(currency(currencyCode));
        setCard(b.getPaymentMethodBuilder().getCardBuilder(), card);
        b.setCaptureMethod(CaptureMethod.valueOf(captureMethod));
        b.getAddressBuilder().getBillingAddressBuilder();
        b.setAuthType(threeDs ? AuthenticationType.THREE_DS : AuthenticationType.NO_THREE_DS);
        if (returnUrl != null) {
            b.setReturnUrl(returnUrl);
        }
        return b.build();
    }

    public static PaymentServiceCaptureRequest capture(final String merchantCaptureId,
                                                      final String connectorTransactionId,
                                                      final long minorAmount,
                                                      final String currencyCode) {
        final PaymentServiceCaptureRequest.Builder b = PaymentServiceCaptureRequest.newBuilder();
        b.setMerchantCaptureId(merchantCaptureId);
        b.setConnectorTransactionId(connectorTransactionId);
        b.getAmountToCaptureBuilder().setMinorAmount(minorAmount).setCurrency(currency(currencyCode));
        return b.build();
    }

    public static PaymentServiceVoidRequest voidPayment(final String merchantVoidId, final String connectorTransactionId) {
        return PaymentServiceVoidRequest.newBuilder()
                .setMerchantVoidId(merchantVoidId)
                .setConnectorTransactionId(connectorTransactionId)
                .build();
    }

    public static PaymentServiceGetRequest get(final String merchantTransactionId,
                                              final String connectorTransactionId,
                                              final long minorAmount,
                                              final String currencyCode) {
        final PaymentServiceGetRequest.Builder b = PaymentServiceGetRequest.newBuilder();
        b.setMerchantTransactionId(merchantTransactionId);
        b.setConnectorTransactionId(connectorTransactionId);
        b.getAmountBuilder().setMinorAmount(minorAmount).setCurrency(currency(currencyCode));
        return b.build();
    }

    public static PaymentServiceRefundRequest refund(final String merchantRefundId,
                                                    final String connectorTransactionId,
                                                    final long paymentMinorAmount,
                                                    final long refundMinorAmount,
                                                    final String currencyCode,
                                                    @Nullable final String reason) {
        final PaymentServiceRefundRequest.Builder b = PaymentServiceRefundRequest.newBuilder();
        b.setMerchantRefundId(merchantRefundId);
        b.setConnectorTransactionId(connectorTransactionId);
        b.setPaymentAmount(paymentMinorAmount);
        b.getRefundAmountBuilder().setMinorAmount(refundMinorAmount).setCurrency(currency(currencyCode));
        if (reason != null) {
            b.setReason(reason);
        }
        return b.build();
    }

    public static RefundServiceGetRequest refundGet(final String merchantRefundId, final String connectorTransactionId) {
        return RefundServiceGetRequest.newBuilder()
                .setMerchantRefundId(merchantRefundId)
                .setConnectorTransactionId(connectorTransactionId)
                .build();
    }

    // ---- recurring / mandates (P4) -----------------------------------------------------------

    /** Create the customer at the connector — e.g. Stripe requires a Customer to reuse a mandate off-session. */
    public static types.Payment.CustomerServiceCreateRequest customerCreate(final String merchantCustomerId,
                                                                            @Nullable final String name,
                                                                            @Nullable final String email) {
        final types.Payment.CustomerServiceCreateRequest.Builder b = types.Payment.CustomerServiceCreateRequest.newBuilder();
        b.setMerchantCustomerId(merchantCustomerId);
        if (name != null) {
            b.setCustomerName(name);
        }
        if (email != null) {
            b.getEmailBuilder().setValue(email);
        }
        return b.build();
    }

    /** Populate the billing address (and customer email) from account data — required by e.g. Cybersource. */
    private static void setBilling(final PaymentServiceSetupRecurringRequest.Builder b,
                                   @Nullable final HyperswitchPluginProperties.Billing billing) {
        if (billing == null) {
            return;
        }
        final types.Payment.Address.Builder addr = b.getAddressBuilder().getBillingAddressBuilder();
        if (billing.firstName != null) addr.getFirstNameBuilder().setValue(billing.firstName);
        if (billing.lastName != null) addr.getLastNameBuilder().setValue(billing.lastName);
        if (billing.email != null) {
            addr.getEmailBuilder().setValue(billing.email);
            b.getCustomerBuilder().getEmailBuilder().setValue(billing.email);
        }
        if (billing.line1 != null) addr.getLine1Builder().setValue(billing.line1);
        if (billing.city != null) addr.getCityBuilder().setValue(billing.city);
        if (billing.state != null) addr.getStateBuilder().setValue(billing.state);
        if (billing.zip != null) addr.getZipCodeBuilder().setValue(billing.zip);
        if (billing.country != null) {
            try {
                addr.setCountryAlpha2Code(types.PaymentMethods.CountryAlpha2.valueOf(billing.country.toUpperCase()));
            } catch (final IllegalArgumentException ignore) {
                // unknown ISO code — leave unset rather than fail the whole request
            }
        }
    }

    public static PaymentServiceSetupRecurringRequest setupRecurring(final String merchantRecurringPaymentId,
                                                                    final String currencyCode,
                                                                    final Card card,
                                                                    @Nullable final HyperswitchPluginProperties.Billing billing,
                                                                    final String customerId,
                                                                    @Nullable final String connectorCustomerId,
                                                                    @Nullable final String connectorMetadata,
                                                                    @Nullable final String returnUrl) {
        final PaymentServiceSetupRecurringRequest.Builder b = PaymentServiceSetupRecurringRequest.newBuilder();
        b.setMerchantRecurringPaymentId(merchantRecurringPaymentId);
        // Connector-specific metadata JSON — some transformers (Cybersource) fail outright when absent, so
        // callers pass at least "{}".
        if (connectorMetadata != null) {
            b.getMetadataBuilder().setValue(connectorMetadata);
        }
        // customer.id (merchant-side reference) is REQUIRED by connectors that key mandates on a shopper
        // reference (Adyen); connector_customer_id carries a connector-created customer (Stripe cus_…).
        b.getCustomerBuilder().setId(customerId);
        if (connectorCustomerId != null) {
            b.getCustomerBuilder().setConnectorCustomerId(connectorCustomerId);
        }
        b.getAmountBuilder().setMinorAmount(0L).setCurrency(currency(currencyCode));
        setCard(b.getPaymentMethodBuilder().getCardBuilder(), card);
        setBilling(b, billing);
        b.setAuthType(AuthenticationType.NO_THREE_DS);
        b.setSetupFutureUsage(FutureUsage.OFF_SESSION);
        b.getCustomerAcceptanceBuilder().setAcceptanceType(AcceptanceType.OFFLINE).setAcceptedAt(0L);
        // Some connectors (Adyen) require browser_info even for server-to-server card setup; KillBill has no
        // real browser context, so send a minimal synthetic one (same approach as other headless integrators).
        b.getBrowserInfoBuilder()
         .setJavaScriptEnabled(false)
         .setJavaEnabled(false)
         .setColorDepth(24)
         .setScreenHeight(1080)
         .setScreenWidth(1920)
         .setTimeZoneOffsetMinutes(0)
         .setLanguage("en-US")
         .setAcceptHeader("*/*")
         .setUserAgent("KillBill-Hyperswitch-Plugin");
        if (returnUrl != null) {
            b.setReturnUrl(returnUrl);
        }
        return b.build();
    }

    public static RecurringPaymentServiceChargeRequest charge(final String merchantChargeId,
                                                             final String mandateId,
                                                             final long minorAmount,
                                                             final String currencyCode,
                                                             @Nullable final String connectorCustomerId,
                                                             @Nullable final String connectorMetadata,
                                                             @Nullable final String returnUrl) {
        final RecurringPaymentServiceChargeRequest.Builder b = RecurringPaymentServiceChargeRequest.newBuilder();
        // Merchant-side reference for the charge — required by connectors like Adyen ("reference").
        b.setMerchantChargeId(merchantChargeId);
        if (connectorMetadata != null) {
            b.getMetadataBuilder().setValue(connectorMetadata);
        }
        if (connectorCustomerId != null) {
            b.setConnectorCustomerId(connectorCustomerId);
        }
        // MandateReference.connector_mandate_id is a ConnectorMandateReferenceId carrying the id string directly.
        // Do NOT also set payment_method: a mandate charge must present ONLY the mandate reference — adding a
        // token turns it into a token payment, which connectors like Braintree reject for the repeat flow.
        b.getConnectorRecurringPaymentIdBuilder().getConnectorMandateIdBuilder().setConnectorMandateId(mandateId);
        b.getAmountBuilder().setMinorAmount(minorAmount).setCurrency(currency(currencyCode));
        b.setOffSession(true);
        if (returnUrl != null) {
            b.setReturnUrl(returnUrl);
        }
        return b.build();
    }

    public static PaymentServiceTokenAuthorizeRequest tokenAuthorize(final String merchantTransactionId,
                                                                    final long minorAmount,
                                                                    final String currencyCode,
                                                                    final String connectorToken,
                                                                    final String captureMethod,
                                                                    @Nullable final String returnUrl) {
        final PaymentServiceTokenAuthorizeRequest.Builder b = PaymentServiceTokenAuthorizeRequest.newBuilder();
        b.setMerchantTransactionId(merchantTransactionId);
        b.getAmountBuilder().setMinorAmount(minorAmount).setCurrency(currency(currencyCode));
        b.getConnectorTokenBuilder().setValue(connectorToken);
        b.getAddressBuilder().getBillingAddressBuilder();
        b.setCaptureMethod(CaptureMethod.valueOf(captureMethod));
        if (returnUrl != null) {
            b.setReturnUrl(returnUrl);
        }
        return b.build();
    }

    // ---- webhooks (P3) -----------------------------------------------------------------------

    public static EventServiceHandleRequest handleEvent(final String merchantEventId,
                                                       final String uri,
                                                       final Map<String, String> headers,
                                                       final byte[] body,
                                                       @Nullable final String webhookSecret) {
        final EventServiceHandleRequest.Builder b = EventServiceHandleRequest.newBuilder();
        b.setMerchantEventId(merchantEventId);
        b.getRequestDetailsBuilder()
         .setMethod(HttpMethod.HTTP_METHOD_POST)
         .setUri(uri)
         .putAllHeaders(headers)
         .setBody(ByteString.copyFrom(body));
        if (webhookSecret != null && !webhookSecret.isEmpty()) {
            b.getWebhookSecretsBuilder().setSecret(webhookSecret);
        }
        return b.build();
    }

    // ---- helpers -----------------------------------------------------------------------------

    private static void setCard(final Object cardBuilder, final Card card) {
        // cardBuilder is types.PaymentMethods.CardDetails.Builder; set via SecretString sub-builders (stripe.kt style).
        final types.PaymentMethods.CardDetails.Builder cb = (types.PaymentMethods.CardDetails.Builder) cardBuilder;
        cb.getCardNumberBuilder().setValue(card.number);
        cb.getCardExpMonthBuilder().setValue(card.expMonth);
        cb.getCardExpYearBuilder().setValue(card.expYear);
        cb.getCardCvcBuilder().setValue(card.cvc);
        if (card.holderName != null) {
            cb.getCardHolderNameBuilder().setValue(card.holderName);
        }
    }

    private static Currency currency(final String currencyCode) {
        return Currency.valueOf(currencyCode);
    }
}
