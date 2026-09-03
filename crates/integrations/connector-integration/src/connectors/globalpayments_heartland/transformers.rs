//! GlobalpaymentsHeartland (Global Payments — Heartland *Portico* gateway) transformers.
//!
//! Portico is a **SOAP 1.1 / XML** gateway with a single RPC endpoint
//! (`PosGatewayService.asmx` → `DoTransaction`). Every flow POSTs the same envelope to
//! the same URL; the operation is selected by the name of the single child element
//! inside `<Ver1.0><Transaction>`.
//!
//! Five things this module deliberately gets right, because they are the easy things to
//! get wrong (all verified against the CERT gateway):
//!
//! 1. Amounts are **major-unit decimal strings** (`"10.00"`), never minor units.
//! 2. Every response is **HTTP 200**; status is read from the body through a two-level
//!    gate — `Ver1.0/Header/GatewayRspCode` first, then the issuer `RspCode` (which only
//!    exists on the auth-bearing `Credit*` transactions).
//! 3. When `GatewayRspCode != 0` there is **no `<Transaction>` element at all**, so the
//!    transaction field is always `Option<…>`.
//! 4. `CreditAddToBatch` (capture), `CreditVoid` (void) and `CreditReturn` (refund)
//!    answer with an **empty element**. Success is `GatewayRspCode == 0` and nothing
//!    else — those responses are modelled header-only.
//! 5. 3DS is **external pass-through only**: a `<Secure3D>` block inside `Block1`. No
//!    redirect, no ACS URL, no second leg. The element is omitted entirely for non-3DS.

use common_enums::{AttemptStatus, RefundStatus};
use common_utils::types::StringMajorUnit;
use domain_types::{
    connector_flow::{Authorize, Capture, PSync, RSync, Refund, Void},
    connector_types::{
        PaymentFlowData, PaymentVoidData, PaymentsAuthorizeData, PaymentsCaptureData,
        PaymentsResponseData, PaymentsSyncData, RefundFlowData, RefundSyncData, RefundsData,
        RefundsResponseData, ResponseId,
    },
    errors::{ConnectorError, IntegrationError, IntegrationErrorContext},
    payment_method_data::{PaymentMethodData, PaymentMethodDataTypes},
    router_data::{ConnectorSpecificConfig, ErrorResponse, FlowStatus},
    router_data_v2::RouterDataV2,
};
use error_stack::Report;
use hyperswitch_masking::{PeekInterface, Secret};
use serde::{Deserialize, Serialize};

use super::{GlobalpaymentsHeartlandAmountConvertor, GlobalpaymentsHeartlandRouterData};
use crate::types::ResponseRouterData;

// =============================================================================
// PINNED CONSTANTS
// =============================================================================

/// Sent on every verified pre-flight call. Optional per the Portico guide, pinned here so
/// the wire format matches exactly what was verified.
const DEVELOPER_ID: &str = "002914";
/// Ditto — pinned alongside `DeveloperID`.
const VERSION_NBR: &str = "3333";
/// **Required.** Without it Portico rejects a repeated same-amount transaction as a
/// duplicate instead of approving it.
const ALLOW_DUP: &str = "Y";
/// Card-not-present e-commerce: the card is not physically present …
const CARD_PRESENT: &str = "N";
/// … and there is no card reader.
const READER_PRESENT: &str = "N";

/// `Ver1.0/Header/GatewayRspCode` value meaning "the gateway accepted the request".
const GATEWAY_RSP_CODE_SUCCESS: &str = "0";
/// Issuer `RspCode` for an approval (`RspText = APPROVAL`).
const ISSUER_RSP_CODE_APPROVAL: &str = "00";
/// Issuer `RspCode` returned by `CreditAccountVerify` (`RspText = CARD OK`). Out of scope
/// here, but treated as an approval so a stray `85` is never read as a decline.
const ISSUER_RSP_CODE_CARD_OK: &str = "85";

/// `ReportTxnDetail` `Data/TxnStatus` for an approved / active transaction.
const TXN_STATUS_ACTIVE: &str = "A";

/// `ReportTxnDetail` `Data/TxnStatus` for a transaction that has been reversed — i.e. voided.
///
/// This is reported on the **original** transaction (the `CreditAuth` / `CreditSale` that was
/// voided), alongside a non-zero `Data/ReversalAmtInfo`. Verified live: `200139845260`
/// (voided `CreditSale`) and `200139841418` (voided `CreditAuth`) both report `R`, while
/// `200139837897` (an auth that was captured, not voided) still reports `A`.
const TXN_STATUS_REVERSED: &str = "R";

/// `ReportTxnDetail` `ServiceName` values.
const SERVICE_NAME_CREDIT_AUTH: &str = "CreditAuth";
const SERVICE_NAME_CREDIT_SALE: &str = "CreditSale";
const SERVICE_NAME_CREDIT_VOID: &str = "CreditVoid";
const SERVICE_NAME_CREDIT_RETURN: &str = "CreditReturn";

const NO_ERROR_CODE: &str = "NO_ERROR_CODE";
const NO_ERROR_MESSAGE: &str = "NO_ERROR_MESSAGE";

/// `Secure3D/Version` when the authentication carries no message version. The schema
/// declares `2` as the default, so this matches the gateway's own assumption.
const SECURE_3D_DEFAULT_VERSION: &str = "2";

/// Normalise an ECI to Portico's `eciType`.
///
/// `eciType` is `xs:string` with `xs:length value="1"` and `xs:pattern value="[0-9]"` — a
/// **single** digit. 3DS servers almost universally emit two digits (`05`, `06`, `02`), so
/// the leading zero has to come off or Portico rejects the whole message at schema
/// validation, before it ever reaches the issuer.
///
/// Anything that is not a 1- or 2-digit number is surfaced as an error rather than
/// truncated: silently reshaping an unrecognised ECI would assert an authentication outcome
/// the 3DS server never reported.
fn normalize_eci(eci: &str) -> Result<String, Report<IntegrationError>> {
    let trimmed = eci.trim();
    let normalized = match trimmed.len() {
        1 => trimmed,
        2 if trimmed.starts_with('0') => &trimmed[1..],
        _ => "",
    };

    if normalized.len() == 1 && normalized.chars().all(|c| c.is_ascii_digit()) {
        return Ok(normalized.to_string());
    }

    Err(error_stack::report!(IntegrationError::InvalidDataFormat {
        field_name: "authentication_data.eci",
        context: IntegrationErrorContext {
            additional_context: Some(format!(
                "globalpayments_heartland: ECI {eci:?} is not a single digit. Portico's eciType \
                 is xs:length 1 with pattern [0-9]; only a bare digit (5) or a zero-padded \
                 digit (05) can be sent."
            )),
            ..Default::default()
        },
    }))
}

/// Map a 3DS message version onto Portico's `Secure3D/Version` enum.
///
/// The enum encodes the **3DS 2.x minor** version, not a semantic major:
///
/// | Value | Protocol            | Value | Protocol            |
/// |-------|---------------------|-------|---------------------|
/// | `1`   | 3DS 1.x (withdrawn) | `6`   | 3DS 2.6             |
/// | `2`   | 3DS **2.2**         | `7`   | 3DS 2.7             |
/// | `3`   | 3DS **2.3**         | `8`   | 3DS 2.8             |
/// | `4`   | 3DS 2.4             | `9`   | 3DS 2.9             |
/// | `5`   | 3DS 2.5             |       |                     |
///
/// So `2.3.1` must be sent as `3`, not `2`. Mapping the major version instead would report
/// every 2.x as 2.2 — schema-valid, and therefore silently wrong on the wire.
///
/// 3DS 2.0 and 2.1 have no value of their own (`1` is taken by the withdrawn v1), so they
/// map to `2`, which is also the schema default.
///
/// 3DS 1.x is refused outright. Portico carries v1 results in the separate
/// `SecureECommerce` block, which this connector does not implement, so emitting
/// `Version = 1` here would send a value the gateway documents as unsupported.
fn secure_3d_version(
    message_version: Option<&common_utils::types::SemanticVersion>,
) -> Result<String, Report<IntegrationError>> {
    let Some(version) = message_version else {
        return Ok(SECURE_3D_DEFAULT_VERSION.to_string());
    };

    match (version.get_major(), version.get_minor()) {
        (1, _) => Err(error_stack::report!(IntegrationError::NotImplemented(
            "globalpayments_heartland: 3DS 1.x is not supported by Portico. Its Secure3D block \
             carries 3DS 2.x only; v1 results belong in the SecureECommerce block, which this \
             connector does not implement."
                .to_string(),
            IntegrationErrorContext::default(),
        ))),
        // 2.0 and 2.1 have no enum value of their own; 2.2 is the value `2`.
        (2, 0..=2) => Ok(SECURE_3D_DEFAULT_VERSION.to_string()),
        (2, minor @ 3..=9) => Ok(minor.to_string()),
        // No 3DS 2.10+ exists. Fall back to the schema default rather than fail an otherwise
        // valid authentication over a version the enum simply has no room for.
        (2, _) => Ok(SECURE_3D_DEFAULT_VERSION.to_string()),
        (other, _) => Err(error_stack::report!(IntegrationError::InvalidDataFormat {
            field_name: "authentication_data.message_version",
            context: IntegrationErrorContext {
                additional_context: Some(format!(
                    "globalpayments_heartland: 3DS major version {other} has no Secure3D/Version \
                     mapping; the enum covers 3DS 1.x and 2.2-2.9 only."
                )),
                ..Default::default()
            },
        })),
    }
}

// =============================================================================
// AUTH
// =============================================================================

/// Portico credentials.
///
/// The `SecretAPIKey` is **not** an HTTP header: it is written into the SOAP body at
/// `Ver1.0/Header/SecretAPIKey`. `ConnectorCommon::get_auth_header` therefore returns an
/// empty vector and this type is threaded into every request transformer instead.
#[derive(Debug, Clone)]
pub struct GlobalpaymentsHeartlandAuthType {
    pub secret_api_key: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for GlobalpaymentsHeartlandAuthType {
    type Error = Report<IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::GlobalpaymentsHeartland { api_key, .. } => Ok(Self {
                secret_api_key: api_key.to_owned(),
            }),
            _ => Err(error_stack::report!(
                IntegrationError::FailedToObtainAuthType {
                    context: IntegrationErrorContext::default(),
                }
            )),
        }
    }
}

// =============================================================================
// SOAP ENVELOPE
// =============================================================================

/// `Ver1.0/Header` on a **request**.
#[derive(Debug, Clone, Serialize)]
#[serde(rename = "Header")]
pub struct GlobalpaymentsHeartlandRequestHeader {
    #[serde(rename = "SecretAPIKey")]
    pub secret_api_key: Secret<String>,
    #[serde(rename = "DeveloperID")]
    pub developer_id: &'static str,
    #[serde(rename = "VersionNbr")]
    pub version_nbr: &'static str,
}

impl GlobalpaymentsHeartlandRequestHeader {
    fn new(secret_api_key: Secret<String>) -> Self {
        Self {
            secret_api_key,
            developer_id: DEVELOPER_ID,
            version_nbr: VERSION_NBR,
        }
    }
}

/// Serialises one PosGateway request into the verified SOAP 1.1 envelope.
///
/// Portico takes **plain nested XML** — unlike `bamboraapac`, there is no `<![CDATA[…]]>`
/// wrapper, and adding one produces a body the gateway rejects. Both the header and the
/// transaction element go through `quick_xml::se` so every value is XML-escaped; only the
/// fixed envelope scaffolding is formatted in.
fn to_pos_request_envelope<TXN: Serialize>(
    header: &GlobalpaymentsHeartlandRequestHeader,
    transaction: &TXN,
) -> String {
    let header_xml = quick_xml::se::to_string(header).unwrap_or_else(|_| String::from("<Header/>"));
    let transaction_xml = quick_xml::se::to_string(transaction).unwrap_or_else(|_| String::new());

    format!(
        concat!(
            r#"<?xml version="1.0" encoding="utf-8"?>"#,
            r#"<soap:Envelope xmlns:soap="http://schemas.xmlsoap.org/soap/envelope/">"#,
            r#"<soap:Body>"#,
            r#"<PosRequest xmlns="http://Hps.Exchange.PosGateway">"#,
            r#"<Ver1.0>{}<Transaction>{}</Transaction></Ver1.0>"#,
            r#"</PosRequest></soap:Body></soap:Envelope>"#
        ),
        header_xml, transaction_xml
    )
}

// =============================================================================
// RESPONSE ENVELOPE
// =============================================================================
//
// `preprocess_response_bytes` strips the `soap:` prefix and the namespace declarations
// before these structs see the body, so the root element is a bare `<Envelope>`. quick-xml
// ignores the root element name and binds the root's children onto the struct fields.

/// `Ver1.0/Header` on a **response**. Present on every reply, including gateway-level
/// rejections, which carry nothing else.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct GlobalpaymentsHeartlandResponseHeader {
    /// The id of **this** transaction. For an authorization it is the payment's
    /// `connector_transaction_id`; for a capture/void it is the follow-up leg's own id;
    /// for a refund it is the `connector_refund_id`.
    #[serde(rename = "GatewayTxnId")]
    pub gateway_txn_id: Option<String>,
    /// `0` = accepted. Anything else = rejected, and there is no `<Transaction>` element.
    #[serde(rename = "GatewayRspCode")]
    pub gateway_rsp_code: Option<String>,
    #[serde(rename = "GatewayRspMsg")]
    pub gateway_rsp_msg: Option<String>,
}

impl GlobalpaymentsHeartlandResponseHeader {
    fn is_gateway_accepted(&self) -> bool {
        self.gateway_rsp_code.as_deref() == Some(GATEWAY_RSP_CODE_SUCCESS)
    }

    fn error_code(&self) -> String {
        self.gateway_rsp_code
            .clone()
            .unwrap_or_else(|| NO_ERROR_CODE.to_string())
    }

    fn error_message(&self) -> String {
        self.gateway_rsp_msg
            .clone()
            .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string())
    }
}

/// `Ver1.0` on a response.
///
/// `transaction` **must** stay `Option`: a gateway-level rejection (`GatewayRspCode != 0`)
/// omits the element entirely, and a required field there would turn every readable
/// gateway error into an opaque deserialization failure.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GlobalpaymentsHeartlandVer<TXN> {
    #[serde(rename = "Header")]
    pub header: GlobalpaymentsHeartlandResponseHeader,
    #[serde(rename = "Transaction")]
    pub transaction: Option<TXN>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GlobalpaymentsHeartlandPosResponse<TXN> {
    #[serde(rename = "Ver1.0")]
    pub ver: GlobalpaymentsHeartlandVer<TXN>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GlobalpaymentsHeartlandBody<TXN> {
    #[serde(rename = "PosResponse")]
    pub pos_response: GlobalpaymentsHeartlandPosResponse<TXN>,
}

/// Full `<Envelope>` for a response that carries a `<Transaction>` payload.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GlobalpaymentsHeartlandEnvelope<TXN> {
    #[serde(rename = "Body")]
    pub body: GlobalpaymentsHeartlandBody<TXN>,
}

impl<TXN> GlobalpaymentsHeartlandEnvelope<TXN> {
    fn header(&self) -> &GlobalpaymentsHeartlandResponseHeader {
        &self.body.pos_response.ver.header
    }

    fn transaction(&self) -> Option<&TXN> {
        self.body.pos_response.ver.transaction.as_ref()
    }
}

// -----------------------------------------------------------------------------
// Header-only envelope — capture / void / refund
// -----------------------------------------------------------------------------
//
// `CreditAddToBatch`, `CreditVoid` and `CreditReturn` all answer with an **empty**
// element (`<CreditAddToBatch />`). There is no `RspCode` inside them, so there is
// nothing to bind: the `<Transaction>` element is simply not modelled and quick-xml skips
// it as an unknown child. Success is `Header/GatewayRspCode == 0`, and nothing else.

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GlobalpaymentsHeartlandAckVer {
    #[serde(rename = "Header")]
    pub header: GlobalpaymentsHeartlandResponseHeader,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GlobalpaymentsHeartlandAckPosResponse {
    #[serde(rename = "Ver1.0")]
    pub ver: GlobalpaymentsHeartlandAckVer,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GlobalpaymentsHeartlandAckBody {
    #[serde(rename = "PosResponse")]
    pub pos_response: GlobalpaymentsHeartlandAckPosResponse,
}

/// `<Envelope>` for the three flows whose transaction element is empty.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GlobalpaymentsHeartlandAckEnvelope {
    #[serde(rename = "Body")]
    pub body: GlobalpaymentsHeartlandAckBody,
}

impl GlobalpaymentsHeartlandAckEnvelope {
    fn header(&self) -> &GlobalpaymentsHeartlandResponseHeader {
        &self.body.pos_response.ver.header
    }
}

/// Error response type used by `ConnectorCommon::build_error_response`. Portico never
/// answers with anything but HTTP 200 on the verified paths, so this only fires for a
/// transport-level surprise; it still parses the gateway header when one is present.
pub type GlobalpaymentsHeartlandErrorResponse = GlobalpaymentsHeartlandAckEnvelope;

// =============================================================================
// SHARED REQUEST FRAGMENTS
// =============================================================================

/// `Block1/CardData/ManualEntry`. `ExpYear` is **4 digits** (`2030`), not 2.
#[derive(Debug, Clone, Serialize)]
#[serde(rename = "ManualEntry")]
pub struct GlobalpaymentsHeartlandManualEntry {
    #[serde(rename = "CardNbr")]
    pub card_nbr: Secret<String>,
    #[serde(rename = "ExpMonth")]
    pub exp_month: Secret<String>,
    #[serde(rename = "ExpYear")]
    pub exp_year: Secret<String>,
    #[serde(rename = "CVV2", skip_serializing_if = "Option::is_none")]
    pub cvv2: Option<Secret<String>>,
    #[serde(rename = "CardPresent")]
    pub card_present: &'static str,
    #[serde(rename = "ReaderPresent")]
    pub reader_present: &'static str,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename = "CardData")]
pub struct GlobalpaymentsHeartlandCardData {
    #[serde(rename = "ManualEntry")]
    pub manual_entry: GlobalpaymentsHeartlandManualEntry,
}

/// Optional AVS / cardholder block. Omitted entirely when no billing data is available —
/// every verified pre-flight call omitted it and still approved.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename = "CardHolderData")]
pub struct GlobalpaymentsHeartlandCardHolderData {
    #[serde(
        rename = "CardHolderFirstName",
        skip_serializing_if = "Option::is_none"
    )]
    pub card_holder_first_name: Option<Secret<String>>,
    #[serde(rename = "CardHolderLastName", skip_serializing_if = "Option::is_none")]
    pub card_holder_last_name: Option<Secret<String>>,
    #[serde(rename = "CardHolderAddr", skip_serializing_if = "Option::is_none")]
    pub card_holder_addr: Option<Secret<String>>,
    #[serde(rename = "CardHolderCity", skip_serializing_if = "Option::is_none")]
    pub card_holder_city: Option<Secret<String>>,
    #[serde(rename = "CardHolderState", skip_serializing_if = "Option::is_none")]
    pub card_holder_state: Option<Secret<String>>,
    #[serde(rename = "CardHolderZip", skip_serializing_if = "Option::is_none")]
    pub card_holder_zip: Option<Secret<String>>,
    #[serde(rename = "CardHolderEmail", skip_serializing_if = "Option::is_none")]
    pub card_holder_email: Option<Secret<String>>,
    #[serde(rename = "CardHolderPhone", skip_serializing_if = "Option::is_none")]
    pub card_holder_phone: Option<Secret<String>>,
}

impl GlobalpaymentsHeartlandCardHolderData {
    fn is_empty(&self) -> bool {
        self.card_holder_first_name.is_none()
            && self.card_holder_last_name.is_none()
            && self.card_holder_addr.is_none()
            && self.card_holder_city.is_none()
            && self.card_holder_state.is_none()
            && self.card_holder_zip.is_none()
            && self.card_holder_email.is_none()
            && self.card_holder_phone.is_none()
    }
}

/// External 3DS pass-through. Portico hosts no challenge and returns no redirect: the
/// authentication already happened elsewhere and its results ride along here.
///
/// Child order is the verified `xs:sequence`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename = "Secure3D")]
pub struct GlobalpaymentsHeartlandSecure3D {
    /// `Secure3D/Version` enum value — see [`secure_3d_version`]. Optional in the schema
    /// (default `2`), always sent here so the version is explicit on the wire.
    #[serde(rename = "Version")]
    pub version: String,
    /// CAVV / AEVV / UCAF. **Optional** in the schema (`minOccurs="0"`): a frictionless or
    /// attempted authentication can carry an ECI with no cryptogram.
    ///
    /// Sent as a bare element with no `EncodingType` attribute. The published schema docs
    /// describe one (defaulting to `base64`), but the deployed cert gateway rejects it —
    /// `Message failed validation. The 'EncodingType' attribute is not declared.` — so the
    /// value is always base64 by convention, not by declaration.
    #[serde(
        rename = "AuthenticationValue",
        skip_serializing_if = "Option::is_none"
    )]
    pub authentication_value: Option<Secret<String>>,
    /// **Required** by the schema (`minOccurs="1"`), so it is not optional here. Always a
    /// single digit — see [`normalize_eci`].
    #[serde(rename = "ECI")]
    pub eci: String,
    #[serde(
        rename = "DirectoryServerTxnId",
        skip_serializing_if = "Option::is_none"
    )]
    pub directory_server_txn_id: Option<String>,
}

/// `CreditAuth`/`CreditSale` `Block1`.
///
/// **Field order is load-bearing**: the XSD declares an `xs:sequence`
/// (`AllowDup`, `Amt`, `CardData`, `CardHolderData`, `Secure3D`) and quick-xml serialises
/// in declaration order. Reordering these gets the request rejected.
#[derive(Debug, Clone, Serialize)]
#[serde(rename = "Block1")]
pub struct GlobalpaymentsHeartlandPaymentBlock1 {
    #[serde(rename = "AllowDup")]
    pub allow_dup: &'static str,
    #[serde(rename = "Amt")]
    pub amt: StringMajorUnit,
    #[serde(rename = "CardData")]
    pub card_data: GlobalpaymentsHeartlandCardData,
    #[serde(rename = "CardHolderData", skip_serializing_if = "Option::is_none")]
    pub card_holder_data: Option<GlobalpaymentsHeartlandCardHolderData>,
    #[serde(rename = "Secure3D", skip_serializing_if = "Option::is_none")]
    pub secure_3d: Option<GlobalpaymentsHeartlandSecure3D>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GlobalpaymentsHeartlandCreditBody {
    #[serde(rename = "Block1")]
    pub block1: GlobalpaymentsHeartlandPaymentBlock1,
}

/// The single child of `<Transaction>` on an authorization.
///
/// `CreditAuth` and `CreditSale` are byte-identical apart from the element name, so the
/// same `Block1` builder serves both; the variant is chosen by `capture_method`.
#[derive(Debug, Clone, Serialize)]
pub enum GlobalpaymentsHeartlandPaymentTransaction {
    /// Manual capture — leaves an open authorization for `CreditAddToBatch`.
    #[serde(rename = "CreditAuth")]
    CreditAuth(GlobalpaymentsHeartlandCreditBody),
    /// Auto capture — enters the open batch immediately.
    #[serde(rename = "CreditSale")]
    CreditSale(GlobalpaymentsHeartlandCreditBody),
}

// =============================================================================
// AUTHORIZE
// =============================================================================

/// Authorize request: `CreditAuth` (manual capture) or `CreditSale` (auto capture).
#[derive(Debug, Clone, Serialize)]
pub struct GlobalpaymentsHeartlandPaymentsRequest {
    pub header: GlobalpaymentsHeartlandRequestHeader,
    pub transaction: GlobalpaymentsHeartlandPaymentTransaction,
}

impl super::super::macros::GetSoapXml for GlobalpaymentsHeartlandPaymentsRequest {
    fn to_soap_xml(&self) -> String {
        to_pos_request_envelope(&self.header, &self.transaction)
    }
}

type AuthorizeRouterData<T> =
    RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>;

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<GlobalpaymentsHeartlandRouterData<AuthorizeRouterData<T>, T>>
    for GlobalpaymentsHeartlandPaymentsRequest
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: GlobalpaymentsHeartlandRouterData<AuthorizeRouterData<T>, T>,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let request = &router_data.request;
        let auth = GlobalpaymentsHeartlandAuthType::try_from(&router_data.connector_config)?;

        let card = match &request.payment_method_data {
            PaymentMethodData::Card(card) => card,
            _ => {
                return Err(error_stack::report!(IntegrationError::NotImplemented(
                    "Only card payments are supported by globalpayments_heartland".to_string(),
                    IntegrationErrorContext::default(),
                )))
            }
        };

        let amt = GlobalpaymentsHeartlandAmountConvertor::convert(
            request.minor_amount,
            request.currency,
        )?;

        let manual_entry = GlobalpaymentsHeartlandManualEntry {
            card_nbr: Secret::new(card.card_number.peek().to_string()),
            exp_month: card.get_card_expiry_month_2_digit()?,
            // Portico wants the full four digits (`2030`).
            exp_year: card.get_expiry_year_4_digit(),
            cvv2: Some(card.card_cvc.clone()),
            card_present: CARD_PRESENT,
            reader_present: READER_PRESENT,
        };

        let common = &router_data.resource_common_data;
        let card_holder_data = GlobalpaymentsHeartlandCardHolderData {
            card_holder_first_name: common.get_optional_billing_first_name(),
            card_holder_last_name: common.get_optional_billing_last_name(),
            card_holder_addr: common.get_optional_billing_line1(),
            card_holder_city: common.get_optional_billing_city(),
            card_holder_state: common.get_optional_billing_state(),
            card_holder_zip: common.get_optional_billing_zip(),
            card_holder_email: request
                .email
                .clone()
                .or_else(|| common.get_optional_billing_email())
                .map(|email| Secret::new(email.peek().to_string())),
            card_holder_phone: common.get_optional_billing_phone_number(),
        };
        // The whole element is optional; sending an empty one is not verified, so it is
        // omitted rather than emitted blank.
        let card_holder_data = (!card_holder_data.is_empty()).then_some(card_holder_data);

        // `<Secure3D>` is emitted only when external authentication results are actually
        // present. There is no redirect leg for this connector: 3DS here means the
        // authentication already completed elsewhere.
        //
        // The gate is the **ECI**, not the cryptogram. `ECI` is the one required child of
        // `Secure3DType`; `AuthenticationValue` is optional. Gating on the CAVV instead would
        // both drop a frictionless/attempted result that has an ECI but no cryptogram, and
        // let a CAVV-without-ECI through as schema-invalid XML.
        let secure_3d = request
            .authentication_data
            .as_ref()
            .and_then(|auth_data| auth_data.eci.as_ref().map(|eci| (auth_data, eci)))
            .map(|(auth_data, eci)| {
                // Mastercard Identity Check requires the directory-server transaction id on
                // the authorization. Assert this only when the network is actually known —
                // `card_network` is frequently absent, and guessing must not fail a payment.
                if auth_data.ds_trans_id.is_none()
                    && card.card_network == Some(common_enums::CardNetwork::Mastercard)
                {
                    return Err(error_stack::report!(
                        IntegrationError::MissingRequiredField {
                            field_name: "authentication_data.ds_transaction_id",
                            context: IntegrationErrorContext {
                                additional_context: Some(
                                    "globalpayments_heartland: Portico requires \
                                     Secure3D/DirectoryServerTxnId for Mastercard Identity \
                                     Check authorizations."
                                        .to_string(),
                                ),
                                ..Default::default()
                            },
                        }
                    ));
                }

                Ok::<_, Report<IntegrationError>>(GlobalpaymentsHeartlandSecure3D {
                    version: secure_3d_version(auth_data.message_version.as_ref())?,
                    authentication_value: auth_data.cavv.clone(),
                    eci: normalize_eci(eci)?,
                    directory_server_txn_id: auth_data.ds_trans_id.clone(),
                })
            })
            .transpose()?;

        let body = GlobalpaymentsHeartlandCreditBody {
            block1: GlobalpaymentsHeartlandPaymentBlock1 {
                allow_dup: ALLOW_DUP,
                amt,
                card_data: GlobalpaymentsHeartlandCardData { manual_entry },
                card_holder_data,
                secure_3d,
            },
        };

        let transaction = if request.is_auto_capture() {
            GlobalpaymentsHeartlandPaymentTransaction::CreditSale(body)
        } else {
            GlobalpaymentsHeartlandPaymentTransaction::CreditAuth(body)
        };

        Ok(Self {
            header: GlobalpaymentsHeartlandRequestHeader::new(auth.secret_api_key),
            transaction,
        })
    }
}

/// Body of a `<CreditAuth>` / `<CreditSale>` response element.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct GlobalpaymentsHeartlandCreditResponseBody {
    /// Issuer response code. `00` = APPROVAL, `85` = CARD OK.
    #[serde(rename = "RspCode")]
    pub rsp_code: Option<String>,
    #[serde(rename = "RspText")]
    pub rsp_text: Option<String>,
    #[serde(rename = "AuthCode")]
    pub auth_code: Option<String>,
    #[serde(rename = "AVSRsltCode")]
    pub avs_rslt_code: Option<String>,
    #[serde(rename = "CVVRsltCode")]
    pub cvv_rslt_code: Option<String>,
    #[serde(rename = "RefNbr")]
    pub ref_nbr: Option<String>,
    #[serde(rename = "CardType")]
    pub card_type: Option<String>,
    #[serde(rename = "CardBrandTxnId")]
    pub card_brand_txn_id: Option<String>,
}

impl GlobalpaymentsHeartlandCreditResponseBody {
    fn is_approved(&self) -> bool {
        matches!(
            self.rsp_code.as_deref(),
            Some(ISSUER_RSP_CODE_APPROVAL) | Some(ISSUER_RSP_CODE_CARD_OK)
        )
    }
}

/// `<Transaction>` on an authorization: exactly one of the two elements is present.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct GlobalpaymentsHeartlandCreditTransaction {
    #[serde(rename = "CreditAuth")]
    pub credit_auth: Option<GlobalpaymentsHeartlandCreditResponseBody>,
    #[serde(rename = "CreditSale")]
    pub credit_sale: Option<GlobalpaymentsHeartlandCreditResponseBody>,
}

impl GlobalpaymentsHeartlandCreditTransaction {
    fn body(&self) -> Option<&GlobalpaymentsHeartlandCreditResponseBody> {
        self.credit_auth.as_ref().or(self.credit_sale.as_ref())
    }
}

pub type GlobalpaymentsHeartlandPaymentsResponse =
    GlobalpaymentsHeartlandEnvelope<GlobalpaymentsHeartlandCreditTransaction>;

fn payment_error_response(
    header: &GlobalpaymentsHeartlandResponseHeader,
    code: String,
    message: String,
    http_code: u16,
    status: AttemptStatus,
) -> ErrorResponse {
    ErrorResponse {
        status_code: http_code,
        code,
        message: message.clone(),
        reason: Some(message),
        attempt_status: Some(FlowStatus::Payment(status)),
        connector_transaction_id: header.gateway_txn_id.clone(),
        network_decline_code: None,
        network_advice_code: None,
        network_error_message: None,
        typed_connector_response: None,
        raw_connector_response: None,
        raw_connector_request: None,
        typed_connector_request: None,
    }
}

impl<T: PaymentMethodDataTypes>
    TryFrom<ResponseRouterData<GlobalpaymentsHeartlandPaymentsResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<GlobalpaymentsHeartlandPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;
        let header = response.header();
        let is_auto_capture = item.router_data.request.is_auto_capture();

        // Level 1 — gateway. A rejection carries no `<Transaction>` at all.
        if !header.is_gateway_accepted() {
            let status = AttemptStatus::Failure;
            return Ok(Self {
                response: Err(payment_error_response(
                    header,
                    header.error_code(),
                    header.error_message(),
                    item.http_code,
                    status,
                )),
                resource_common_data: PaymentFlowData {
                    status,
                    ..item.router_data.resource_common_data.clone()
                },
                ..item.router_data
            });
        }

        // Level 2 — issuer.
        let body = response
            .transaction()
            .and_then(GlobalpaymentsHeartlandCreditTransaction::body)
            .ok_or_else(|| {
                crate::utils::response_deserialization_fail(
                    item.http_code,
                    "globalpayments_heartland: gateway accepted the request but the response carried no CreditAuth/CreditSale element.",
                )
            })?;

        if !body.is_approved() {
            let status = if is_auto_capture {
                AttemptStatus::Failure
            } else {
                AttemptStatus::AuthorizationFailed
            };
            return Ok(Self {
                response: Err(payment_error_response(
                    header,
                    body.rsp_code
                        .clone()
                        .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
                    body.rsp_text
                        .clone()
                        .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
                    item.http_code,
                    status,
                )),
                resource_common_data: PaymentFlowData {
                    status,
                    ..item.router_data.resource_common_data.clone()
                },
                ..item.router_data
            });
        }

        // `connector_transaction_id` always comes from the *header*, never from the
        // transaction body.
        let connector_transaction_id = header.gateway_txn_id.clone().ok_or_else(|| {
            crate::utils::response_deserialization_fail(
                item.http_code,
                "globalpayments_heartland: approved response missing Header/GatewayTxnId.",
            )
        })?;

        let status = if is_auto_capture {
            AttemptStatus::Charged
        } else {
            AttemptStatus::Authorized
        };

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(connector_transaction_id),
                // Portico never returns a redirect: external 3DS only.
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: body.card_brand_txn_id.clone(),
                network_txn_link_id: None,
                connector_response_reference_id: body.ref_nbr.clone(),
                incremental_authorization_allowed: None,
                splits: None,
                status_code: item.http_code,
                payment_account_reference: None,
            }),
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data.clone()
            },
            ..item.router_data
        })
    }
}

// =============================================================================
// CAPTURE — `CreditAddToBatch`
// =============================================================================

/// `<CreditAddToBatch>` — note it does **not** nest in a `Block1` (unlike `CreditReturn`).
#[derive(Debug, Clone, Serialize)]
#[serde(rename = "CreditAddToBatch")]
pub struct GlobalpaymentsHeartlandCreditAddToBatch {
    #[serde(rename = "GatewayTxnId")]
    pub gateway_txn_id: String,
    #[serde(rename = "Amt")]
    pub amt: StringMajorUnit,
}

#[derive(Debug, Clone, Serialize)]
pub struct GlobalpaymentsHeartlandCaptureRequest {
    pub header: GlobalpaymentsHeartlandRequestHeader,
    pub transaction: GlobalpaymentsHeartlandCreditAddToBatch,
}

impl super::super::macros::GetSoapXml for GlobalpaymentsHeartlandCaptureRequest {
    fn to_soap_xml(&self) -> String {
        to_pos_request_envelope(&self.header, &self.transaction)
    }
}

type CaptureRouterData =
    RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>;

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<GlobalpaymentsHeartlandRouterData<CaptureRouterData, T>>
    for GlobalpaymentsHeartlandCaptureRequest
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: GlobalpaymentsHeartlandRouterData<CaptureRouterData, T>,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth = GlobalpaymentsHeartlandAuthType::try_from(&router_data.connector_config)?;

        let amt = GlobalpaymentsHeartlandAmountConvertor::convert(
            router_data.request.minor_amount_to_capture,
            router_data.request.currency,
        )?;

        Ok(Self {
            header: GlobalpaymentsHeartlandRequestHeader::new(auth.secret_api_key),
            transaction: GlobalpaymentsHeartlandCreditAddToBatch {
                gateway_txn_id: router_data.request.get_connector_transaction_id()?,
                amt,
            },
        })
    }
}

/// `<CreditAddToBatch />` is empty — success is the gateway code alone.
pub type GlobalpaymentsHeartlandCaptureResponse = GlobalpaymentsHeartlandAckEnvelope;

impl TryFrom<ResponseRouterData<GlobalpaymentsHeartlandCaptureResponse, Self>>
    for CaptureRouterData
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<GlobalpaymentsHeartlandCaptureResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let header = item.response.header();

        if !header.is_gateway_accepted() {
            let status = AttemptStatus::CaptureFailed;
            return Ok(Self {
                response: Err(payment_error_response(
                    header,
                    header.error_code(),
                    header.error_message(),
                    item.http_code,
                    status,
                )),
                resource_common_data: PaymentFlowData {
                    status,
                    ..item.router_data.resource_common_data.clone()
                },
                ..item.router_data
            });
        }

        // The capture's own `GatewayTxnId` identifies the *batch* leg, not the payment.
        // PSync must keep querying the original authorization id, so the payment's
        // `connector_transaction_id` is echoed back unchanged and the batch id is exposed
        // as the response reference instead.
        let original_txn_id = item.router_data.request.get_connector_transaction_id().ok();
        let resource_id = match original_txn_id {
            Some(id) => ResponseId::ConnectorTransactionId(id),
            None => ResponseId::NoResponseId,
        };

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id,
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: header.gateway_txn_id.clone(),
                incremental_authorization_allowed: None,
                splits: None,
                status_code: item.http_code,
                payment_account_reference: None,
            }),
            resource_common_data: PaymentFlowData {
                status: AttemptStatus::Charged,
                ..item.router_data.resource_common_data.clone()
            },
            ..item.router_data
        })
    }
}

// =============================================================================
// VOID — `CreditVoid`
// =============================================================================

/// `<CreditVoid>` — no amount; voids are always full. Works on both an uncaptured
/// `CreditAuth` and a `CreditSale` still in the open batch, so no `CreditReversal` is
/// needed.
#[derive(Debug, Clone, Serialize)]
#[serde(rename = "CreditVoid")]
pub struct GlobalpaymentsHeartlandCreditVoid {
    #[serde(rename = "GatewayTxnId")]
    pub gateway_txn_id: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct GlobalpaymentsHeartlandVoidRequest {
    pub header: GlobalpaymentsHeartlandRequestHeader,
    pub transaction: GlobalpaymentsHeartlandCreditVoid,
}

impl super::super::macros::GetSoapXml for GlobalpaymentsHeartlandVoidRequest {
    fn to_soap_xml(&self) -> String {
        to_pos_request_envelope(&self.header, &self.transaction)
    }
}

type VoidRouterData = RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>;

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<GlobalpaymentsHeartlandRouterData<VoidRouterData, T>>
    for GlobalpaymentsHeartlandVoidRequest
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: GlobalpaymentsHeartlandRouterData<VoidRouterData, T>,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth = GlobalpaymentsHeartlandAuthType::try_from(&router_data.connector_config)?;
        let gateway_txn_id = router_data.request.connector_transaction_id.clone();

        if gateway_txn_id.is_empty() {
            return Err(error_stack::report!(
                IntegrationError::MissingRequiredField {
                    field_name: "connector_transaction_id",
                    context: IntegrationErrorContext::default(),
                }
            ));
        }

        Ok(Self {
            header: GlobalpaymentsHeartlandRequestHeader::new(auth.secret_api_key),
            transaction: GlobalpaymentsHeartlandCreditVoid { gateway_txn_id },
        })
    }
}

/// `<CreditVoid />` is empty — success is the gateway code alone.
pub type GlobalpaymentsHeartlandVoidResponse = GlobalpaymentsHeartlandAckEnvelope;

impl TryFrom<ResponseRouterData<GlobalpaymentsHeartlandVoidResponse, Self>> for VoidRouterData {
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<GlobalpaymentsHeartlandVoidResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let header = item.response.header();

        if !header.is_gateway_accepted() {
            let status = AttemptStatus::VoidFailed;
            return Ok(Self {
                response: Err(payment_error_response(
                    header,
                    header.error_code(),
                    header.error_message(),
                    item.http_code,
                    status,
                )),
                resource_common_data: PaymentFlowData {
                    status,
                    ..item.router_data.resource_common_data.clone()
                },
                ..item.router_data
            });
        }

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                // As with capture, the void's own id is a new leg and must not replace
                // the payment's id.
                resource_id: ResponseId::ConnectorTransactionId(
                    item.router_data.request.connector_transaction_id.clone(),
                ),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: header.gateway_txn_id.clone(),
                incremental_authorization_allowed: None,
                splits: None,
                status_code: item.http_code,
                payment_account_reference: None,
            }),
            resource_common_data: PaymentFlowData {
                status: AttemptStatus::Voided,
                ..item.router_data.resource_common_data.clone()
            },
            ..item.router_data
        })
    }
}

// =============================================================================
// REFUND — `CreditReturn`
// =============================================================================

/// `CreditReturn` **does** nest its fields in `Block1` — unlike `CreditAddToBatch` and
/// `CreditVoid`. `Amt` is positive on the request; the gateway applies the sign.
#[derive(Debug, Clone, Serialize)]
#[serde(rename = "Block1")]
pub struct GlobalpaymentsHeartlandRefundBlock1 {
    #[serde(rename = "AllowDup")]
    pub allow_dup: &'static str,
    #[serde(rename = "Amt")]
    pub amt: StringMajorUnit,
    #[serde(rename = "GatewayTxnId")]
    pub gateway_txn_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename = "CreditReturn")]
pub struct GlobalpaymentsHeartlandCreditReturn {
    #[serde(rename = "Block1")]
    pub block1: GlobalpaymentsHeartlandRefundBlock1,
}

#[derive(Debug, Clone, Serialize)]
pub struct GlobalpaymentsHeartlandRefundRequest {
    pub header: GlobalpaymentsHeartlandRequestHeader,
    pub transaction: GlobalpaymentsHeartlandCreditReturn,
}

impl super::super::macros::GetSoapXml for GlobalpaymentsHeartlandRefundRequest {
    fn to_soap_xml(&self) -> String {
        to_pos_request_envelope(&self.header, &self.transaction)
    }
}

type RefundRouterData = RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>;

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<GlobalpaymentsHeartlandRouterData<RefundRouterData, T>>
    for GlobalpaymentsHeartlandRefundRequest
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: GlobalpaymentsHeartlandRouterData<RefundRouterData, T>,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth = GlobalpaymentsHeartlandAuthType::try_from(&router_data.connector_config)?;

        let amt = GlobalpaymentsHeartlandAmountConvertor::convert(
            router_data.request.minor_refund_amount,
            router_data.request.currency,
        )?;

        let gateway_txn_id = router_data.request.connector_transaction_id.clone();
        if gateway_txn_id.is_empty() {
            return Err(error_stack::report!(
                IntegrationError::MissingRequiredField {
                    field_name: "connector_transaction_id",
                    context: IntegrationErrorContext::default(),
                }
            ));
        }

        Ok(Self {
            header: GlobalpaymentsHeartlandRequestHeader::new(auth.secret_api_key),
            transaction: GlobalpaymentsHeartlandCreditReturn {
                block1: GlobalpaymentsHeartlandRefundBlock1 {
                    allow_dup: ALLOW_DUP,
                    amt,
                    gateway_txn_id,
                },
            },
        })
    }
}

/// `<CreditReturn />` is empty — success is the gateway code alone.
pub type GlobalpaymentsHeartlandRefundResponse = GlobalpaymentsHeartlandAckEnvelope;

/// `status` is what the refund should be left as. A genuine rejection passes
/// `RefundStatus::Failure`; a failed sync QUERY passes the status it started with.
///
/// `generate_refund_sync_response` has no 2xx fallback — it reads `attempt_status` and
/// `unwrap_or_default()`s — so omitting it does not hold the status, it erases it.
fn refund_error_response(
    header: &GlobalpaymentsHeartlandResponseHeader,
    http_code: u16,
    status: RefundStatus,
) -> ErrorResponse {
    let message = header.error_message();
    ErrorResponse {
        status_code: http_code,
        code: header.error_code(),
        message: message.clone(),
        reason: Some(message),
        attempt_status: Some(FlowStatus::Refund(status)),
        connector_transaction_id: header.gateway_txn_id.clone(),
        network_decline_code: None,
        network_advice_code: None,
        network_error_message: None,
        typed_connector_response: None,
        raw_connector_response: None,
        raw_connector_request: None,
        typed_connector_request: None,
    }
}

impl TryFrom<ResponseRouterData<GlobalpaymentsHeartlandRefundResponse, Self>> for RefundRouterData {
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<GlobalpaymentsHeartlandRefundResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let header = item.response.header();

        if !header.is_gateway_accepted() {
            return Ok(Self {
                response: Err(refund_error_response(
                    header,
                    item.http_code,
                    RefundStatus::Failure,
                )),
                resource_common_data: RefundFlowData {
                    status: RefundStatus::Failure,
                    ..item.router_data.resource_common_data.clone()
                },
                ..item.router_data
            });
        }

        // Here the new leg id *is* the refund, and it is what RSync must query.
        let connector_refund_id = header.gateway_txn_id.clone().ok_or_else(|| {
            crate::utils::response_deserialization_fail(
                item.http_code,
                "globalpayments_heartland: CreditReturn response missing Header/GatewayTxnId; it is required as connector_refund_id.",
            )
        })?;

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id,
                refund_status: RefundStatus::Success,
                status_code: item.http_code,
                acquirer_reference_number: None,
            }),
            resource_common_data: RefundFlowData {
                status: RefundStatus::Success,
                ..item.router_data.resource_common_data.clone()
            },
            ..item.router_data
        })
    }
}

// =============================================================================
// REPORT TXN DETAIL — shared by PSync and RSync
// =============================================================================

#[derive(Debug, Clone, Serialize)]
#[serde(rename = "ReportTxnDetail")]
pub struct GlobalpaymentsHeartlandReportTxnDetail {
    #[serde(rename = "TxnId")]
    pub txn_id: String,
}

/// `ReportTxnDetail/Data`. Amounts are major-unit decimal strings and are **negative for
/// a return** (`-5.00`), so they are parsed as `String`, never as an unsigned type.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct GlobalpaymentsHeartlandReportData {
    /// `A` is the only value observed live.
    #[serde(rename = "TxnStatus")]
    pub txn_status: Option<String>,
    /// Issuer result — **empty for a `CreditReturn`**, which is not a failure.
    #[serde(rename = "RspCode")]
    pub rsp_code: Option<String>,
    #[serde(rename = "RspText")]
    pub rsp_text: Option<String>,
    #[serde(rename = "Amt")]
    pub amt: Option<String>,
    #[serde(rename = "AuthAmt")]
    pub auth_amt: Option<String>,
    #[serde(rename = "SettlementAmt")]
    pub settlement_amt: Option<String>,
    #[serde(rename = "AuthCode")]
    pub auth_code: Option<String>,
    /// `S` = sale, `R` = return.
    #[serde(rename = "SaleReturnInd")]
    pub sale_return_ind: Option<String>,
    #[serde(rename = "ReturnAmtInfo")]
    pub return_amt_info: Option<String>,
    #[serde(rename = "ReversalAmtInfo")]
    pub reversal_amt_info: Option<String>,
    #[serde(rename = "MaskedCardNbr")]
    pub masked_card_nbr: Option<String>,
    #[serde(rename = "CardType")]
    pub card_type: Option<String>,
    #[serde(rename = "RefNbr")]
    pub ref_nbr: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct GlobalpaymentsHeartlandReportBody {
    #[serde(rename = "GatewayTxnId")]
    pub gateway_txn_id: Option<String>,
    /// `CreditAuth` | `CreditSale` | `CreditReturn` | `CreditVoid`.
    #[serde(rename = "ServiceName")]
    pub service_name: Option<String>,
    /// `0` for a fresh transaction; the parent id for a `CreditReturn`.
    #[serde(rename = "OriginalGatewayTxnId")]
    pub original_gateway_txn_id: Option<String>,
    #[serde(rename = "GatewayRspCode")]
    pub gateway_rsp_code: Option<String>,
    #[serde(rename = "GatewayRspMsg")]
    pub gateway_rsp_msg: Option<String>,
    #[serde(rename = "Data")]
    pub data: Option<GlobalpaymentsHeartlandReportData>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct GlobalpaymentsHeartlandReportTransaction {
    #[serde(rename = "ReportTxnDetail")]
    pub report_txn_detail: Option<GlobalpaymentsHeartlandReportBody>,
}

/// PSync status derivation.
///
/// `ServiceName` alone is not enough and `TxnStatus` alone is not enough: an auth and a
/// sale both report `A` and they mean different UCS statuses, and both report `R` once
/// voided.
///
/// Observed live: `A` (active) and `R` (reversed/voided) on the original transaction, and
/// `I` on a `CreditVoid` record. A `ReportTxnDetail` fetched by a `CreditVoid` id *is* the
/// void record, so its mere existence proves the void — hence that arm matches any
/// `TxnStatus` rather than pinning one.
///
/// Genuinely unobserved values still fall through to `Pending`: a conservative `Pending` is
/// recoverable, a wrong `Charged`/`Failure` is not.
fn map_psync_status(body: Option<&GlobalpaymentsHeartlandReportBody>) -> AttemptStatus {
    let Some(body) = body else {
        return AttemptStatus::Pending;
    };
    let data = body.data.as_ref();
    let txn_status = data.and_then(|data| data.txn_status.as_deref());
    let service_name = body.service_name.as_deref();

    // An issuer decline is reported on an auth-bearing transaction with a non-approval
    // `Data/RspCode`. Without this the report's issuer result is deserialized and never read,
    // so a declined sale reports Charged and a declined auth sticks in Pending forever.
    //
    // Gated on the service name because a successful `CreditReturn` reports an **empty**
    // `RspCode` — verified live — which must not be read as a decline.
    if matches!(
        service_name,
        Some(SERVICE_NAME_CREDIT_AUTH) | Some(SERVICE_NAME_CREDIT_SALE)
    ) {
        if let Some(rsp_code) = data.and_then(|data| data.rsp_code.as_deref()) {
            if !rsp_code.is_empty()
                && !matches!(rsp_code, ISSUER_RSP_CODE_APPROVAL | ISSUER_RSP_CODE_CARD_OK)
            {
                return AttemptStatus::Failure;
            }
        }
    }

    match (service_name, txn_status) {
        // KNOWN LIMITATION: Portico reports a captured auth identically to an open one, so
        // this arm reports `Authorized` for both and a PSync after Capture walks the payment
        // back from `Charged`.
        //
        // This is not an oversight. `ReportTxnDetail` was diffed field by field either side of
        // a `CreditAddToBatch` (txn 200139928251): `ServiceName`, `TxnStatus`, `Amt`, `AuthAmt`
        // and `SettlementAmt` are all byte-identical. `SettlementAmt` is populated *before*
        // capture — it is the amount that would settle, not the amount that has. The only
        // delta is `ReversalAmtInfo` appearing as `0.00`, a zero-valued reversal field that
        // says nothing about capture.
        //
        // The framework cannot help either: `PaymentFlowData::status` is hard-coded to
        // `Pending` for every sync and `PaymentServiceGetRequest` carries no prior status, so
        // there is nothing to compare against. Closing this needs either a Portico batch query
        // (a second round trip per sync) or an optional prior-status field on the proto.
        (Some(SERVICE_NAME_CREDIT_AUTH), Some(TXN_STATUS_ACTIVE)) => AttemptStatus::Authorized,
        (Some(SERVICE_NAME_CREDIT_SALE), Some(TXN_STATUS_ACTIVE)) => AttemptStatus::Charged,
        // A voided auth/sale reports `R` on the ORIGINAL transaction. Without these two arms
        // a PSync after a successful Void reports `Pending` forever.
        (Some(SERVICE_NAME_CREDIT_AUTH), Some(TXN_STATUS_REVERSED))
        | (Some(SERVICE_NAME_CREDIT_SALE), Some(TXN_STATUS_REVERSED)) => AttemptStatus::Voided,
        // Syncing the void record itself. `I` is what the gateway reports here; matching any
        // status keeps this correct if other values surface.
        (Some(SERVICE_NAME_CREDIT_VOID), _) => AttemptStatus::Voided,
        // The underlying payment stays charged; the refund itself is tracked by RSync.
        (Some(SERVICE_NAME_CREDIT_RETURN), Some(TXN_STATUS_ACTIVE)) => AttemptStatus::Charged,
        _ => AttemptStatus::Pending,
    }
}

/// RSync status derivation.
///
/// Gated on `ServiceName == "CreditReturn"` first, which deliberately skips the issuer
/// code check: a successful return reports an **empty** `RspCode`, and reading that as a
/// decline is the RSync equivalent of the empty-element trap.
fn map_rsync_status(body: Option<&GlobalpaymentsHeartlandReportBody>) -> RefundStatus {
    let Some(body) = body else {
        return RefundStatus::Pending;
    };
    let txn_status = body
        .data
        .as_ref()
        .and_then(|data| data.txn_status.as_deref());

    // A `CreditReturn` that the issuer rejected carries a non-approval `Data/RspCode`. A
    // SUCCESSFUL return reports an empty `RspCode`, so only a non-empty non-approval value is
    // a failure — otherwise every good refund would be marked failed.
    if body.service_name.as_deref() == Some(SERVICE_NAME_CREDIT_RETURN) {
        if let Some(rsp_code) = body
            .data
            .as_ref()
            .and_then(|data| data.rsp_code.as_deref())
        {
            if !rsp_code.is_empty()
                && !matches!(rsp_code, ISSUER_RSP_CODE_APPROVAL | ISSUER_RSP_CODE_CARD_OK)
            {
                return RefundStatus::Failure;
            }
        }
    }

    match (body.service_name.as_deref(), txn_status) {
        (Some(SERVICE_NAME_CREDIT_RETURN), Some(TXN_STATUS_ACTIVE)) => RefundStatus::Success,
        // A reversed return is a refund that did not stand.
        (Some(SERVICE_NAME_CREDIT_RETURN), Some(TXN_STATUS_REVERSED)) => RefundStatus::Failure,
        _ => RefundStatus::Pending,
    }
}

// -----------------------------------------------------------------------------
// PSYNC
// -----------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct GlobalpaymentsHeartlandPSyncRequest {
    pub header: GlobalpaymentsHeartlandRequestHeader,
    pub transaction: GlobalpaymentsHeartlandReportTxnDetail,
}

impl super::super::macros::GetSoapXml for GlobalpaymentsHeartlandPSyncRequest {
    fn to_soap_xml(&self) -> String {
        to_pos_request_envelope(&self.header, &self.transaction)
    }
}

type PSyncRouterData = RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>;

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<GlobalpaymentsHeartlandRouterData<PSyncRouterData, T>>
    for GlobalpaymentsHeartlandPSyncRequest
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: GlobalpaymentsHeartlandRouterData<PSyncRouterData, T>,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth = GlobalpaymentsHeartlandAuthType::try_from(&router_data.connector_config)?;

        Ok(Self {
            header: GlobalpaymentsHeartlandRequestHeader::new(auth.secret_api_key),
            transaction: GlobalpaymentsHeartlandReportTxnDetail {
                txn_id: router_data.request.get_connector_transaction_id()?,
            },
        })
    }
}

pub type GlobalpaymentsHeartlandPSyncResponse =
    GlobalpaymentsHeartlandEnvelope<GlobalpaymentsHeartlandReportTransaction>;

impl TryFrom<ResponseRouterData<GlobalpaymentsHeartlandPSyncResponse, Self>> for PSyncRouterData {
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<GlobalpaymentsHeartlandPSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;
        let header = response.header();

        if !header.is_gateway_accepted() {
            // The gateway rejected the ReportTxnDetail QUERY, not the payment. Hold the status
            // the sync started with and leave `resource_common_data` untouched — inventing a
            // terminal status from a failed lookup previously marked a perfectly good
            // authorization `Failure`.
            let held_status = item.router_data.resource_common_data.status;
            return Ok(Self {
                response: Err(payment_error_response(
                    header,
                    header.error_code(),
                    header.error_message(),
                    item.http_code,
                    held_status,
                )),
                ..item.router_data
            });
        }

        let body = response
            .transaction()
            .and_then(|txn| txn.report_txn_detail.as_ref());
        let status = map_psync_status(body);

        let resource_id = match body.and_then(|body| body.gateway_txn_id.clone()) {
            Some(txn_id) => ResponseId::ConnectorTransactionId(txn_id),
            None => item.router_data.request.connector_transaction_id.clone(),
        };
        let connector_response_reference_id = body
            .and_then(|body| body.data.as_ref())
            .and_then(|data| data.ref_nbr.clone());

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id,
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id,
                incremental_authorization_allowed: None,
                splits: None,
                status_code: item.http_code,
                payment_account_reference: None,
            }),
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data.clone()
            },
            ..item.router_data
        })
    }
}

// -----------------------------------------------------------------------------
// RSYNC
// -----------------------------------------------------------------------------

/// Byte-identical on the wire to the PSync request — the only difference is that `TxnId`
/// is the **refund's own** `GatewayTxnId`, not the sale's.
#[derive(Debug, Clone, Serialize)]
pub struct GlobalpaymentsHeartlandRSyncRequest {
    pub header: GlobalpaymentsHeartlandRequestHeader,
    pub transaction: GlobalpaymentsHeartlandReportTxnDetail,
}

impl super::super::macros::GetSoapXml for GlobalpaymentsHeartlandRSyncRequest {
    fn to_soap_xml(&self) -> String {
        to_pos_request_envelope(&self.header, &self.transaction)
    }
}

type RSyncRouterData = RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>;

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<GlobalpaymentsHeartlandRouterData<RSyncRouterData, T>>
    for GlobalpaymentsHeartlandRSyncRequest
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: GlobalpaymentsHeartlandRouterData<RSyncRouterData, T>,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth = GlobalpaymentsHeartlandAuthType::try_from(&router_data.connector_config)?;

        let txn_id = router_data.request.connector_refund_id.clone();
        if txn_id.is_empty() {
            return Err(error_stack::report!(
                IntegrationError::MissingRequiredField {
                    field_name: "connector_refund_id",
                    context: IntegrationErrorContext::default(),
                }
            ));
        }

        Ok(Self {
            header: GlobalpaymentsHeartlandRequestHeader::new(auth.secret_api_key),
            transaction: GlobalpaymentsHeartlandReportTxnDetail { txn_id },
        })
    }
}

pub type GlobalpaymentsHeartlandRSyncResponse =
    GlobalpaymentsHeartlandEnvelope<GlobalpaymentsHeartlandReportTransaction>;

impl TryFrom<ResponseRouterData<GlobalpaymentsHeartlandRSyncResponse, Self>> for RSyncRouterData {
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<GlobalpaymentsHeartlandRSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;
        let header = response.header();

        if !header.is_gateway_accepted() {
            // The gateway rejected the ReportTxnDetail QUERY — an unqueryable id, reporting
            // disabled on the MID, throttling. That says nothing about the refund, so the
            // status is held and `resource_common_data` is left untouched. Writing `Failure`
            // here made a failed lookup indistinguishable from an issuer-rejected refund.
            let held_status = item.router_data.resource_common_data.status;
            return Ok(Self {
                response: Err(refund_error_response(header, item.http_code, held_status)),
                ..item.router_data
            });
        }

        let body = response
            .transaction()
            .and_then(|txn| txn.report_txn_detail.as_ref());
        let refund_status = map_rsync_status(body);

        let connector_refund_id = body
            .and_then(|body| body.gateway_txn_id.clone())
            .unwrap_or_else(|| item.router_data.request.connector_refund_id.clone());

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id,
                refund_status,
                status_code: item.http_code,
                acquirer_reference_number: None,
            }),
            resource_common_data: RefundFlowData {
                status: refund_status,
                ..item.router_data.resource_common_data.clone()
            },
            ..item.router_data
        })
    }
}
