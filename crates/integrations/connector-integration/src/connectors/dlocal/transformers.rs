use common_utils::{pii, request::Method, FloatMajorUnit};
use domain_types::{
    connector_flow::{self, Authorize, RepeatPayment, SetupMandate},
    connector_types::{
        MandateReference, MandateReferenceId, PaymentFlowData, PaymentVoidData,
        PaymentsAuthorizeData, PaymentsCaptureData, PaymentsResponseData, PaymentsSyncData,
        RefundFlowData, RefundSyncData, RefundsData, RefundsResponseData, RepeatPaymentData,
        ResponseId, SetupMandateRequestData,
    },
    errors::{ConnectorError, IntegrationError, ResponseTransformationErrorContext},
    payment_method_data::{self, PaymentMethodData, PaymentMethodDataTypes, RawCardNumber},
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::RedirectForm,
    utils,
};
use error_stack::ResultExt;
use hyperswitch_masking::Secret;
use serde::{Deserialize, Serialize};

use crate::{connectors::dlocal::DlocalRouterData, types::ResponseRouterData};

#[derive(Debug, Default, Eq, PartialEq, Serialize)]
pub struct Payer {
    pub name: Secret<String>,
    pub email: pii::Email,
    pub document: Secret<String>,
    // `phone` and `address` are only populated for payment methods that dLocal
    // requires them for (e.g. GCash Recurring "RG", which rejects requests with
    // code 5001 when they are missing). For all other methods they stay None so
    // existing card / APM request bodies are unchanged.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phone: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<DlocalAddress>,
}

/// dLocal payer address, required for some recurring APMs (e.g. GCash Recurring).
#[derive(Debug, Default, Eq, PartialEq, Serialize)]
pub struct DlocalAddress {
    pub country: common_enums::CountryAlpha2,
    pub state: Secret<String>,
    pub city: Secret<String>,
    pub zip_code: Secret<String>,
    pub street: Secret<String>,
    pub number: Secret<String>,
}

/// dLocal `wallet` object, used by tokenizable wallet APMs such as GCash
/// Recurring (RG).
///
/// - CIT (enrollment): `save: true` + the shopper's `username`/`email`/`name`
///   ask dLocal to persist a reusable token. The token itself is NOT returned in
///   the synchronous response — it arrives later via the IPN webhook
///   (`wallet.token`) once the shopper completes the redirect.
/// - MIT (reuse): `token: <wallet_token>` replays a previously saved wallet.
#[derive(Debug, Default, Eq, PartialEq, Serialize)]
pub struct Wallet {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub save: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capture: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<pii::Email>,
}

/// Builds the dLocal payer `phone` + `address` (both required for GCash
/// Recurring) from the billing address. Returns `(phone, address)` — either may
/// be `None` if the billing data is incomplete; the dLocal API will surface a
/// 5001 if a required field is actually missing.
fn build_payer_phone_and_address(
    billing: Option<&domain_types::payment_address::Address>,
) -> (Option<Secret<String>>, Option<DlocalAddress>) {
    let phone = billing
        .and_then(|b| b.phone.as_ref())
        .and_then(|p| p.get_number_with_country_code().ok());

    let address = billing
        .and_then(|b| b.address.as_ref())
        .and_then(|ad| {
            let country = ad.country?;
            Some(DlocalAddress {
                country,
                state: ad.state.clone().unwrap_or_default(),
                city: ad.city.clone().unwrap_or_default(),
                zip_code: ad.zip.clone().unwrap_or_default(),
                street: ad.line1.clone().unwrap_or_default(),
                number: ad.line2.clone().unwrap_or_else(|| Secret::new("NA".to_string())),
            })
        });

    (phone, address)
}

/// Derives the wallet `username` (and reusable display name) for the shopper.
/// dLocal wants a stable identifier; we prefer the billing full name, falling
/// back to the email local-part.
fn wallet_username(name: &Secret<String>, email: &pii::Email) -> String {
    use hyperswitch_masking::PeekInterface;
    let name_str = name.peek().trim().to_string();
    if !name_str.is_empty() {
        return name_str;
    }
    email
        .peek()
        .split('@')
        .next()
        .unwrap_or("shopper")
        .to_string()
}

#[derive(Debug, Default, Eq, Clone, PartialEq, Serialize, Deserialize)]
pub struct Card<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize> {
    pub holder_name: Option<Secret<String>>,
    pub number: RawCardNumber<T>,
    pub cvv: Secret<String>,
    pub expiration_month: Secret<String>,
    pub expiration_year: Secret<String>,
    pub capture: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub save: Option<bool>,
}

#[derive(Debug, Default, Eq, PartialEq, Serialize)]
pub struct ThreeDSecureReqData {
    pub force: bool,
}

#[derive(Debug, Serialize, Default, Deserialize, Clone, Eq, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
pub enum PaymentMethodId {
    #[default]
    Card,
    #[serde(untagged)]
    Other(String),
}

#[derive(Debug, Serialize, Default, Deserialize, Clone, Eq, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
pub enum PaymentMethodFlow {
    #[default]
    Direct,
    #[serde(rename = "REDIRECT")]
    Redirect,
}

#[derive(Default, Debug, Serialize, PartialEq)]
pub struct DlocalPaymentsRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    pub amount: FloatMajorUnit,
    pub currency: common_enums::Currency,
    pub country: common_enums::CountryAlpha2,
    pub payment_method_id: PaymentMethodId,
    pub payment_method_flow: PaymentMethodFlow,
    pub payer: Payer,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card: Option<Card<T>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wallet: Option<Wallet>,
    pub order_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub three_dsecure: Option<ThreeDSecureReqData>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub callback_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notification_url: Option<String>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        DlocalRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for DlocalPaymentsRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: DlocalRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let email = item.router_data.request.get_email()?;
        let address = item
            .router_data
            .resource_common_data
            .get_billing_address()?;
        let country = *address.get_country()?;
        let name = address.get_full_name()?;
        // dLocal requires the payer document (national/tax ID). Take it from the real
        // customer document plumbed through the request — required, mirroring hyperswitch
        // dlocal (no hardcoded placeholder).
        let document = item
            .router_data
            .request
            .get_customer_document_details()?
            .document_number;
        let amount = utils::convert_amount(
            item.connector.amount_converter,
            item.router_data.request.minor_amount,
            item.router_data.request.currency,
        )?;
        let order_id = item
            .router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();
        let callback_url = Some(item.router_data.request.get_router_return_url()?);
        let description = item.router_data.resource_common_data.description.clone();

        match item.router_data.request.payment_method_data {
            PaymentMethodData::Card(ref ccard) => {
                let should_capture = matches!(
                    item.router_data.request.capture_method,
                    Some(common_enums::CaptureMethod::Automatic)
                        | Some(common_enums::CaptureMethod::SequentialAutomatic)
                );
                let payment_request = Self {
                    amount,
                    currency: item.router_data.request.currency,
                    payment_method_id: PaymentMethodId::Card,
                    payment_method_flow: PaymentMethodFlow::Direct,
                    country,
                    payer: Payer {
                        name,
                        email,
                        document: document.clone(),
                        phone: None,
                        address: None,
                    },
                    card: Some(Card {
                        holder_name: ccard.card_holder_name.clone(),
                        number: ccard.card_number.clone(),
                        cvv: ccard.card_cvc.clone(),
                        expiration_month: ccard.card_exp_month.clone(),
                        expiration_year: ccard.card_exp_year.clone(),
                        capture: should_capture.to_string(),
                        // `save` is None for regular Authorize flow — card tokenization
                        // is handled separately via SetupMandate with `save: Some(true)`.
                        save: None,
                    }),
                    wallet: None,
                    order_id,
                    three_dsecure: match item.router_data.resource_common_data.auth_type {
                        common_enums::AuthenticationType::ThreeDs => {
                            Some(ThreeDSecureReqData { force: true })
                        }
                        common_enums::AuthenticationType::NoThreeDs => None,
                    },
                    callback_url,
                    description,
                    notification_url: None,
                };
                Ok(payment_request)
            }
            PaymentMethodData::BankDebit(ref bank_debit_data) => {
                let (payment_method_id, _notification_url) =
                    get_bank_debit_payment_method_id(bank_debit_data, country)?;
                let webhook_url = item.router_data.request.webhook_url.clone();
                let payment_request = Self {
                    amount,
                    currency: item.router_data.request.currency,
                    payment_method_id,
                    payment_method_flow: PaymentMethodFlow::Redirect,
                    country,
                    payer: Payer {
                        name,
                        email,
                        document: document.clone(),
                        phone: None,
                        address: None,
                    },
                    card: None,
                    wallet: None,
                    order_id,
                    three_dsecure: None,
                    callback_url,
                    description,
                    notification_url: webhook_url,
                };
                Ok(payment_request)
            }
            // Redirect-flow APM wallets (e.g. GCash, DANA). Same request shape as
            // the BankDebit arm above: REDIRECT flow, no card, required payer
            // document, callback + notification (webhook) URLs.
            PaymentMethodData::Wallet(ref wallet_data) => {
                let webhook_url = item.router_data.request.webhook_url.clone();
                let is_gcash = matches!(
                    wallet_data,
                    payment_method_data::WalletData::GcashRedirect(_)
                );
                // GCash Recurring (RG) enrollment via a mandate-creating sale
                // (setup_future_usage = OffSession): dLocal's "Sale + verify" charges the
                // customer AND saves a reusable wallet token in a single Authorize. The
                // reusable token is delivered later via the IPN webhook (see SetupMandate
                // arm / IncomingWebhook). Non-mandate payments and other wallets keep the
                // plain one-time redirect (GC) with no `wallet.save`.
                if is_gcash && item.router_data.request.is_mandate_payment() {
                    let billing = item.router_data.resource_common_data.get_optional_billing();
                    let (phone, payer_address) = build_payer_phone_and_address(billing);
                    let username = wallet_username(&name, &email);
                    let payment_request = Self {
                        amount,
                        currency: item.router_data.request.currency,
                        payment_method_id: PaymentMethodId::Other("RG".to_string()),
                        payment_method_flow: PaymentMethodFlow::Redirect,
                        country,
                        payer: Payer {
                            name: name.clone(),
                            email: email.clone(),
                            document: document.clone(),
                            phone,
                            address: payer_address,
                        },
                        card: None,
                        wallet: Some(Wallet {
                            name: Some(name),
                            save: Some(true),
                            token: None,
                            capture: Some(true),
                            username: Some(username),
                            email: Some(email),
                        }),
                        order_id,
                        three_dsecure: None,
                        callback_url,
                        description,
                        notification_url: webhook_url,
                    };
                    Ok(payment_request)
                } else {
                    let payment_method_id = get_wallet_payment_method_id(wallet_data)?;
                    let payment_request = Self {
                        amount,
                        currency: item.router_data.request.currency,
                        payment_method_id,
                        payment_method_flow: PaymentMethodFlow::Redirect,
                        country,
                        payer: Payer {
                            name,
                            email,
                            document: document.clone(),
                            phone: None,
                            address: None,
                        },
                        card: None,
                        wallet: None,
                        order_id,
                        three_dsecure: None,
                        callback_url,
                        description,
                        notification_url: webhook_url,
                    };
                    Ok(payment_request)
                }
            }
            // Redirect-flow cash vouchers (e.g. OXXO, Boleto, Efecty, PagoEfectivo,
            // RedPagos, Indomaret).
            PaymentMethodData::Voucher(ref voucher_data) => {
                let payment_method_id = get_voucher_payment_method_id(voucher_data)?;
                let webhook_url = item.router_data.request.webhook_url.clone();
                let payment_request = Self {
                    amount,
                    currency: item.router_data.request.currency,
                    payment_method_id,
                    payment_method_flow: PaymentMethodFlow::Redirect,
                    country,
                    payer: Payer {
                        name,
                        email,
                        document: document.clone(),
                        phone: None,
                        address: None,
                    },
                    card: None,
                    wallet: None,
                    order_id,
                    three_dsecure: None,
                    callback_url,
                    description,
                    notification_url: webhook_url,
                };
                Ok(payment_request)
            }
            // Redirect-flow bank-transfer / real-time APMs (e.g. PSE, PIX).
            PaymentMethodData::BankTransfer(ref bank_transfer_data) => {
                let payment_method_id =
                    get_bank_transfer_redirect_payment_method_id(bank_transfer_data)?;
                let webhook_url = item.router_data.request.webhook_url.clone();
                let payment_request = Self {
                    amount,
                    currency: item.router_data.request.currency,
                    payment_method_id,
                    payment_method_flow: PaymentMethodFlow::Redirect,
                    country,
                    payer: Payer {
                        name,
                        email,
                        document: document.clone(),
                        phone: None,
                        address: None,
                    },
                    card: None,
                    wallet: None,
                    order_id,
                    three_dsecure: None,
                    callback_url,
                    description,
                    notification_url: webhook_url,
                };
                Ok(payment_request)
            }
            PaymentMethodData::CardRedirect(_)
            | PaymentMethodData::PayLater(_)
            | PaymentMethodData::BankRedirect(_)
            | PaymentMethodData::Crypto(_)
            | PaymentMethodData::MandatePayment
            | PaymentMethodData::Reward
            | PaymentMethodData::RealTimePayment(_)
            | PaymentMethodData::MobilePayment(_)
            | PaymentMethodData::Upi(_)
            | PaymentMethodData::GiftCard(_)
            | PaymentMethodData::OpenBanking(_)
            | PaymentMethodData::PaymentMethodToken(_)
            | PaymentMethodData::NetworkToken(_)
            | PaymentMethodData::DecryptedWalletTokenDetailsForNetworkTransactionId(_)
            | PaymentMethodData::CardDetailsForNetworkTransactionId(_) => {
                Err(IntegrationError::NotImplemented(
                    crate::utils::get_unimplemented_payment_method_error_message("Dlocal"),
                    Default::default(),
                ))?
            }
        }
    }
}

#[derive(Default, Debug, Serialize, PartialEq)]
pub struct DlocalPaymentsCaptureRequest {
    pub authorization_id: Secret<String>,
    pub amount: FloatMajorUnit,
    pub currency: String,
    pub order_id: String,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        DlocalRouterData<
            RouterDataV2<
                connector_flow::Capture,
                PaymentFlowData,
                PaymentsCaptureData,
                PaymentsResponseData,
            >,
            T,
        >,
    > for DlocalPaymentsCaptureRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: DlocalRouterData<
            RouterDataV2<
                connector_flow::Capture,
                PaymentFlowData,
                PaymentsCaptureData,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let amount = utils::convert_amount(
            item.connector.amount_converter,
            item.router_data.request.minor_amount_to_capture,
            item.router_data.request.currency,
        )?;

        Ok(Self {
            authorization_id: Secret::new(
                item.router_data
                    .request
                    .connector_transaction_id
                    .get_connector_transaction_id()
                    .change_context(IntegrationError::MissingConnectorTransactionID {
                        context: Default::default(),
                    })?,
            ),
            amount,
            currency: item.router_data.request.currency.to_string(),
            order_id: item
                .router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
        })
    }
}
// RepeatPayment (MIT) flow types

#[derive(Debug, Serialize, Clone, Eq, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum StoredCredentialType {
    CardOnFile,
    Subscription,
    UnscheduledCardOnFile,
    Installments,
}

impl From<Option<common_enums::MitCategory>> for StoredCredentialType {
    fn from(category: Option<common_enums::MitCategory>) -> Self {
        match category {
            Some(common_enums::MitCategory::Recurring) => Self::Subscription,
            Some(common_enums::MitCategory::Installment) => Self::Installments,
            Some(common_enums::MitCategory::Unscheduled) => Self::UnscheduledCardOnFile,
            Some(common_enums::MitCategory::Resubmission) => Self::UnscheduledCardOnFile,
            None => Self::CardOnFile,
        }
    }
}

#[derive(Debug, Serialize, Clone, Eq, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum StoredCredentialUsage {
    Used,
}

#[derive(Debug, Serialize)]
pub struct DlocalRepeatPaymentCard {
    pub card_id: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capture: Option<String>,
    pub stored_credential_type: StoredCredentialType,
    pub stored_credential_usage: StoredCredentialUsage,
}

#[derive(Debug, Serialize)]
pub struct DlocalRepeatPaymentRequest {
    pub amount: FloatMajorUnit,
    pub currency: common_enums::Currency,
    pub country: common_enums::CountryAlpha2,
    pub payment_method_id: PaymentMethodId,
    pub payment_method_flow: PaymentMethodFlow,
    pub payer: Payer,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card: Option<DlocalRepeatPaymentCard>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wallet: Option<Wallet>,
    pub order_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notification_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        DlocalRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for DlocalRepeatPaymentRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: DlocalRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;

        // Extract the connector_mandate_id. For card MIT this is the saved
        // `card_id`; for GCash Recurring (RG) MIT it is the wallet `token`
        // delivered earlier via the IPN webhook during the CIT enrollment.
        let connector_mandate_id = match &router_data.request.mandate_reference {
            MandateReferenceId::ConnectorMandateId(connector_mandate_ref) => connector_mandate_ref
                .get_connector_mandate_id()
                .ok_or(IntegrationError::MissingRequiredField {
                    field_name: "connector_mandate_id",
                    context: Default::default(),
                })?,
            MandateReferenceId::NetworkMandateId(_)
            | MandateReferenceId::NetworkTokenWithNTI(_) => {
                Err(error_stack::report!(IntegrationError::NotSupported {
                    message: "Network mandate ID not supported for repeat payments in dlocal"
                        .to_string(),
                    connector: "Dlocal",
                    context: Default::default(),
                }))?
            }
        };

        let address = router_data.resource_common_data.get_billing_address()?;
        let country = *address.get_country()?;
        let name = address.get_full_name()?;

        let email = router_data.request.get_email()?;

        let amount = utils::convert_amount(
            item.connector.amount_converter,
            router_data.request.minor_amount,
            router_data.request.currency,
        )?;

        let order_id = router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();

        let should_capture = matches!(
            router_data.request.capture_method,
            Some(common_enums::CaptureMethod::Automatic)
                | Some(common_enums::CaptureMethod::SequentialAutomatic)
                | None
        );

        let document = router_data
            .request
            .get_customer_document_details()?
            .document_number;

        // GCash Recurring (RG) MIT: replay the saved wallet token instead of a
        // card. dLocal still requires payer phone + address for RG. We detect
        // the GCash flow from either the explicit `payment_method_type` or the
        // `payment_method_data` (a GCash wallet), since callers may supply
        // either when replaying a wallet-token mandate.
        let is_gcash = matches!(
            router_data.request.payment_method_type,
            Some(common_enums::PaymentMethodType::Gcash)
        ) || matches!(
            router_data.request.payment_method_data,
            PaymentMethodData::Wallet(payment_method_data::WalletData::GcashRedirect(_))
        );
        if is_gcash {
            let billing = router_data.resource_common_data.get_optional_billing();
            let (phone, payer_address) = build_payer_phone_and_address(billing);
            let username = wallet_username(&name, &email);

            return Ok(Self {
                amount,
                currency: router_data.request.currency,
                country,
                payment_method_id: PaymentMethodId::Other("RG".to_string()),
                payment_method_flow: PaymentMethodFlow::Redirect,
                payer: Payer {
                    name: name.clone(),
                    email: email.clone(),
                    document,
                    phone,
                    address: payer_address,
                },
                card: None,
                wallet: Some(Wallet {
                    name: None,
                    save: None,
                    token: Some(Secret::new(connector_mandate_id)),
                    capture: Some(should_capture),
                    username: Some(username),
                    email: Some(email),
                }),
                order_id,
                notification_url: router_data.request.webhook_url.clone(),
                description: router_data.resource_common_data.description.clone(),
            });
        }

        // Default: card-on-file MIT using the saved card_id.
        let stored_credential_type =
            StoredCredentialType::from(router_data.request.mit_category.clone());

        Ok(Self {
            amount,
            currency: router_data.request.currency,
            country,
            payment_method_id: PaymentMethodId::Card,
            payment_method_flow: PaymentMethodFlow::Direct,
            payer: Payer {
                name,
                email,
                document,
                phone: None,
                address: None,
            },
            card: Some(DlocalRepeatPaymentCard {
                card_id: Secret::new(connector_mandate_id),
                capture: Some(should_capture.to_string()),
                stored_credential_type,
                stored_credential_usage: StoredCredentialUsage::Used,
            }),
            wallet: None,
            order_id,
            notification_url: router_data.request.webhook_url.clone(),
            description: router_data.resource_common_data.description.clone(),
        })
    }
}

// RepeatPayment response - reuses DlocalPaymentsResponse (generic TryFrom covers this)
pub type DlocalRepeatPaymentResponse = DlocalPaymentsResponse;

// SetupMandate (CIT) flow: tokenize a card by sending a zero-amount payment with
// `card.save=true`. dLocal's response includes a `card` object containing the
// saved `card_id` which is later used in RepeatPayment (MIT) flow.

#[derive(Debug, Serialize)]
pub struct DlocalSetupMandateRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    pub amount: FloatMajorUnit,
    pub currency: common_enums::Currency,
    pub country: common_enums::CountryAlpha2,
    pub payment_method_id: PaymentMethodId,
    pub payment_method_flow: PaymentMethodFlow,
    pub payer: Payer,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card: Option<Card<T>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wallet: Option<Wallet>,
    pub order_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub three_dsecure: Option<ThreeDSecureReqData>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub callback_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notification_url: Option<String>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        DlocalRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for DlocalSetupMandateRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: DlocalRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;

        let email = router_data.request.get_email()?;
        let address = router_data.resource_common_data.get_billing_address()?;
        let country = *address.get_country()?;
        let name = address.get_full_name()?;

        // For SetupMandate (CIT), dLocal requires a non-zero authorization amount
        // alongside `card.save = true` to tokenize the card. dLocal rejects
        // amounts <= 1.00 with code 5016 "Amount too low", so callers must
        // provide an appropriate verify amount (e.g. 5.00 BRL). The request
        // runs with `capture: false` so funds are released immediately after
        // the authorization.
        let minor_amount =
            router_data
                .request
                .minor_amount
                .ok_or(IntegrationError::MissingRequiredField {
                    field_name: "minor_amount",
                    context: Default::default(),
                })?;
        let amount = utils::convert_amount(
            item.connector.amount_converter,
            minor_amount,
            router_data.request.currency,
        )?;

        let order_id = router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();
        let callback_url = router_data.request.router_return_url.clone();
        let description = router_data.resource_common_data.description.clone();

        let document = router_data
            .request
            .get_customer_document_details()?
            .document_number;

        match &router_data.request.payment_method_data {
            PaymentMethodData::Card(ccard) => Ok(Self {
                amount,
                currency: router_data.request.currency,
                payment_method_id: PaymentMethodId::Card,
                payment_method_flow: PaymentMethodFlow::Direct,
                country,
                payer: Payer {
                    name,
                    email,
                    document,
                    phone: None,
                    address: None,
                },
                card: Some(Card {
                    holder_name: ccard.card_holder_name.clone(),
                    number: ccard.card_number.clone(),
                    cvv: ccard.card_cvc.clone(),
                    expiration_month: ccard.card_exp_month.clone(),
                    expiration_year: ccard.card_exp_year.clone(),
                    // Setup mandate is always a verify/no-capture operation
                    capture: "false".to_string(),
                    save: Some(true),
                }),
                wallet: None,
                order_id,
                three_dsecure: None,
                callback_url,
                description,
                notification_url: router_data.request.webhook_url.clone(),
            }),
            // CIT — GCash Recurring (RG) wallet enrollment. We send a REDIRECT
            // payment with `wallet.save = true`; dLocal returns PENDING + a
            // redirect_url for the shopper to authorise. The reusable wallet
            // token is NOT in this synchronous response — dLocal delivers it
            // later via the IPN webhook (`wallet.token`) once the shopper
            // completes the redirect. The IncomingWebhook handler is therefore
            // responsible for capturing that token and persisting it as the
            // mandate reference for the downstream RepeatPayment (MIT) flow;
            // `mandate_reference` is left `None` here in the sync mapping.
            PaymentMethodData::Wallet(payment_method_data::WalletData::GcashRedirect(_)) => {
                let billing = router_data.resource_common_data.get_optional_billing();
                let (phone, payer_address) = build_payer_phone_and_address(billing);
                let username = wallet_username(&name, &email);
                Ok(Self {
                    amount,
                    currency: router_data.request.currency,
                    payment_method_id: PaymentMethodId::Other("RG".to_string()),
                    payment_method_flow: PaymentMethodFlow::Redirect,
                    country,
                    payer: Payer {
                        name: name.clone(),
                        email: email.clone(),
                        document,
                        phone,
                        address: payer_address,
                    },
                    card: None,
                    wallet: Some(Wallet {
                        name: Some(name),
                        save: Some(true),
                        token: None,
                        capture: Some(true),
                        username: Some(username),
                        email: Some(email),
                    }),
                    order_id,
                    three_dsecure: None,
                    callback_url,
                    description,
                    notification_url: router_data.request.webhook_url.clone(),
                })
            }
            _ => Err(error_stack::report!(IntegrationError::NotSupported {
                message: crate::utils::get_unimplemented_payment_method_error_message("Dlocal"),
                connector: "Dlocal",
                context: Default::default(),
            }))?,
        }
    }
}

// SetupMandate response — adds a `card` object with the saved `card_id` on top of
// the standard payment response fields. Per dLocal's Save-Card API the saved token
// is returned as `card.card_id` (same field consumed by the RepeatPayment/MIT flow
// above, see `DlocalRepeatPaymentCard`).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DlocalSetupMandateCardData {
    pub card_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DlocalSetupMandateResponse {
    pub status: DlocalPaymentStatus,
    pub id: String,
    pub three_dsecure: Option<ThreeDSecureResData>,
    pub order_id: Option<String>,
    pub redirect_url: Option<url::Url>,
    pub card: Option<DlocalSetupMandateCardData>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<DlocalSetupMandateResponse, Self>>
    for RouterDataV2<
        SetupMandate,
        PaymentFlowData,
        SetupMandateRequestData<T>,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<DlocalSetupMandateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // For failed payments (Rejected/Cancelled), return ErrorResponse instead
        // of TransactionResponse to capture the failure details.
        if matches!(
            item.response.status,
            DlocalPaymentStatus::Rejected | DlocalPaymentStatus::Cancelled
        ) {
            return Ok(Self {
                resource_common_data: PaymentFlowData {
                    status: common_enums::AttemptStatus::from(item.response.status.clone()),
                    ..item.router_data.resource_common_data
                },
                response: Err(ErrorResponse {
                    code: format!("{:?}", item.response.status),
                    message: format!(
                        "SetupMandate failed with status: {:?}",
                        item.response.status
                    ),
                    reason: Some(format!(
                        "dLocal returned {:?} status for SetupMandate request",
                        item.response.status
                    )),
                    status_code: item.http_code,
                    attempt_status: Some(common_enums::AttemptStatus::from(
                        item.response.status.clone(),
                    )),
                    connector_transaction_id: Some(item.response.id.clone()),
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                }),
                ..item.router_data
            });
        }

        let redirection_data = item
            .response
            .three_dsecure
            .and_then(|three_secure_data| three_secure_data.redirect_url)
            .or(item.response.redirect_url)
            .map(|redirect_url| RedirectForm::from((redirect_url, Method::Get)));

        // Extract the saved card identifier from the response. Per dLocal's
        // Save-Card API contract the token is returned on `card.card_id`.
        let saved_card_id = item.response.card.as_ref().and_then(|c| c.card_id.clone());

        // SetupMandate only succeeds if dLocal hands us a tokenised card_id —
        // otherwise the downstream RepeatPayment (MIT) flow has nothing to
        // reference. Fail fast on AUTHORIZED / PAID responses that are missing
        // the token rather than silently completing with `mandate_reference = None`.
        let is_successful = matches!(
            item.response.status,
            DlocalPaymentStatus::Authorized | DlocalPaymentStatus::Paid
        );
        if is_successful && saved_card_id.is_none() {
            return Err(ConnectorError::UnexpectedResponseError {
                context: ResponseTransformationErrorContext {
                    http_status_code: Some(item.http_code),
                    additional_context: Some(
                        "dLocal SetupMandate succeeded but response is missing `card.card_id` — \
                         cannot build MandateReference for downstream RepeatPayment (MIT) flow."
                            .to_string(),
                    ),
                },
            }
            .into());
        }

        let mandate_reference = saved_card_id.map(|card_id| MandateReference {
            connector_mandate_id: Some(card_id),
            payment_method_id: None,
            connector_mandate_request_reference_id: None,
        });

        // A redirect-flow APM (e.g. GCash) returns PENDING + a redirect_url; surface it as
        // AuthenticationPending so HS emits next_action.redirect_to_url rather than just
        // "processing".
        let status = if redirection_data.is_some() {
            common_enums::AttemptStatus::AuthenticationPending
        } else {
            common_enums::AttemptStatus::from(item.response.status.clone())
        };
        let response = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(item.response.id.clone()),
            redirection_data: redirection_data.map(Box::new),
            mandate_reference: mandate_reference.map(Box::new),
            connector_metadata: None,
            network_txn_id: None,
            connector_response_reference_id: item.response.order_id.clone(),
            incremental_authorization_allowed: None,
            status_code: item.http_code,
        };

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            response: Ok(response),
            ..item.router_data
        })
    }
}

// Auth Struct
pub struct DlocalAuthType {
    pub(super) x_login: Secret<String>,
    pub(super) x_trans_key: Secret<String>,
    pub(super) secret: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for DlocalAuthType {
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        if let ConnectorSpecificConfig::Dlocal {
            x_login,
            x_trans_key,
            secret,
            ..
        } = auth_type
        {
            Ok(Self {
                x_login: x_login.to_owned(),
                x_trans_key: x_trans_key.to_owned(),
                secret: secret.to_owned(),
            })
        } else {
            Err(IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            }
            .into())
        }
    }
}
#[derive(Debug, Clone, Eq, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
pub enum DlocalPaymentStatus {
    Authorized,
    Paid,
    Cancelled,
    #[default]
    Pending,
    Rejected,
}

impl From<DlocalPaymentStatus> for common_enums::AttemptStatus {
    fn from(item: DlocalPaymentStatus) -> Self {
        match item {
            DlocalPaymentStatus::Authorized => Self::Authorized,
            DlocalPaymentStatus::Paid => Self::Charged,
            DlocalPaymentStatus::Pending => Self::Pending,
            DlocalPaymentStatus::Cancelled => Self::Voided,
            DlocalPaymentStatus::Rejected => Self::Failure,
        }
    }
}

/// Wallet object echoed back in the dLocal IPN webhook (and payment responses).
///
/// For recurring wallet flows (e.g. GCash Recurring "RG"), the reusable
/// `token` is NOT returned in the synchronous Authorize/CIT response — it only
/// arrives here, in the IPN, once the shopper completes the redirect. We surface
/// it as a mandate reference so Hyperswitch can persist the connector mandate id.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct DlocalWebhookWallet {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<pii::Email>,
    /// Reusable wallet token (the connector mandate id) for recurring flows.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token: Option<Secret<String>>,
}

/// dLocal IPN (Instant Payment Notification) webhook body.
///
/// The IPN payload is the dLocal Payment object as JSON. We only deserialize the
/// fields needed to build the PSync-shaped status response (status, ids) plus the
/// optional `wallet.token` for mandate persistence; every other field is ignored.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct DlocalWebhookBody {
    /// Connector transaction id (the dLocal payment `id`, e.g. "F-...").
    pub id: String,
    pub status: DlocalPaymentStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_detail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_code: Option<serde_json::Value>,
    /// Merchant-assigned reference echoed back by dLocal.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payment_method_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payment_method_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payment_method_flow: Option<String>,
    /// Present for wallet flows; carries the reusable `token` (mandate id).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wallet: Option<DlocalWebhookWallet>,
}

/// Maps a parsed dLocal IPN status to the UCS webhook `EventType`.
///
/// Mirrors the `DlocalPaymentStatus -> AttemptStatus` mapping used by the sync
/// flows, then projects each terminal/intermediate status onto the payment-intent
/// webhook event the EventService expects.
impl From<&DlocalWebhookBody> for domain_types::connector_types::EventType {
    fn from(body: &DlocalWebhookBody) -> Self {
        use domain_types::connector_types::EventType;
        match body.status {
            // PAID(200) -> charged
            DlocalPaymentStatus::Paid => EventType::PaymentIntentSuccess,
            // AUTHORIZED -> funds held, capture pending
            DlocalPaymentStatus::Authorized => EventType::PaymentIntentAuthorizationSuccess,
            // PENDING(100) -> still processing
            DlocalPaymentStatus::Pending => EventType::PaymentIntentProcessing,
            // CANCELLED -> voided
            DlocalPaymentStatus::Cancelled => EventType::PaymentIntentCancelled,
            // REJECTED(300) -> failure
            DlocalPaymentStatus::Rejected => EventType::PaymentIntentFailure,
        }
    }
}

#[derive(Eq, Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ThreeDSecureResData {
    pub redirect_url: Option<url::Url>,
}

#[derive(Debug, Default, Eq, Clone, PartialEq, Serialize, Deserialize)]
pub struct DlocalPaymentsResponse {
    status: DlocalPaymentStatus,
    id: String,
    three_dsecure: Option<ThreeDSecureResData>,
    order_id: Option<String>,
    redirect_url: Option<url::Url>,
}

impl<F, T> TryFrom<ResponseRouterData<DlocalPaymentsResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, T, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<DlocalPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // Check for redirect URL from both 3DS (card) and top-level (bank transfer/APM) responses
        let redirection_data = item
            .response
            .three_dsecure
            .and_then(|three_secure_data| three_secure_data.redirect_url)
            .or(item.response.redirect_url)
            .map(|redirect_url| RedirectForm::from((redirect_url, Method::Get)));

        // A redirect-flow APM (e.g. GCash) returns PENDING + a redirect_url; surface it as
        // AuthenticationPending so HS emits next_action.redirect_to_url rather than just
        // "processing".
        let status = if redirection_data.is_some() {
            common_enums::AttemptStatus::AuthenticationPending
        } else {
            common_enums::AttemptStatus::from(item.response.status.clone())
        };
        let response = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(item.response.id.clone()),
            redirection_data: redirection_data.map(Box::new),
            mandate_reference: None,
            connector_metadata: None,
            network_txn_id: None,
            connector_response_reference_id: item.response.order_id.clone(),
            incremental_authorization_allowed: None,
            status_code: item.http_code,
        };
        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            response: Ok(response),
            ..item.router_data
        })
    }
}

#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DlocalPaymentsSyncResponse {
    status: DlocalPaymentStatus,
    id: String,
    order_id: Option<String>,
}

impl<F> TryFrom<ResponseRouterData<DlocalPaymentsSyncResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<DlocalPaymentsSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            resource_common_data: PaymentFlowData {
                status: common_enums::AttemptStatus::from(item.response.status),
                ..item.router_data.resource_common_data
            },
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.id.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: item.response.order_id.clone(),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DlocalPaymentsCaptureResponse {
    status: DlocalPaymentStatus,
    id: String,
    order_id: Option<String>,
}

impl<F> TryFrom<ResponseRouterData<DlocalPaymentsCaptureResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<DlocalPaymentsCaptureResponse, Self>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            resource_common_data: PaymentFlowData {
                status: common_enums::AttemptStatus::from(item.response.status),
                ..item.router_data.resource_common_data
            },
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.id.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: item.response.order_id.clone(),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

pub struct DlocalPaymentsCancelResponse {
    status: DlocalPaymentStatus,
    order_id: String,
}

impl<F> TryFrom<ResponseRouterData<DlocalPaymentsCancelResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<DlocalPaymentsCancelResponse, Self>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            resource_common_data: PaymentFlowData {
                status: common_enums::AttemptStatus::from(item.response.status),
                ..item.router_data.resource_common_data
            },
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.order_id.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: Some(item.response.order_id.clone()),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

// REFUND :
#[derive(Default, Debug, Serialize)]
pub struct DlocalRefundRequest {
    pub amount: FloatMajorUnit,
    pub payment_id: String,
    pub currency: common_enums::Currency,
    pub id: String,
}

impl<F, T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<DlocalRouterData<RouterDataV2<F, RefundFlowData, RefundsData, RefundsResponseData>, T>>
    for DlocalRefundRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: DlocalRouterData<
            RouterDataV2<F, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let amount_to_refund = utils::convert_amount(
            item.connector.amount_converter,
            item.router_data.request.minor_refund_amount,
            item.router_data.request.currency,
        )?;

        Ok(Self {
            amount: amount_to_refund,
            payment_id: item.router_data.request.connector_transaction_id.clone(),
            currency: item.router_data.request.currency,
            id: item.router_data.request.refund_id.clone(),
        })
    }
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Default, Deserialize, Clone)]
#[serde(rename_all = "UPPERCASE")]
pub enum RefundStatus {
    Success,
    #[default]
    Pending,
    Rejected,
    Cancelled,
}

impl From<RefundStatus> for common_enums::RefundStatus {
    fn from(item: RefundStatus) -> Self {
        match item {
            RefundStatus::Success => Self::Success,
            RefundStatus::Pending => Self::Pending,
            RefundStatus::Rejected | RefundStatus::Cancelled => Self::Failure,
        }
    }
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct RefundResponse {
    pub id: String,
    pub status: RefundStatus,
}

impl<F> TryFrom<ResponseRouterData<RefundResponse, Self>>
    for RouterDataV2<F, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(item: ResponseRouterData<RefundResponse, Self>) -> Result<Self, Self::Error> {
        let refund_status = common_enums::RefundStatus::from(item.response.status);
        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: item.response.id,
                refund_status,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

impl<F> TryFrom<ResponseRouterData<RefundResponse, Self>>
    for RouterDataV2<F, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(item: ResponseRouterData<RefundResponse, Self>) -> Result<Self, Self::Error> {
        let refund_status = common_enums::RefundStatus::from(item.response.status);
        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: item.response.id,
                refund_status,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

#[derive(Default, Debug, Serialize, Deserialize, PartialEq)]
pub struct DlocalErrorResponse {
    pub code: i32,
    pub message: String,
    pub param: Option<String>,
}

/// Maps BankDebitData and country to the dLocal-specific payment_method_id.
/// dLocal uses country-specific payment method codes for bank transfer / direct debit payments.
/// For REDIRECT flow bank transfers, the payment_method_id depends on the country:
///   - BR: "IO" (Bank Transfer)
///   - AR: "IO" (Bank Transfer)
///   - MX: "SE" (SPEI)
///   - CL: "WP" (Webpay)
///   - CO: "PC" (PSE)
///   - PE: "BC" (BCP)
///
/// We use well-known codes per country from dLocal's payment methods API.
fn get_bank_debit_payment_method_id(
    bank_debit_data: &payment_method_data::BankDebitData,
    country: common_enums::CountryAlpha2,
) -> Result<(PaymentMethodId, Option<String>), error_stack::Report<IntegrationError>> {
    match bank_debit_data {
        payment_method_data::BankDebitData::SepaBankDebit { .. }
        | payment_method_data::BankDebitData::SepaGuaranteedBankDebit { .. }
        | payment_method_data::BankDebitData::AchBankDebit { .. } => {
            let method_id = get_bank_transfer_method_id_for_country(country)?;
            Ok((PaymentMethodId::Other(method_id), None))
        }
        payment_method_data::BankDebitData::BecsBankDebit { .. }
        | payment_method_data::BankDebitData::EftBankDebit { .. }
        | payment_method_data::BankDebitData::BacsBankDebit { .. } => {
            Err(error_stack::report!(IntegrationError::NotSupported {
                message: crate::utils::get_unimplemented_payment_method_error_message("Dlocal"),
                connector: "Dlocal",
                context: Default::default(),
            }))?
        }
    }
}

/// Maps a redirect-flow `WalletData` variant to the dLocal APM `payment_method_id`.
///
/// dLocal hosts the wallet UX; the orchestrator only needs to send the correct
/// `payment_method_id` (plus `payment_method_flow = REDIRECT`). Country/currency
/// come from the request, so they are not encoded here.
///   - GcashRedirect -> "GC" (GCash, PH)
///   - DanaRedirect   -> "DN" (DANA, ID)
fn get_wallet_payment_method_id(
    wallet_data: &payment_method_data::WalletData,
) -> Result<PaymentMethodId, error_stack::Report<IntegrationError>> {
    match wallet_data {
        payment_method_data::WalletData::GcashRedirect(_) => {
            Ok(PaymentMethodId::Other("GC".to_string()))
        }
        payment_method_data::WalletData::DanaRedirect {} => {
            Ok(PaymentMethodId::Other("DN".to_string()))
        }
        _ => Err(IntegrationError::NotImplemented(
            crate::utils::get_unimplemented_payment_method_error_message("Dlocal"),
            Default::default(),
        ))?,
    }
}

/// Maps a redirect-flow `VoucherData` variant to the dLocal cash-voucher
/// `payment_method_id`.
///   - Oxxo        -> "OX" (MX)
///   - Boleto      -> "BL" (BR)
///   - Efecty      -> "EY" (CO)
///   - PagoEfectivo-> "EF" (PE)
///   - RedPagos    -> "RE" (UY)
///   - Indomaret   -> "IM" (ID)
fn get_voucher_payment_method_id(
    voucher_data: &payment_method_data::VoucherData,
) -> Result<PaymentMethodId, error_stack::Report<IntegrationError>> {
    match voucher_data {
        payment_method_data::VoucherData::Oxxo => Ok(PaymentMethodId::Other("OX".to_string())),
        payment_method_data::VoucherData::Boleto(_) => Ok(PaymentMethodId::Other("BL".to_string())),
        payment_method_data::VoucherData::Efecty => Ok(PaymentMethodId::Other("EY".to_string())),
        payment_method_data::VoucherData::PagoEfectivo => {
            Ok(PaymentMethodId::Other("EF".to_string()))
        }
        payment_method_data::VoucherData::RedPagos => Ok(PaymentMethodId::Other("RE".to_string())),
        payment_method_data::VoucherData::Indomaret(_) => {
            Ok(PaymentMethodId::Other("IM".to_string()))
        }
        _ => Err(IntegrationError::NotImplemented(
            crate::utils::get_unimplemented_payment_method_error_message("Dlocal"),
            Default::default(),
        ))?,
    }
}

/// Maps a redirect-flow `BankTransferData` variant to the dLocal bank-redirect /
/// real-time `payment_method_id`.
///   - Pse -> "PC" (PSE, CO)
///   - Pix -> "PQ" (PIX redirect, BR)
fn get_bank_transfer_redirect_payment_method_id(
    bank_transfer_data: &payment_method_data::BankTransferData,
) -> Result<PaymentMethodId, error_stack::Report<IntegrationError>> {
    match bank_transfer_data {
        payment_method_data::BankTransferData::Pse {} => {
            Ok(PaymentMethodId::Other("PC".to_string()))
        }
        payment_method_data::BankTransferData::Pix { .. } => {
            Ok(PaymentMethodId::Other("PQ".to_string()))
        }
        _ => Err(IntegrationError::NotImplemented(
            crate::utils::get_unimplemented_payment_method_error_message("Dlocal"),
            Default::default(),
        ))?,
    }
}

/// Returns a well-known bank transfer payment_method_id for the given country.
fn get_bank_transfer_method_id_for_country(
    country: common_enums::CountryAlpha2,
) -> Result<String, error_stack::Report<IntegrationError>> {
    match country {
        common_enums::CountryAlpha2::BR => Ok("IO".to_string()), // Bank Transfer
        common_enums::CountryAlpha2::AR => Ok("IO".to_string()), // Bank Transfer
        common_enums::CountryAlpha2::MX => Ok("SE".to_string()), // SPEI
        common_enums::CountryAlpha2::CL => Ok("WP".to_string()), // Webpay
        common_enums::CountryAlpha2::CO => Ok("PC".to_string()), // PSE
        common_enums::CountryAlpha2::PE => Ok("BC".to_string()), // BCP
        _ => Err(IntegrationError::NotSupported {
            message: format!("Bank debit is not supported for country: {country}"),
            connector: "Dlocal",
            context: Default::default(),
        }
        .into()),
    }
}
