//! # Unsupported features
//!
//! The following Prism flows / payment methods are intentionally NOT supported
//! by this connector because CryptoPay does not offer them (or Prism's abstraction
//! does not cleanly map to what CryptoPay does offer). Each entry is derived from
//! `grace/workflow-v2/runs/cryptopay/2026-04-21/capability_matrix.json`.
//!
//! Prism's trait system returns `ConnectorError::NotSupported` for these paths
//! by default; this block exists purely for discoverability.
//!
//! ## Flows
//!
//! - `Accept` — Crypto payments cannot be charged back; no dispute API. Evidence: https://developers.cryptopay.me/
//! - `Authenticate` — No 3DS (crypto, not card). Evidence: https://developers.cryptopay.me/
//! - `Capture` — Crypto invoices are auto-captured on transaction confirmation; no manual capture endpoint. Evidence: https://developers.cryptopay.me/guides/invoices
//! - `ClientAuthenticationToken` — HMAC per-request auth only. Evidence: https://developers.cryptopay.me/guides/api-basics/authentication
//! - `CreateConnectorCustomer` — No customer-object endpoint; a customer is identified by custom_id on the invoice. Evidence: https://developers.cryptopay.me/
//! - `CreateOrder` — CryptoPay invoices don't require a separate CreateOrder — the Authorize (POST /api/invoices) creates the invoice dire... Evidence: https://developers.cryptopay.me/guides/invoices
//! - `DefendDispute` — No dispute management endpoints. Evidence: https://developers.cryptopay.me/
//! - `IncrementalAuthorization` — Invoice amount cannot be incrementally increased. Evidence: https://developers.cryptopay.me/guides/invoices
//! - `MandateRevoke` — No mandate primitive to revoke. Evidence: https://developers.cryptopay.me/
//! - `PaymentMethodToken` — No tokenization primitive. Evidence: https://developers.cryptopay.me/
//! - `PayoutCreate` — CryptoPay offers Coin Withdrawal (on-chain) but not the Hyperswitch Payout primitive (card/bank-transfer beneficiary). Evidence: https://developers.cryptopay.me/guides/payouts
//! - `PayoutCreateLink` — Not supported. Evidence: https://developers.cryptopay.me/guides/payouts
//! - `PayoutCreateRecipient` — Not supported. Evidence: https://developers.cryptopay.me/guides/payouts
//! - `PayoutEnrollDisburseAccount` — Not supported. Evidence: https://developers.cryptopay.me/guides/payouts
//! - `PayoutGet` — Payout primitive not aligned; out of scope. Evidence: https://developers.cryptopay.me/guides/payouts
//! - `PayoutStage` — Not supported. Evidence: https://developers.cryptopay.me/guides/payouts
//! - `PayoutTransfer` — Not part of CryptoPay's API surface. Evidence: https://developers.cryptopay.me/guides/payouts
//! - `PayoutVoid` — Not supported. Evidence: https://developers.cryptopay.me/guides/payouts
//! - `PostAuthenticate` — No 3DS. Evidence: https://developers.cryptopay.me/
//! - `PreAuthenticate` — No 3DS (crypto, not card). Evidence: https://developers.cryptopay.me/
//! - `RSync` — No refund API endpoint; refunds are on-chain returns handled automatically by CryptoPay for unresolved invoices. Evidence: https://developers.cryptopay.me/guides/invoices/how-to-handle-unresolved-invoices/invoice-refunds
//! - `Refund` — No merchant-initiated refund API. Refunds occur automatically (underpaid/paid late/overpaid) or via dashboard. Evidence: https://developers.cryptopay.me/guides/invoices/how-to-handle-unresolved-invoices/invoice-refunds
//! - `RepeatPayment` — No recurring / stored-payment mechanism. Evidence: https://developers.cryptopay.me/
//! - `ServerAuthenticationToken` — HMAC per-request auth only. Evidence: https://developers.cryptopay.me/guides/api-basics/authentication
//! - `ServerSessionAuthenticationToken` — HMAC per-request auth; no session-token endpoint. Evidence: https://developers.cryptopay.me/guides/api-basics/authentication
//! - `SetupMandate` — No mandate primitives exist in CryptoPay API. Evidence: https://developers.cryptopay.me/
//! - `SubmitEvidence` — No dispute management endpoints. Evidence: https://developers.cryptopay.me/
//! - `Void` — No void endpoint: invoices cannot be cancelled via API once created; they expire or are paid. Evidence: https://developers.cryptopay.me/guides/invoices
//! - `VoidPC` — No pre-capture void endpoint. Evidence: https://developers.cryptopay.me/guides/invoices
//!
//! ## Payment methods
//!
//! - `AFFIRM` (BNPL) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `AFTERPAY_CLEARPAY` (BNPL) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `ALMA` (BNPL) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `ATOME` (BNPL) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `LAZY_PAY` (BNPL) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `PAY_BRIGHT` (BNPL) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `WALLEY` (BNPL) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `ACH` (BankDebit) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `BACS` (BankDebit) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `BECS` (BankDebit) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `EFT` (BankDebit) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `SEPA` (BankDebit) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `BANCONTACT_CARD` (BankRedirect) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `BLIK` (BankRedirect) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `EPS` (BankRedirect) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `GIROPAY` (BankRedirect) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `IDEAL` (BankRedirect) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `LOCAL_BANK_REDIRECT` (BankRedirect) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `NETBANKING` (BankRedirect) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `ONLINE_BANKING_CZECH_REPUBLIC` (BankRedirect) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `ONLINE_BANKING_FINLAND` (BankRedirect) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `ONLINE_BANKING_FPX` (BankRedirect) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `ONLINE_BANKING_POLAND` (BankRedirect) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `ONLINE_BANKING_SLOVAKIA` (BankRedirect) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `ONLINE_BANKING_THAILAND` (BankRedirect) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `OPEN_BANKING_UK` (BankRedirect) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `PRZELEWY24` (BankRedirect) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `PSE` (BankRedirect) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `SOFORT` (BankRedirect) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `TRUSTLY_BANK_REDIRECT` (BankRedirect) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `BCA_BANK_TRANSFER` (BankTransfer) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `BNI_VA` (BankTransfer) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `BRI_VA` (BankTransfer) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `CIMB_VA` (BankTransfer) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `DANAMON_VA` (BankTransfer) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `INSTANT_BANK_TRANSFER` (BankTransfer) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `INSTANT_BANK_TRANSFER_FINLAND` (BankTransfer) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `INSTANT_BANK_TRANSFER_POLAND` (BankTransfer) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `LOCAL_BANK_TRANSFER` (BankTransfer) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `MANDIRI_VA` (BankTransfer) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `MULTIBANCO` (BankTransfer) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `PERMATA_BANK_TRANSFER` (BankTransfer) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `PIX` (BankTransfer) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `SEPA_BANK_TRANSFER` (BankTransfer) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `CARD_REDIRECT` (Card) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `CREDIT` (Card) — Crypto-only processor; no card acceptance API. Evidence: https://developers.cryptopay.me/
//! - `DEBIT` (Card) — Crypto-only processor; no card acceptance API. Evidence: https://developers.cryptopay.me/
//! - `NETWORK_TOKEN` (Card) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `GIVEX` (GiftCard) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `PAY_SAFE_CARD` (GiftCard) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `OPEN_BANKING` (OpenBanking) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `OPEN_BANKING_PIS` (OpenBanking) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `BENEFIT` (Other) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `BILL_DESK` (Other) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `BIZUM` (Other) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `CASH_FREE` (Other) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `DIRECT_CARRIER_BILLING` (Other) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `DUIT_NOW` (Other) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `EASE_BUZZ` (Other) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `FPS` (Other) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `INTERAC` (Other) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `KNET` (Other) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `PAY_U` (Other) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `PROMPT_PAY` (Other) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `VIET_QR` (Other) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `CLASSIC_REWARD` (Reward) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `UPI_COLLECT` (Upi) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `UPI_INTENT` (Upi) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `UPI_QR` (Upi) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `ALFAMART` (Voucher) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `BOLETO` (Voucher) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `EFECTY` (Voucher) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `EVOUCHER` (Voucher) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `FAMILY_MART` (Voucher) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `INDOMARET` (Voucher) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `LAWSON` (Voucher) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `MINI_STOP` (Voucher) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `OXXO` (Voucher) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `PAGO_EFECTIVO` (Voucher) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `PAY_EASY` (Voucher) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `RED_COMPRA` (Voucher) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `RED_PAGOS` (Voucher) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `SEICOMART` (Voucher) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `SEVEN_ELEVEN` (Voucher) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `ALI_PAY` (Wallet) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `ALI_PAY_HK` (Wallet) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `AMAZON_PAY` (Wallet) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `APPLE_PAY` (Wallet) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `CASHAPP` (Wallet) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `DANA` (Wallet) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `GCASH` (Wallet) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `GOOGLE_PAY` (Wallet) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `GO_PAY` (Wallet) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `KAKAO_PAY` (Wallet) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `MB_WAY` (Wallet) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `MOBILE_PAY` (Wallet) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `MOMO` (Wallet) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `MOMO_ATM` (Wallet) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `PAY_PAL` (Wallet) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `PAZE` (Wallet) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `PHONE_PE` (Wallet) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `REVOLUT_PAY` (Wallet) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `SAMSUNG_PAY` (Wallet) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `SATISPAY` (Wallet) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `SWISH` (Wallet) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `TOUCH_N_GO` (Wallet) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `TWINT` (Wallet) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `VENMO` (Wallet) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `VIPPS` (Wallet) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `WERO` (Wallet) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//! - `WE_CHAT_PAY` (Wallet) — Crypto-only processor. Evidence: https://developers.cryptopay.me/
//!
//! _Last generated: 2026-04-21T00:00:00Z from capability_matrix.json_

pub mod transformers;

use std::fmt::Debug;

use common_enums::CurrencyUnit;
use transformers::{
    self as cryptopay, CryptopayPaymentsRequest, CryptopayPaymentsResponse,
    CryptopayPaymentsResponse as CryptopayPaymentsSyncResponse,
};

use super::macros;
use crate::types::ResponseRouterData;
use hex::encode;

use domain_types::{
    connector_flow::{
        Accept, Authenticate, Authorize, Capture, ClientAuthenticationToken,
        CreateConnectorCustomer, CreateOrder, DefendDispute, IncrementalAuthorization,
        MandateRevoke, PSync, PaymentMethodToken, PostAuthenticate, PreAuthenticate, RSync, Refund,
        RepeatPayment, ServerAuthenticationToken, ServerSessionAuthenticationToken, SetupMandate,
        SubmitEvidence, Void, VoidPC,
    },
    connector_types::{
        AcceptDisputeData, ClientAuthenticationTokenRequestData, ConnectorCustomerData,
        ConnectorCustomerResponse, ConnectorWebhookSecrets, DisputeDefendData, DisputeFlowData,
        DisputeResponseData, EventType, MandateRevokeRequestData, MandateRevokeResponseData,
        PaymentCreateOrderData, PaymentCreateOrderResponse, PaymentFlowData,
        PaymentMethodTokenResponse, PaymentMethodTokenizationData, PaymentVoidData,
        PaymentsAuthenticateData, PaymentsAuthorizeData, PaymentsCancelPostCaptureData,
        PaymentsCaptureData, PaymentsIncrementalAuthorizationData, PaymentsPostAuthenticateData,
        PaymentsPreAuthenticateData, PaymentsResponseData, PaymentsSyncData, RefundFlowData,
        RefundSyncData, RefundsData, RefundsResponseData, RepeatPaymentData, RequestDetails,
        ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData,
        ServerSessionAuthenticationTokenRequestData, ServerSessionAuthenticationTokenResponseData,
        SetupMandateRequestData, SubmitEvidenceData, WebhookDetailsResponse,
    },
    payment_method_data::PaymentMethodDataTypes,
    types::Connectors,
};

use common_utils::{
    crypto::{self, GenerateDigest, SignMessage, VerifySignature},
    date_time,
    errors::CustomResult,
    events,
    ext_traits::ByteSliceExt,
    request::Method,
};
use serde::Serialize;

use domain_types::{
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
};

use domain_types::router_response_types::Response;
use interfaces::{
    api::ConnectorCommon, connector_integration_v2::ConnectorIntegrationV2, connector_types,
    decode::BodyDecoding, verification::SourceVerification,
};

use hyperswitch_masking::{Mask, Maskable, PeekInterface};

use crate::with_error_response_body;

use base64::Engine;

pub const BASE64_ENGINE: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;

use domain_types::errors::ConnectorError;
use domain_types::errors::{IntegrationError, WebhookError};
use error_stack::{report, ResultExt};
pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    pub(crate) const AUTHORIZATION: &str = "Authorization";
    pub(crate) const DATE: &str = "Date";
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        IncrementalAuthorization,
        PaymentFlowData,
        PaymentsIncrementalAuthorizationData,
        PaymentsResponseData,
    > for Cryptopay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Cryptopay<T>
{
    fn id(&self) -> &'static str {
        "cryptopay"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Base
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn get_auth_header(
        &self,
        auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        let auth = cryptopay::CryptopayAuthType::try_from(auth_type).change_context(
            IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            },
        )?;
        Ok(vec![(
            headers::AUTHORIZATION.to_string(),
            auth.api_key.peek().to_owned().into_masked(),
        )])
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        connectors.cryptopay.base_url.as_ref()
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        let response: cryptopay::CryptopayErrorResponse = res
            .response
            .parse_struct("CryptopayErrorResponse")
            .change_context(
                crate::utils::response_deserialization_fail(
                    res.status_code,
                "cryptopay: response body did not match the expected format; confirm API version and connector documentation."),
            )?;

        with_error_response_body!(event_builder, response);

        Ok(ErrorResponse {
            status_code: res.status_code,
            code: response.error.code,
            message: response.error.message,
            reason: response.error.reason,
            attempt_status: None,
            connector_transaction_id: None,
            network_advice_code: None,
            network_decline_code: None,
            network_error_message: None,
        })
    }
}

// Trait implementations with generic type parameters
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ClientAuthentication for Cryptopay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Cryptopay<T>
{
}

macros::macro_connector_payout_implementation!(
    connector: Cryptopay,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Cryptopay<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for Cryptopay<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ServerAuthentication for Cryptopay<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::CreateConnectorCustomer for Cryptopay<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ServerSessionAuthentication for Cryptopay<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidPostCaptureV2 for Cryptopay<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        VoidPC,
        PaymentFlowData,
        PaymentsCancelPostCaptureData,
        PaymentsResponseData,
    > for Cryptopay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidV2 for Cryptopay<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundSyncV2 for Cryptopay<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for Cryptopay<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentCapture for Cryptopay<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::SetupMandateV2<T> for Cryptopay<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Cryptopay<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for Cryptopay<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Cryptopay<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::AcceptDispute for Cryptopay<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentIncrementalAuthorization for Cryptopay<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::SubmitEvidenceV2 for Cryptopay<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::DisputeDefend for Cryptopay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentOrderCreate for Cryptopay<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Cryptopay<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RepeatPaymentV2<T> for Cryptopay<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentTokenV2<T> for Cryptopay<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentPreAuthenticateV2<T> for Cryptopay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthenticateV2<T> for Cryptopay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentPostAuthenticateV2<T> for Cryptopay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::MandateRevokeV2 for Cryptopay<T>
{
}

macros::create_amount_converter_wrapper!(connector_name: Cryptopay, amount_type: StringMajorUnit);
macros::create_all_prerequisites!(
    connector_name: Cryptopay,
    generic_type: T,
    api: [
        (
            flow: Authorize,
            request_body: CryptopayPaymentsRequest,
            response_body: CryptopayPaymentsResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: PSync,
            response_body: CryptopayPaymentsSyncResponse,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        )
    ],
    amount_converters: [],
    member_functions: {
        pub fn build_headers<F, Req, Res>(
            &self,
            req: &RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError>
        where
            Self: ConnectorIntegrationV2<F, PaymentFlowData, Req, Res>,
        {
            let method = self.get_http_method();
            let payload = match method {
                Method::Get => String::default(),
                Method::Post | Method::Put | Method::Delete | Method::Patch => {
                    let body = self
                        .get_request_body(req)?
                        .map(|content| content.get_inner_value().peek().to_owned())
                        .unwrap_or_default();
                    let md5_payload = crypto::Md5
                        .generate_digest(body.as_bytes())
                        .change_context(IntegrationError::RequestEncodingFailed { context: Default::default() })?;
                    encode(md5_payload)
                }
            };
            let api_method = method.to_string();

            let now = date_time::date_as_yyyymmddthhmmssmmmz()
                .change_context(IntegrationError::RequestEncodingFailed { context: Default::default() })?;
            let date = format!("{}+00:00", now.split_at(now.len() - 5).0);

            let content_type = self.get_content_type().to_string();

            let api = (self.get_url(req)?).replace(self.connector_base_url_payments(req), "");

            let auth = cryptopay::CryptopayAuthType::try_from(&req.connector_config)?;

            let sign_req: String = format!("{api_method}\n{payload}\n{content_type}\n{date}\n{api}");
            let authz = crypto::HmacSha1::sign_message(
                &crypto::HmacSha1,
                auth.api_secret.peek().as_bytes(),
                sign_req.as_bytes(),
            )
            .change_context(IntegrationError::RequestEncodingFailed { context: Default::default() })
            .attach_printable("Failed to sign the message")?;
            let authz = BASE64_ENGINE.encode(authz);
            let auth_string: String = format!("HMAC {}:{}", auth.api_key.peek(), authz);

            let headers = vec![
                (
                    headers::AUTHORIZATION.to_string(),
                    auth_string.into_masked(),
                ),
                (headers::DATE.to_string(), date.into()),
                (
                    headers::CONTENT_TYPE.to_string(),
                    self.get_content_type().to_string().into(),
                ),
            ];
            Ok(headers)
        }

        pub fn connector_base_url_payments<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.cryptopay.base_url
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Cryptopay,
    curl_request: Json(CryptopayPaymentsRequest),
    curl_response: CryptopayResponse,
    flow_name: Authorize,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsAuthorizeData<T>,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}/api/invoices", self.connector_base_url_payments(req)))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Cryptopay,
    curl_response: CryptopayPaymentResponse,
    flow_name: PSync,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsSyncData,
    flow_response: PaymentsResponseData,
    http_method: Get,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let custom_id = req.resource_common_data.connector_request_reference_id.clone();

            Ok(format!(
                "{}/api/invoices/custom_id/{custom_id}",
                self.connector_base_url_payments(req),
            ))
        }
    }
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Cryptopay<T>
{
    fn get_webhook_source_verification_signature(
        &self,
        request: &RequestDetails,
        _connector_webhook_secret: &ConnectorWebhookSecrets,
    ) -> Result<Vec<u8>, error_stack::Report<WebhookError>> {
        let base64_signature = request
            .headers
            .get("x-cryptopay-signature")
            .ok_or_else(|| report!(WebhookError::WebhookSignatureNotFound))
            .attach_printable("Missing incoming webhook signature for Cryptopay")?;
        hex::decode(base64_signature).change_context(WebhookError::WebhookSourceVerificationFailed)
    }

    fn get_webhook_source_verification_message(
        &self,
        request: &RequestDetails,
        _connector_webhook_secrets: &ConnectorWebhookSecrets,
    ) -> Result<Vec<u8>, error_stack::Report<WebhookError>> {
        let message = std::str::from_utf8(&request.body)
            .change_context(WebhookError::WebhookSourceVerificationFailed)
            .attach_printable("Webhook source verification message parsing failed for Cryptopay")?;
        Ok(message.to_string().into_bytes())
    }

    fn verify_webhook_source(
        &self,
        request: RequestDetails,
        connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<bool, error_stack::Report<WebhookError>> {
        let algorithm = crypto::HmacSha256;

        let connector_webhook_secrets = match connector_webhook_secret {
            Some(secrets) => secrets,
            None => {
                return Err(error_stack::report!(
                    WebhookError::WebhookVerificationSecretNotFound
                ));
            }
        };

        let signature =
            self.get_webhook_source_verification_signature(&request, &connector_webhook_secrets)?;

        let message =
            self.get_webhook_source_verification_message(&request, &connector_webhook_secrets)?;

        algorithm
            .verify_signature(&connector_webhook_secrets.secret, &signature, &message)
            .change_context(WebhookError::WebhookSourceVerificationFailed)
            .attach_printable("Webhook source verification failed for Cryptopay")
    }

    fn get_event_type(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<EventType, error_stack::Report<WebhookError>> {
        let notif: cryptopay::CryptopayWebhookDetails = request
            .body
            .parse_struct("CryptopayWebhookDetails")
            .change_context(WebhookError::WebhookBodyDecodingFailed)?;
        match notif.data.status {
            cryptopay::CryptopayPaymentStatus::Completed => Ok(EventType::PaymentIntentSuccess),
            cryptopay::CryptopayPaymentStatus::Unresolved => Ok(EventType::PaymentActionRequired),
            cryptopay::CryptopayPaymentStatus::Cancelled => Ok(EventType::PaymentIntentFailure),
            _ => Ok(EventType::IncomingWebhookEventUnspecified),
        }
    }

    fn process_payment_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<WebhookDetailsResponse, error_stack::Report<WebhookError>> {
        let notif: cryptopay::CryptopayWebhookDetails = request
            .body
            .parse_struct("CryptopayWebhookDetails")
            .change_context(WebhookError::WebhookBodyDecodingFailed)?;
        let response = WebhookDetailsResponse::try_from(notif)
            .change_context(WebhookError::WebhookBodyDecodingFailed);

        response.map(|mut response| {
            response.raw_connector_response =
                Some(String::from_utf8_lossy(&request.body).to_string());
            response
        })
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
    for Cryptopay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
    for Cryptopay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
    for Cryptopay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
    for Cryptopay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        SetupMandate,
        PaymentFlowData,
        SetupMandateRequestData<T>,
        PaymentsResponseData,
    > for Cryptopay<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<Accept, DisputeFlowData, AcceptDisputeData, DisputeResponseData>
    for Cryptopay<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<SubmitEvidence, DisputeFlowData, SubmitEvidenceData, DisputeResponseData>
    for Cryptopay<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<DefendDispute, DisputeFlowData, DisputeDefendData, DisputeResponseData>
    for Cryptopay<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        CreateOrder,
        PaymentFlowData,
        PaymentCreateOrderData,
        PaymentCreateOrderResponse,
    > for Cryptopay<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        ServerSessionAuthenticationToken,
        PaymentFlowData,
        ServerSessionAuthenticationTokenRequestData,
        ServerSessionAuthenticationTokenResponseData,
    > for Cryptopay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        ServerAuthenticationToken,
        PaymentFlowData,
        ServerAuthenticationTokenRequestData,
        ServerAuthenticationTokenResponseData,
    > for Cryptopay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        PaymentMethodToken,
        PaymentFlowData,
        PaymentMethodTokenizationData<T>,
        PaymentMethodTokenResponse,
    > for Cryptopay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        PreAuthenticate,
        PaymentFlowData,
        PaymentsPreAuthenticateData<T>,
        PaymentsResponseData,
    > for Cryptopay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        Authenticate,
        PaymentFlowData,
        PaymentsAuthenticateData<T>,
        PaymentsResponseData,
    > for Cryptopay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        PostAuthenticate,
        PaymentFlowData,
        PaymentsPostAuthenticateData<T>,
        PaymentsResponseData,
    > for Cryptopay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        ClientAuthenticationToken,
        PaymentFlowData,
        ClientAuthenticationTokenRequestData,
        PaymentsResponseData,
    > for Cryptopay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        CreateConnectorCustomer,
        PaymentFlowData,
        ConnectorCustomerData,
        ConnectorCustomerResponse,
    > for Cryptopay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        MandateRevoke,
        PaymentFlowData,
        MandateRevokeRequestData,
        MandateRevokeResponseData,
    > for Cryptopay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        RepeatPayment,
        PaymentFlowData,
        RepeatPaymentData<T>,
        PaymentsResponseData,
    > for Cryptopay<T>
{
}
