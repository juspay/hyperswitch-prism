// AUTO-GENERATED — do not edit by hand.
// Source: services.proto ∩ bindings/uniffi.rs  |  Regenerate: make generate

import { UniffiClient as _UniffiClientBase } from "./uniffi_client";

export class UniffiClient extends _UniffiClientBase {
  /** Build connector HTTP request for accept flow. */
  acceptReq(
    requestBytes: Buffer | Uint8Array,
    optionsBytes: Buffer | Uint8Array
  ): Buffer {
    return this.callReq('accept', requestBytes, optionsBytes);
  }

  /** Parse connector HTTP response for accept flow. */
  acceptRes(
    responseBytes: Buffer | Uint8Array,
    requestBytes: Buffer | Uint8Array,
    optionsBytes: Buffer | Uint8Array
  ): Buffer {
    return this.callRes('accept', responseBytes, requestBytes, optionsBytes);
  }

  /** Build connector HTTP request for authenticate flow. */
  authenticateReq(
    requestBytes: Buffer | Uint8Array,
    optionsBytes: Buffer | Uint8Array
  ): Buffer {
    return this.callReq('authenticate', requestBytes, optionsBytes);
  }

  /** Parse connector HTTP response for authenticate flow. */
  authenticateRes(
    responseBytes: Buffer | Uint8Array,
    requestBytes: Buffer | Uint8Array,
    optionsBytes: Buffer | Uint8Array
  ): Buffer {
    return this.callRes('authenticate', responseBytes, requestBytes, optionsBytes);
  }

  /** Build connector HTTP request for authorize flow. */
  authorizeReq(
    requestBytes: Buffer | Uint8Array,
    optionsBytes: Buffer | Uint8Array
  ): Buffer {
    return this.callReq('authorize', requestBytes, optionsBytes);
  }

  /** Parse connector HTTP response for authorize flow. */
  authorizeRes(
    responseBytes: Buffer | Uint8Array,
    requestBytes: Buffer | Uint8Array,
    optionsBytes: Buffer | Uint8Array
  ): Buffer {
    return this.callRes('authorize', responseBytes, requestBytes, optionsBytes);
  }

  /** Build connector HTTP request for capture flow. */
  captureReq(
    requestBytes: Buffer | Uint8Array,
    optionsBytes: Buffer | Uint8Array
  ): Buffer {
    return this.callReq('capture', requestBytes, optionsBytes);
  }

  /** Parse connector HTTP response for capture flow. */
  captureRes(
    responseBytes: Buffer | Uint8Array,
    requestBytes: Buffer | Uint8Array,
    optionsBytes: Buffer | Uint8Array
  ): Buffer {
    return this.callRes('capture', responseBytes, requestBytes, optionsBytes);
  }

  /** Build connector HTTP request for charge flow. */
  chargeReq(
    requestBytes: Buffer | Uint8Array,
    optionsBytes: Buffer | Uint8Array
  ): Buffer {
    return this.callReq('charge', requestBytes, optionsBytes);
  }

  /** Parse connector HTTP response for charge flow. */
  chargeRes(
    responseBytes: Buffer | Uint8Array,
    requestBytes: Buffer | Uint8Array,
    optionsBytes: Buffer | Uint8Array
  ): Buffer {
    return this.callRes('charge', responseBytes, requestBytes, optionsBytes);
  }

  /** Build connector HTTP request for create flow. */
  createReq(
    requestBytes: Buffer | Uint8Array,
    optionsBytes: Buffer | Uint8Array
  ): Buffer {
    return this.callReq('create', requestBytes, optionsBytes);
  }

  /** Parse connector HTTP response for create flow. */
  createRes(
    responseBytes: Buffer | Uint8Array,
    requestBytes: Buffer | Uint8Array,
    optionsBytes: Buffer | Uint8Array
  ): Buffer {
    return this.callRes('create', responseBytes, requestBytes, optionsBytes);
  }

  /** Build connector HTTP request for create_client_authentication_token flow. */
  createClientAuthenticationTokenReq(
    requestBytes: Buffer | Uint8Array,
    optionsBytes: Buffer | Uint8Array
  ): Buffer {
    return this.callReq('create_client_authentication_token', requestBytes, optionsBytes);
  }

  /** Parse connector HTTP response for create_client_authentication_token flow. */
  createClientAuthenticationTokenRes(
    responseBytes: Buffer | Uint8Array,
    requestBytes: Buffer | Uint8Array,
    optionsBytes: Buffer | Uint8Array
  ): Buffer {
    return this.callRes('create_client_authentication_token', responseBytes, requestBytes, optionsBytes);
  }

  /** Build connector HTTP request for create_order flow. */
  createOrderReq(
    requestBytes: Buffer | Uint8Array,
    optionsBytes: Buffer | Uint8Array
  ): Buffer {
    return this.callReq('create_order', requestBytes, optionsBytes);
  }

  /** Parse connector HTTP response for create_order flow. */
  createOrderRes(
    responseBytes: Buffer | Uint8Array,
    requestBytes: Buffer | Uint8Array,
    optionsBytes: Buffer | Uint8Array
  ): Buffer {
    return this.callRes('create_order', responseBytes, requestBytes, optionsBytes);
  }

  /** Build connector HTTP request for create_server_authentication_token flow. */
  createServerAuthenticationTokenReq(
    requestBytes: Buffer | Uint8Array,
    optionsBytes: Buffer | Uint8Array
  ): Buffer {
    return this.callReq('create_server_authentication_token', requestBytes, optionsBytes);
  }

  /** Parse connector HTTP response for create_server_authentication_token flow. */
  createServerAuthenticationTokenRes(
    responseBytes: Buffer | Uint8Array,
    requestBytes: Buffer | Uint8Array,
    optionsBytes: Buffer | Uint8Array
  ): Buffer {
    return this.callRes('create_server_authentication_token', responseBytes, requestBytes, optionsBytes);
  }

  /** Build connector HTTP request for create_server_session_authentication_token flow. */
  createServerSessionAuthenticationTokenReq(
    requestBytes: Buffer | Uint8Array,
    optionsBytes: Buffer | Uint8Array
  ): Buffer {
    return this.callReq('create_server_session_authentication_token', requestBytes, optionsBytes);
  }

  /** Parse connector HTTP response for create_server_session_authentication_token flow. */
  createServerSessionAuthenticationTokenRes(
    responseBytes: Buffer | Uint8Array,
    requestBytes: Buffer | Uint8Array,
    optionsBytes: Buffer | Uint8Array
  ): Buffer {
    return this.callRes('create_server_session_authentication_token', responseBytes, requestBytes, optionsBytes);
  }

  /** Build connector HTTP request for defend flow. */
  defendReq(
    requestBytes: Buffer | Uint8Array,
    optionsBytes: Buffer | Uint8Array
  ): Buffer {
    return this.callReq('defend', requestBytes, optionsBytes);
  }

  /** Parse connector HTTP response for defend flow. */
  defendRes(
    responseBytes: Buffer | Uint8Array,
    requestBytes: Buffer | Uint8Array,
    optionsBytes: Buffer | Uint8Array
  ): Buffer {
    return this.callRes('defend', responseBytes, requestBytes, optionsBytes);
  }

  /** Build connector HTTP request for get flow. */
  getReq(
    requestBytes: Buffer | Uint8Array,
    optionsBytes: Buffer | Uint8Array
  ): Buffer {
    return this.callReq('get', requestBytes, optionsBytes);
  }

  /** Parse connector HTTP response for get flow. */
  getRes(
    responseBytes: Buffer | Uint8Array,
    requestBytes: Buffer | Uint8Array,
    optionsBytes: Buffer | Uint8Array
  ): Buffer {
    return this.callRes('get', responseBytes, requestBytes, optionsBytes);
  }

  /** Build connector HTTP request for incremental_authorization flow. */
  incrementalAuthorizationReq(
    requestBytes: Buffer | Uint8Array,
    optionsBytes: Buffer | Uint8Array
  ): Buffer {
    return this.callReq('incremental_authorization', requestBytes, optionsBytes);
  }

  /** Parse connector HTTP response for incremental_authorization flow. */
  incrementalAuthorizationRes(
    responseBytes: Buffer | Uint8Array,
    requestBytes: Buffer | Uint8Array,
    optionsBytes: Buffer | Uint8Array
  ): Buffer {
    return this.callRes('incremental_authorization', responseBytes, requestBytes, optionsBytes);
  }

  /** Build connector HTTP request for payout_create flow. */
  payoutCreateReq(
    requestBytes: Buffer | Uint8Array,
    optionsBytes: Buffer | Uint8Array
  ): Buffer {
    return this.callReq('payout_create', requestBytes, optionsBytes);
  }

  /** Parse connector HTTP response for payout_create flow. */
  payoutCreateRes(
    responseBytes: Buffer | Uint8Array,
    requestBytes: Buffer | Uint8Array,
    optionsBytes: Buffer | Uint8Array
  ): Buffer {
    return this.callRes('payout_create', responseBytes, requestBytes, optionsBytes);
  }

  /** Build connector HTTP request for payout_create_link flow. */
  payoutCreateLinkReq(
    requestBytes: Buffer | Uint8Array,
    optionsBytes: Buffer | Uint8Array
  ): Buffer {
    return this.callReq('payout_create_link', requestBytes, optionsBytes);
  }

  /** Parse connector HTTP response for payout_create_link flow. */
  payoutCreateLinkRes(
    responseBytes: Buffer | Uint8Array,
    requestBytes: Buffer | Uint8Array,
    optionsBytes: Buffer | Uint8Array
  ): Buffer {
    return this.callRes('payout_create_link', responseBytes, requestBytes, optionsBytes);
  }

  /** Build connector HTTP request for payout_create_recipient flow. */
  payoutCreateRecipientReq(
    requestBytes: Buffer | Uint8Array,
    optionsBytes: Buffer | Uint8Array
  ): Buffer {
    return this.callReq('payout_create_recipient', requestBytes, optionsBytes);
  }

  /** Parse connector HTTP response for payout_create_recipient flow. */
  payoutCreateRecipientRes(
    responseBytes: Buffer | Uint8Array,
    requestBytes: Buffer | Uint8Array,
    optionsBytes: Buffer | Uint8Array
  ): Buffer {
    return this.callRes('payout_create_recipient', responseBytes, requestBytes, optionsBytes);
  }

  /** Build connector HTTP request for payout_enroll_disburse_account flow. */
  payoutEnrollDisburseAccountReq(
    requestBytes: Buffer | Uint8Array,
    optionsBytes: Buffer | Uint8Array
  ): Buffer {
    return this.callReq('payout_enroll_disburse_account', requestBytes, optionsBytes);
  }

  /** Parse connector HTTP response for payout_enroll_disburse_account flow. */
  payoutEnrollDisburseAccountRes(
    responseBytes: Buffer | Uint8Array,
    requestBytes: Buffer | Uint8Array,
    optionsBytes: Buffer | Uint8Array
  ): Buffer {
    return this.callRes('payout_enroll_disburse_account', responseBytes, requestBytes, optionsBytes);
  }

  /** Build connector HTTP request for payout_get flow. */
  payoutGetReq(
    requestBytes: Buffer | Uint8Array,
    optionsBytes: Buffer | Uint8Array
  ): Buffer {
    return this.callReq('payout_get', requestBytes, optionsBytes);
  }

  /** Parse connector HTTP response for payout_get flow. */
  payoutGetRes(
    responseBytes: Buffer | Uint8Array,
    requestBytes: Buffer | Uint8Array,
    optionsBytes: Buffer | Uint8Array
  ): Buffer {
    return this.callRes('payout_get', responseBytes, requestBytes, optionsBytes);
  }

  /** Build connector HTTP request for payout_stage flow. */
  payoutStageReq(
    requestBytes: Buffer | Uint8Array,
    optionsBytes: Buffer | Uint8Array
  ): Buffer {
    return this.callReq('payout_stage', requestBytes, optionsBytes);
  }

  /** Parse connector HTTP response for payout_stage flow. */
  payoutStageRes(
    responseBytes: Buffer | Uint8Array,
    requestBytes: Buffer | Uint8Array,
    optionsBytes: Buffer | Uint8Array
  ): Buffer {
    return this.callRes('payout_stage', responseBytes, requestBytes, optionsBytes);
  }

  /** Build connector HTTP request for payout_transfer flow. */
  payoutTransferReq(
    requestBytes: Buffer | Uint8Array,
    optionsBytes: Buffer | Uint8Array
  ): Buffer {
    return this.callReq('payout_transfer', requestBytes, optionsBytes);
  }

  /** Parse connector HTTP response for payout_transfer flow. */
  payoutTransferRes(
    responseBytes: Buffer | Uint8Array,
    requestBytes: Buffer | Uint8Array,
    optionsBytes: Buffer | Uint8Array
  ): Buffer {
    return this.callRes('payout_transfer', responseBytes, requestBytes, optionsBytes);
  }

  /** Build connector HTTP request for payout_void flow. */
  payoutVoidReq(
    requestBytes: Buffer | Uint8Array,
    optionsBytes: Buffer | Uint8Array
  ): Buffer {
    return this.callReq('payout_void', requestBytes, optionsBytes);
  }

  /** Parse connector HTTP response for payout_void flow. */
  payoutVoidRes(
    responseBytes: Buffer | Uint8Array,
    requestBytes: Buffer | Uint8Array,
    optionsBytes: Buffer | Uint8Array
  ): Buffer {
    return this.callRes('payout_void', responseBytes, requestBytes, optionsBytes);
  }

  /** Build connector HTTP request for post_authenticate flow. */
  postAuthenticateReq(
    requestBytes: Buffer | Uint8Array,
    optionsBytes: Buffer | Uint8Array
  ): Buffer {
    return this.callReq('post_authenticate', requestBytes, optionsBytes);
  }

  /** Parse connector HTTP response for post_authenticate flow. */
  postAuthenticateRes(
    responseBytes: Buffer | Uint8Array,
    requestBytes: Buffer | Uint8Array,
    optionsBytes: Buffer | Uint8Array
  ): Buffer {
    return this.callRes('post_authenticate', responseBytes, requestBytes, optionsBytes);
  }

  /** Build connector HTTP request for pre_authenticate flow. */
  preAuthenticateReq(
    requestBytes: Buffer | Uint8Array,
    optionsBytes: Buffer | Uint8Array
  ): Buffer {
    return this.callReq('pre_authenticate', requestBytes, optionsBytes);
  }

  /** Parse connector HTTP response for pre_authenticate flow. */
  preAuthenticateRes(
    responseBytes: Buffer | Uint8Array,
    requestBytes: Buffer | Uint8Array,
    optionsBytes: Buffer | Uint8Array
  ): Buffer {
    return this.callRes('pre_authenticate', responseBytes, requestBytes, optionsBytes);
  }

  /** Build connector HTTP request for proxy_authorize flow. */
  proxyAuthorizeReq(
    requestBytes: Buffer | Uint8Array,
    optionsBytes: Buffer | Uint8Array
  ): Buffer {
    return this.callReq('proxy_authorize', requestBytes, optionsBytes);
  }

  /** Parse connector HTTP response for proxy_authorize flow. */
  proxyAuthorizeRes(
    responseBytes: Buffer | Uint8Array,
    requestBytes: Buffer | Uint8Array,
    optionsBytes: Buffer | Uint8Array
  ): Buffer {
    return this.callRes('proxy_authorize', responseBytes, requestBytes, optionsBytes);
  }

  /** Build connector HTTP request for proxy_setup_recurring flow. */
  proxySetupRecurringReq(
    requestBytes: Buffer | Uint8Array,
    optionsBytes: Buffer | Uint8Array
  ): Buffer {
    return this.callReq('proxy_setup_recurring', requestBytes, optionsBytes);
  }

  /** Parse connector HTTP response for proxy_setup_recurring flow. */
  proxySetupRecurringRes(
    responseBytes: Buffer | Uint8Array,
    requestBytes: Buffer | Uint8Array,
    optionsBytes: Buffer | Uint8Array
  ): Buffer {
    return this.callRes('proxy_setup_recurring', responseBytes, requestBytes, optionsBytes);
  }

  /** Build connector HTTP request for recurring_revoke flow. */
  recurringRevokeReq(
    requestBytes: Buffer | Uint8Array,
    optionsBytes: Buffer | Uint8Array
  ): Buffer {
    return this.callReq('recurring_revoke', requestBytes, optionsBytes);
  }

  /** Parse connector HTTP response for recurring_revoke flow. */
  recurringRevokeRes(
    responseBytes: Buffer | Uint8Array,
    requestBytes: Buffer | Uint8Array,
    optionsBytes: Buffer | Uint8Array
  ): Buffer {
    return this.callRes('recurring_revoke', responseBytes, requestBytes, optionsBytes);
  }

  /** Build connector HTTP request for refund flow. */
  refundReq(
    requestBytes: Buffer | Uint8Array,
    optionsBytes: Buffer | Uint8Array
  ): Buffer {
    return this.callReq('refund', requestBytes, optionsBytes);
  }

  /** Parse connector HTTP response for refund flow. */
  refundRes(
    responseBytes: Buffer | Uint8Array,
    requestBytes: Buffer | Uint8Array,
    optionsBytes: Buffer | Uint8Array
  ): Buffer {
    return this.callRes('refund', responseBytes, requestBytes, optionsBytes);
  }

  /** Build connector HTTP request for refund_get flow. */
  refundGetReq(
    requestBytes: Buffer | Uint8Array,
    optionsBytes: Buffer | Uint8Array
  ): Buffer {
    return this.callReq('refund_get', requestBytes, optionsBytes);
  }

  /** Parse connector HTTP response for refund_get flow. */
  refundGetRes(
    responseBytes: Buffer | Uint8Array,
    requestBytes: Buffer | Uint8Array,
    optionsBytes: Buffer | Uint8Array
  ): Buffer {
    return this.callRes('refund_get', responseBytes, requestBytes, optionsBytes);
  }

  /** Build connector HTTP request for reverse flow. */
  reverseReq(
    requestBytes: Buffer | Uint8Array,
    optionsBytes: Buffer | Uint8Array
  ): Buffer {
    return this.callReq('reverse', requestBytes, optionsBytes);
  }

  /** Parse connector HTTP response for reverse flow. */
  reverseRes(
    responseBytes: Buffer | Uint8Array,
    requestBytes: Buffer | Uint8Array,
    optionsBytes: Buffer | Uint8Array
  ): Buffer {
    return this.callRes('reverse', responseBytes, requestBytes, optionsBytes);
  }

  /** Build connector HTTP request for setup_recurring flow. */
  setupRecurringReq(
    requestBytes: Buffer | Uint8Array,
    optionsBytes: Buffer | Uint8Array
  ): Buffer {
    return this.callReq('setup_recurring', requestBytes, optionsBytes);
  }

  /** Parse connector HTTP response for setup_recurring flow. */
  setupRecurringRes(
    responseBytes: Buffer | Uint8Array,
    requestBytes: Buffer | Uint8Array,
    optionsBytes: Buffer | Uint8Array
  ): Buffer {
    return this.callRes('setup_recurring', responseBytes, requestBytes, optionsBytes);
  }

  /** Build connector HTTP request for submit_evidence flow. */
  submitEvidenceReq(
    requestBytes: Buffer | Uint8Array,
    optionsBytes: Buffer | Uint8Array
  ): Buffer {
    return this.callReq('submit_evidence', requestBytes, optionsBytes);
  }

  /** Parse connector HTTP response for submit_evidence flow. */
  submitEvidenceRes(
    responseBytes: Buffer | Uint8Array,
    requestBytes: Buffer | Uint8Array,
    optionsBytes: Buffer | Uint8Array
  ): Buffer {
    return this.callRes('submit_evidence', responseBytes, requestBytes, optionsBytes);
  }

  /** Build connector HTTP request for token_authorize flow. */
  tokenAuthorizeReq(
    requestBytes: Buffer | Uint8Array,
    optionsBytes: Buffer | Uint8Array
  ): Buffer {
    return this.callReq('token_authorize', requestBytes, optionsBytes);
  }

  /** Parse connector HTTP response for token_authorize flow. */
  tokenAuthorizeRes(
    responseBytes: Buffer | Uint8Array,
    requestBytes: Buffer | Uint8Array,
    optionsBytes: Buffer | Uint8Array
  ): Buffer {
    return this.callRes('token_authorize', responseBytes, requestBytes, optionsBytes);
  }

  /** Build connector HTTP request for token_setup_recurring flow. */
  tokenSetupRecurringReq(
    requestBytes: Buffer | Uint8Array,
    optionsBytes: Buffer | Uint8Array
  ): Buffer {
    return this.callReq('token_setup_recurring', requestBytes, optionsBytes);
  }

  /** Parse connector HTTP response for token_setup_recurring flow. */
  tokenSetupRecurringRes(
    responseBytes: Buffer | Uint8Array,
    requestBytes: Buffer | Uint8Array,
    optionsBytes: Buffer | Uint8Array
  ): Buffer {
    return this.callRes('token_setup_recurring', responseBytes, requestBytes, optionsBytes);
  }

  /** Build connector HTTP request for tokenize flow. */
  tokenizeReq(
    requestBytes: Buffer | Uint8Array,
    optionsBytes: Buffer | Uint8Array
  ): Buffer {
    return this.callReq('tokenize', requestBytes, optionsBytes);
  }

  /** Parse connector HTTP response for tokenize flow. */
  tokenizeRes(
    responseBytes: Buffer | Uint8Array,
    requestBytes: Buffer | Uint8Array,
    optionsBytes: Buffer | Uint8Array
  ): Buffer {
    return this.callRes('tokenize', responseBytes, requestBytes, optionsBytes);
  }

  /** Build connector HTTP request for void flow. */
  voidReq(
    requestBytes: Buffer | Uint8Array,
    optionsBytes: Buffer | Uint8Array
  ): Buffer {
    return this.callReq('void', requestBytes, optionsBytes);
  }

  /** Parse connector HTTP response for void flow. */
  voidRes(
    responseBytes: Buffer | Uint8Array,
    requestBytes: Buffer | Uint8Array,
    optionsBytes: Buffer | Uint8Array
  ): Buffer {
    return this.callRes('void', responseBytes, requestBytes, optionsBytes);
  }

  /** Direct single-step transform for handle_event (no HTTP round-trip). */
  handleEventDirect(
    requestBytes: Buffer | Uint8Array,
    optionsBytes: Buffer | Uint8Array
  ): Buffer {
    return this.callDirect('handle_event', requestBytes, optionsBytes);
  }

  /** Direct single-step transform for parse_event (no HTTP round-trip). */
  parseEventDirect(
    requestBytes: Buffer | Uint8Array,
    optionsBytes: Buffer | Uint8Array
  ): Buffer {
    return this.callDirect('parse_event', requestBytes, optionsBytes);
  }

  /** Direct single-step transform for verify_redirect_response (no HTTP round-trip). */
  verifyRedirectResponseDirect(
    requestBytes: Buffer | Uint8Array,
    optionsBytes: Buffer | Uint8Array
  ): Buffer {
    return this.callDirect('verify_redirect_response', requestBytes, optionsBytes);
  }

}
