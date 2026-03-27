# AUTO-GENERATED — do not edit by hand.
# Source: services.proto  |  Regenerate: make generate  (or: python3 scripts/generators/code/generate.py --lang python)

from __future__ import annotations

import ctypes
import json
import os
import platform
import sys
from dataclasses import dataclass, field
from typing import Any, Dict, Optional

from payments.generated import payment_pb2

# ── Config ────────────────────────────────────────────────────────────────────


@dataclass
class GrpcConfig:
    """Connection configuration for the gRPC client.

    Field names are snake_case — they are serialised to JSON and sent to the
    Rust FFI layer which deserialises them into GrpcConfigInput.
    
    The connector_config field should contain the connector-specific authentication
    and configuration in the format expected by the server:
    {"config": {"ConnectorName": {"api_key": "...", ...}}}
    """

    endpoint: str
    connector: str
    connector_config: Dict[str, Any]

    def to_json_bytes(self) -> bytes:
        d: dict = {
            "endpoint": self.endpoint,
            "connector": self.connector,
            "connector_config": self.connector_config,
        }
        return json.dumps(d).encode()


# ── ctypes FFI bindings ───────────────────────────────────────────────────────


class _GrpcFfi:
    """Thin ctypes wrapper around libhyperswitch_grpc_ffi."""

    def __init__(self, lib_path: Optional[str] = None) -> None:
        if lib_path is None:
            here = os.path.dirname(os.path.abspath(__file__))
            ext  = "dylib" if platform.system() == "Darwin" else "so"
            lib_path = os.path.join(here, "generated", f"libhyperswitch_grpc_ffi.{ext}")

        lib = ctypes.CDLL(lib_path)

        # uint8_t *hyperswitch_grpc_call(
        #     const char   *method,
        #     const uint8_t *config_ptr, uint32_t config_len,
        #     const uint8_t *req_ptr,    uint32_t req_len,
        #     uint32_t      *out_len,
        # );
        self._call = lib.hyperswitch_grpc_call
        self._call.restype  = ctypes.POINTER(ctypes.c_uint8)
        self._call.argtypes = [
            ctypes.c_char_p,                         # method
            ctypes.POINTER(ctypes.c_uint8),          # config_ptr
            ctypes.c_uint32,                         # config_len
            ctypes.POINTER(ctypes.c_uint8),          # req_ptr
            ctypes.c_uint32,                         # req_len
            ctypes.POINTER(ctypes.c_uint32),         # out_len  (written by callee)
        ]

        # void hyperswitch_grpc_free(uint8_t *ptr, uint32_t len);
        self._free = lib.hyperswitch_grpc_free
        self._free.restype  = None
        self._free.argtypes = [ctypes.POINTER(ctypes.c_uint8), ctypes.c_uint32]

    def call(self, method: str, config_bytes: bytes, req_bytes: bytes) -> bytes:
        config_arr = (ctypes.c_uint8 * len(config_bytes)).from_buffer_copy(config_bytes)
        req_arr    = (ctypes.c_uint8 * len(req_bytes)).from_buffer_copy(req_bytes)
        out_len    = ctypes.c_uint32(0)

        ptr = self._call(
            method.encode(),
            config_arr, len(config_bytes),
            req_arr,    len(req_bytes),
            ctypes.byref(out_len),
        )
        length = out_len.value
        raw = bytes(ptr[:length])
        self._free(ptr, length)
        return raw


# ── Dispatch helper ───────────────────────────────────────────────────────────


def _call_grpc(ffi: _GrpcFfi, config: GrpcConfig, method: str, req, res_cls):
    """Serialize req, call FFI, parse response or raise on error."""
    config_bytes = config.to_json_bytes()
    req_bytes    = req.SerializeToString()
    raw          = ffi.call(method, config_bytes, req_bytes)

    if not raw:
        raise RuntimeError(f"gRPC error ({method}): empty response from FFI")

    if raw[0] == 1:  # error flag
        raise RuntimeError(f"gRPC error ({method}): {raw[1:].decode('utf-8', errors='replace')}")

    res = res_cls()
    res.ParseFromString(raw[1:])
    return res


# ── Sub-clients (one per proto service) ──────────────────────────────────────

class GrpcCustomerClient:
    """CustomerService — gRPC sub-client."""

    def __init__(self, ffi: _GrpcFfi, config: GrpcConfig) -> None:
        self._ffi    = ffi
        self._config = config

    def create(self, req: payment_pb2.CustomerServiceCreateRequest) -> payment_pb2.CustomerServiceCreateResponse:
        """CustomerService.Create — Create customer record in the payment processor system. Stores customer details for future payment operations without re-sending personal information."""
        return _call_grpc(
            self._ffi, self._config,
            "customer/create",
            req, payment_pb2.CustomerServiceCreateResponse,
        )


class GrpcDisputeClient:
    """DisputeService — gRPC sub-client."""

    def __init__(self, ffi: _GrpcFfi, config: GrpcConfig) -> None:
        self._ffi    = ffi
        self._config = config

    def submit_evidence(self, req: payment_pb2.DisputeServiceSubmitEvidenceRequest) -> payment_pb2.DisputeServiceSubmitEvidenceResponse:
        """DisputeService.SubmitEvidence — Upload evidence to dispute customer chargeback. Provides documentation like receipts and delivery proof to contest fraudulent transaction claims."""
        return _call_grpc(
            self._ffi, self._config,
            "dispute/submit_evidence",
            req, payment_pb2.DisputeServiceSubmitEvidenceResponse,
        )

    def defend(self, req: payment_pb2.DisputeServiceDefendRequest) -> payment_pb2.DisputeServiceDefendResponse:
        """DisputeService.Defend — Submit defense with reason code for dispute. Presents formal argument against customer's chargeback claim with supporting documentation."""
        return _call_grpc(
            self._ffi, self._config,
            "dispute/defend",
            req, payment_pb2.DisputeServiceDefendResponse,
        )

    def accept(self, req: payment_pb2.DisputeServiceAcceptRequest) -> payment_pb2.DisputeServiceAcceptResponse:
        """DisputeService.Accept — Concede dispute and accepts chargeback loss. Acknowledges liability and stops dispute defense process when evidence is insufficient."""
        return _call_grpc(
            self._ffi, self._config,
            "dispute/accept",
            req, payment_pb2.DisputeServiceAcceptResponse,
        )


class GrpcEventClient:
    """EventService — gRPC sub-client."""

    def __init__(self, ffi: _GrpcFfi, config: GrpcConfig) -> None:
        self._ffi    = ffi
        self._config = config

    def handle_event(self, req: payment_pb2.EventServiceHandleRequest) -> payment_pb2.EventServiceHandleResponse:
        """EventService.HandleEvent — Process webhook notifications from connectors. Translates connector events into standardized responses for asynchronous payment state updates."""
        return _call_grpc(
            self._ffi, self._config,
            "event/handle_event",
            req, payment_pb2.EventServiceHandleResponse,
        )


class GrpcMerchantAuthenticationClient:
    """MerchantAuthenticationService — gRPC sub-client."""

    def __init__(self, ffi: _GrpcFfi, config: GrpcConfig) -> None:
        self._ffi    = ffi
        self._config = config

    def create_access_token(self, req: payment_pb2.MerchantAuthenticationServiceCreateAccessTokenRequest) -> payment_pb2.MerchantAuthenticationServiceCreateAccessTokenResponse:
        """MerchantAuthenticationService.CreateAccessToken — Generate short-lived connector authentication token. Provides secure credentials for connector API access without storing secrets client-side."""
        return _call_grpc(
            self._ffi, self._config,
            "merchant_authentication/create_access_token",
            req, payment_pb2.MerchantAuthenticationServiceCreateAccessTokenResponse,
        )

    def create_session_token(self, req: payment_pb2.MerchantAuthenticationServiceCreateSessionTokenRequest) -> payment_pb2.MerchantAuthenticationServiceCreateSessionTokenResponse:
        """MerchantAuthenticationService.CreateSessionToken — Create session token for payment processing. Maintains session state across multiple payment operations for improved security and tracking."""
        return _call_grpc(
            self._ffi, self._config,
            "merchant_authentication/create_session_token",
            req, payment_pb2.MerchantAuthenticationServiceCreateSessionTokenResponse,
        )

    def create_sdk_session_token(self, req: payment_pb2.MerchantAuthenticationServiceCreateSdkSessionTokenRequest) -> payment_pb2.MerchantAuthenticationServiceCreateSdkSessionTokenResponse:
        """MerchantAuthenticationService.CreateSdkSessionToken — Initialize wallet payment sessions for Apple Pay, Google Pay, etc. Sets up secure context for tokenized wallet payments with device verification."""
        return _call_grpc(
            self._ffi, self._config,
            "merchant_authentication/create_sdk_session_token",
            req, payment_pb2.MerchantAuthenticationServiceCreateSdkSessionTokenResponse,
        )


class GrpcPaymentMethodAuthenticationClient:
    """PaymentMethodAuthenticationService — gRPC sub-client."""

    def __init__(self, ffi: _GrpcFfi, config: GrpcConfig) -> None:
        self._ffi    = ffi
        self._config = config

    def pre_authenticate(self, req: payment_pb2.PaymentMethodAuthenticationServicePreAuthenticateRequest) -> payment_pb2.PaymentMethodAuthenticationServicePreAuthenticateResponse:
        """PaymentMethodAuthenticationService.PreAuthenticate — Initiate 3DS flow before payment authorization. Collects device data and prepares authentication context for frictionless or challenge-based verification."""
        return _call_grpc(
            self._ffi, self._config,
            "payment_method_authentication/pre_authenticate",
            req, payment_pb2.PaymentMethodAuthenticationServicePreAuthenticateResponse,
        )

    def authenticate(self, req: payment_pb2.PaymentMethodAuthenticationServiceAuthenticateRequest) -> payment_pb2.PaymentMethodAuthenticationServiceAuthenticateResponse:
        """PaymentMethodAuthenticationService.Authenticate — Execute 3DS challenge or frictionless verification. Authenticates customer via bank challenge or behind-the-scenes verification for fraud prevention."""
        return _call_grpc(
            self._ffi, self._config,
            "payment_method_authentication/authenticate",
            req, payment_pb2.PaymentMethodAuthenticationServiceAuthenticateResponse,
        )

    def post_authenticate(self, req: payment_pb2.PaymentMethodAuthenticationServicePostAuthenticateRequest) -> payment_pb2.PaymentMethodAuthenticationServicePostAuthenticateResponse:
        """PaymentMethodAuthenticationService.PostAuthenticate — Validate authentication results with the issuing bank. Processes bank's authentication decision to determine if payment can proceed."""
        return _call_grpc(
            self._ffi, self._config,
            "payment_method_authentication/post_authenticate",
            req, payment_pb2.PaymentMethodAuthenticationServicePostAuthenticateResponse,
        )


class GrpcPaymentMethodClient:
    """PaymentMethodService — gRPC sub-client."""

    def __init__(self, ffi: _GrpcFfi, config: GrpcConfig) -> None:
        self._ffi    = ffi
        self._config = config

    def tokenize(self, req: payment_pb2.PaymentMethodServiceTokenizeRequest) -> payment_pb2.PaymentMethodServiceTokenizeResponse:
        """PaymentMethodService.Tokenize — Tokenize payment method for secure storage. Replaces raw card details with secure token for one-click payments and recurring billing."""
        return _call_grpc(
            self._ffi, self._config,
            "payment_method/tokenize",
            req, payment_pb2.PaymentMethodServiceTokenizeResponse,
        )

    def eligibility(self, req: payment_pb2.PayoutMethodEligibilityRequest) -> payment_pb2.PayoutMethodEligibilityResponse:
        """PaymentMethodService.Eligibility — Check if the payout method is eligible for the transaction"""
        return _call_grpc(
            self._ffi, self._config,
            "payment_method/eligibility",
            req, payment_pb2.PayoutMethodEligibilityResponse,
        )


class GrpcPaymentClient:
    """PaymentService — gRPC sub-client."""

    def __init__(self, ffi: _GrpcFfi, config: GrpcConfig) -> None:
        self._ffi    = ffi
        self._config = config

    def authorize(self, req: payment_pb2.PaymentServiceAuthorizeRequest) -> payment_pb2.PaymentServiceAuthorizeResponse:
        """PaymentService.Authorize — Authorize a payment amount on a payment method. This reserves funds without capturing them, essential for verifying availability before finalizing."""
        return _call_grpc(
            self._ffi, self._config,
            "payment/authorize",
            req, payment_pb2.PaymentServiceAuthorizeResponse,
        )

    def get(self, req: payment_pb2.PaymentServiceGetRequest) -> payment_pb2.PaymentServiceGetResponse:
        """PaymentService.Get — Retrieve current payment status from the payment processor. Enables synchronization between your system and payment processors for accurate state tracking."""
        return _call_grpc(
            self._ffi, self._config,
            "payment/get",
            req, payment_pb2.PaymentServiceGetResponse,
        )

    def void(self, req: payment_pb2.PaymentServiceVoidRequest) -> payment_pb2.PaymentServiceVoidResponse:
        """PaymentService.Void — Cancel an authorized payment before capture. Releases held funds back to customer, typically used when orders are cancelled or abandoned."""
        return _call_grpc(
            self._ffi, self._config,
            "payment/void",
            req, payment_pb2.PaymentServiceVoidResponse,
        )

    def reverse(self, req: payment_pb2.PaymentServiceReverseRequest) -> payment_pb2.PaymentServiceReverseResponse:
        """PaymentService.Reverse — Reverse a captured payment before settlement. Recovers funds after capture but before bank settlement, used for corrections or cancellations."""
        return _call_grpc(
            self._ffi, self._config,
            "payment/reverse",
            req, payment_pb2.PaymentServiceReverseResponse,
        )

    def capture(self, req: payment_pb2.PaymentServiceCaptureRequest) -> payment_pb2.PaymentServiceCaptureResponse:
        """PaymentService.Capture — Finalize an authorized payment transaction. Transfers reserved funds from customer to merchant account, completing the payment lifecycle."""
        return _call_grpc(
            self._ffi, self._config,
            "payment/capture",
            req, payment_pb2.PaymentServiceCaptureResponse,
        )

    def create_order(self, req: payment_pb2.PaymentServiceCreateOrderRequest) -> payment_pb2.PaymentServiceCreateOrderResponse:
        """PaymentService.CreateOrder — Initialize an order in the payment processor system. Sets up payment context before customer enters card details for improved authorization rates."""
        return _call_grpc(
            self._ffi, self._config,
            "payment/create_order",
            req, payment_pb2.PaymentServiceCreateOrderResponse,
        )

    def refund(self, req: payment_pb2.PaymentServiceRefundRequest) -> payment_pb2.RefundResponse:
        """PaymentService.Refund — Initiate a refund to customer's payment method. Returns funds for returns, cancellations, or service adjustments after original payment."""
        return _call_grpc(
            self._ffi, self._config,
            "payment/refund",
            req, payment_pb2.RefundResponse,
        )

    def incremental_authorization(self, req: payment_pb2.PaymentServiceIncrementalAuthorizationRequest) -> payment_pb2.PaymentServiceIncrementalAuthorizationResponse:
        """PaymentService.IncrementalAuthorization — Increase authorized amount if still in authorized state. Allows adding charges to existing authorization for hospitality, tips, or incremental services."""
        return _call_grpc(
            self._ffi, self._config,
            "payment/incremental_authorization",
            req, payment_pb2.PaymentServiceIncrementalAuthorizationResponse,
        )

    def verify_redirect_response(self, req: payment_pb2.PaymentServiceVerifyRedirectResponseRequest) -> payment_pb2.PaymentServiceVerifyRedirectResponseResponse:
        """PaymentService.VerifyRedirectResponse — Validate redirect-based payment responses. Confirms authenticity of redirect-based payment completions to prevent fraud and tampering."""
        return _call_grpc(
            self._ffi, self._config,
            "payment/verify_redirect_response",
            req, payment_pb2.PaymentServiceVerifyRedirectResponseResponse,
        )

    def setup_recurring(self, req: payment_pb2.PaymentServiceSetupRecurringRequest) -> payment_pb2.PaymentServiceSetupRecurringResponse:
        """PaymentService.SetupRecurring — Setup a recurring payment instruction for future payments/ debits. This could be for SaaS subscriptions, monthly bill payments, insurance payments and similar use cases."""
        return _call_grpc(
            self._ffi, self._config,
            "payment/setup_recurring",
            req, payment_pb2.PaymentServiceSetupRecurringResponse,
        )


class GrpcPayoutClient:
    """PayoutService — gRPC sub-client."""

    def __init__(self, ffi: _GrpcFfi, config: GrpcConfig) -> None:
        self._ffi    = ffi
        self._config = config

    def transfer(self, req: payment_pb2.PayoutServiceTransferRequest) -> payment_pb2.PayoutServiceTransferResponse:
        """PayoutService.Transfer — Creates a payout fund transfer."""
        return _call_grpc(
            self._ffi, self._config,
            "payout/transfer",
            req, payment_pb2.PayoutServiceTransferResponse,
        )

    def stage(self, req: payment_pb2.PayoutServiceStageRequest) -> payment_pb2.PayoutServiceStageResponse:
        """PayoutService.Stage — Stage the payout."""
        return _call_grpc(
            self._ffi, self._config,
            "payout/stage",
            req, payment_pb2.PayoutServiceStageResponse,
        )

    def create_link(self, req: payment_pb2.PayoutServiceCreateLinkRequest) -> payment_pb2.PayoutServiceCreateLinkResponse:
        """PayoutService.CreateLink — Creates a link between the recipient and the payout."""
        return _call_grpc(
            self._ffi, self._config,
            "payout/create_link",
            req, payment_pb2.PayoutServiceCreateLinkResponse,
        )

    def create_recipient(self, req: payment_pb2.PayoutServiceCreateRecipientRequest) -> payment_pb2.PayoutServiceCreateRecipientResponse:
        """PayoutService.CreateRecipient — Create payout recipient."""
        return _call_grpc(
            self._ffi, self._config,
            "payout/create_recipient",
            req, payment_pb2.PayoutServiceCreateRecipientResponse,
        )

    def enroll_disburse_account(self, req: payment_pb2.PayoutServiceEnrollDisburseAccountRequest) -> payment_pb2.PayoutServiceEnrollDisburseAccountResponse:
        """PayoutService.EnrollDisburseAccount — Enroll disburse account."""
        return _call_grpc(
            self._ffi, self._config,
            "payout/enroll_disburse_account",
            req, payment_pb2.PayoutServiceEnrollDisburseAccountResponse,
        )


class GrpcRecurringPaymentClient:
    """RecurringPaymentService — gRPC sub-client."""

    def __init__(self, ffi: _GrpcFfi, config: GrpcConfig) -> None:
        self._ffi    = ffi
        self._config = config

    def charge(self, req: payment_pb2.RecurringPaymentServiceChargeRequest) -> payment_pb2.RecurringPaymentServiceChargeResponse:
        """RecurringPaymentService.Charge — Charge using an existing stored recurring payment instruction. Processes repeat payments for subscriptions or recurring billing without collecting payment details."""
        return _call_grpc(
            self._ffi, self._config,
            "recurring_payment/charge",
            req, payment_pb2.RecurringPaymentServiceChargeResponse,
        )

    def revoke(self, req: payment_pb2.RecurringPaymentServiceRevokeRequest) -> payment_pb2.RecurringPaymentServiceRevokeResponse:
        """RecurringPaymentService.Revoke — Cancel an existing recurring payment mandate. Stops future automatic charges on customer's stored consent for subscription cancellations."""
        return _call_grpc(
            self._ffi, self._config,
            "recurring_payment/revoke",
            req, payment_pb2.RecurringPaymentServiceRevokeResponse,
        )


# ── Top-level GrpcClient ──────────────────────────────────────────────────────


class GrpcClient:
    """Top-level gRPC client for the connector-service.

    Each sub-client corresponds to one proto service.  Auth headers from
    ``GrpcConfig`` are injected automatically on every call via the Rust FFI layer.

    Example::

        client = GrpcClient(GrpcConfig(
            endpoint  = "http://localhost:8000",
            connector = "stripe",
            connector_config = {"config": {"Stripe": {"api_key": "sk_test_..."}}},
        ))
        res = client.customer.create(...)
        res = client.dispute.submit_evidence(...)
        res = client.event.handle_event(...)
        res = client.merchant_authentication.create_access_token(...)
    """

    customer: GrpcCustomerClient
    dispute: GrpcDisputeClient
    event: GrpcEventClient
    merchant_authentication: GrpcMerchantAuthenticationClient
    payment_method_authentication: GrpcPaymentMethodAuthenticationClient
    payment_method: GrpcPaymentMethodClient
    payment: GrpcPaymentClient
    payout: GrpcPayoutClient
    recurring_payment: GrpcRecurringPaymentClient

    def __init__(self, config: GrpcConfig, lib_path: Optional[str] = None) -> None:
        ffi = _GrpcFfi(lib_path)
        self.customer = GrpcCustomerClient(ffi, config)
        self.dispute = GrpcDisputeClient(ffi, config)
        self.event = GrpcEventClient(ffi, config)
        self.merchant_authentication = GrpcMerchantAuthenticationClient(ffi, config)
        self.payment_method_authentication = GrpcPaymentMethodAuthenticationClient(ffi, config)
        self.payment_method = GrpcPaymentMethodClient(ffi, config)
        self.payment = GrpcPaymentClient(ffi, config)
        self.payout = GrpcPayoutClient(ffi, config)
        self.recurring_payment = GrpcRecurringPaymentClient(ffi, config)
