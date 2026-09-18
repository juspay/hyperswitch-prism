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

// NOTE (P0 verification): the Prism SDK proto types below use the in-repo Gradle package layout
// (`types.Payment.*`), which is the source of truth in this repository. The published io.hyperswitch:prism:0.0.6
// artifact MAY expose them under `com.hyperswitch.payments.*` (per the SDK README). Reconcile these imports
// against the actual artifact when the module is first compiled during the P0 spike.
import types.Payment.EventServiceHandleRequest;
import types.Payment.EventServiceHandleResponse;
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

import types.Payment.PaymentServiceVoidRequest;
import types.Payment.PaymentServiceVoidResponse;
import types.Payment.RecurringPaymentServiceChargeRequest;
import types.Payment.RecurringPaymentServiceChargeResponse;
import types.Payment.RefundResponse;
import types.Payment.RefundServiceGetRequest;

/**
 * Transport abstraction over Prism. The primary implementation ({@link SdkPrismClient}) embeds the
 * {@code io.hyperswitch:prism} Java SDK in-process (JNA → native FFI), exactly like the Medusa plugin embeds
 * the Node SDK. Keeping this an interface means a sidecar transport (Prism {@code grpc-server} with
 * {@code type="http"}) could be dropped in behind the same contract without touching the payment-plugin logic,
 * if the OSGi packaging of the native SDK ever proves unworkable.
 *
 * <p>Each method takes the KillBill tenant id so the implementation can resolve that tenant's connector
 * credentials ({@code ConnectorConfig}). All Prism calls throw {@link PrismClientException} on failure.
 */
public interface PrismClient {

    PaymentServiceAuthorizeResponse authorize(UUID kbTenantId, PaymentServiceAuthorizeRequest request) throws PrismClientException;

    PaymentServiceCaptureResponse capture(UUID kbTenantId, PaymentServiceCaptureRequest request) throws PrismClientException;

    PaymentServiceVoidResponse voidPayment(UUID kbTenantId, PaymentServiceVoidRequest request) throws PrismClientException;

    /** Payment sync (PSync). */
    PaymentServiceGetResponse get(UUID kbTenantId, PaymentServiceGetRequest request) throws PrismClientException;

    RefundResponse refund(UUID kbTenantId, PaymentServiceRefundRequest request) throws PrismClientException;

    /** Refund sync (RSync). */
    RefundResponse refundGet(UUID kbTenantId, RefundServiceGetRequest request) throws PrismClientException;

    /** Create a customer record at the connector (required by e.g. Stripe before a mandate can be reused). */
    types.Payment.CustomerServiceCreateResponse customerCreate(UUID kbTenantId, types.Payment.CustomerServiceCreateRequest request) throws PrismClientException;

    /** Mandate setup (customer present). */
    PaymentServiceSetupRecurringResponse setupRecurring(UUID kbTenantId, PaymentServiceSetupRecurringRequest request) throws PrismClientException;

    /** Merchant-initiated charge against a stored mandate (customer NOT present). */
    RecurringPaymentServiceChargeResponse charge(UUID kbTenantId, RecurringPaymentServiceChargeRequest request) throws PrismClientException;

    /** Payment against a stored connector token (customer NOT present). */
    PaymentServiceAuthorizeResponse tokenAuthorize(UUID kbTenantId, PaymentServiceTokenAuthorizeRequest request) throws PrismClientException;

    /** Tokenize a payment method for future reuse. */
    PaymentMethodServiceTokenizeResponse tokenize(UUID kbTenantId, PaymentMethodServiceTokenizeRequest request) throws PrismClientException;

    /** Verify + parse an incoming connector webhook. */
    EventServiceHandleResponse handleEvent(UUID kbTenantId, EventServiceHandleRequest request) throws PrismClientException;
}
