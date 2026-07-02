use crate::{connectors::mollie::MollieRouterData, types::ResponseRouterData};
use common_utils::{
    pii::Email,
    types::{AmountConvertor, MinorUnit, StringMajorUnit, StringMajorUnitForConnector},
};
use domain_types::errors::{ConnectorError, IntegrationError, IntegrationErrorContext};
use domain_types::{
    connector_flow::{
        Authorize, Capture, ClientAuthenticationToken, CreateConnectorCustomer, PSync,
        PaymentMethodToken, RSync, Refund, SetupMandate, Void,
    },
    connector_types::{
        ClientAuthenticationTokenData, ClientAuthenticationTokenRequestData, ConnectorCustomerData,
        ConnectorCustomerResponse, ConnectorSpecificClientAuthenticationResponse, MandateReference,
        MollieClientAuthenticationResponse as MollieClientAuthenticationResponseDomain,
        PaymentFlowData, PaymentMethodTokenResponse, PaymentMethodTokenizationData,
        PaymentVoidData, PaymentsAuthorizeData, PaymentsCaptureData, PaymentsResponseData,
        PaymentsSyncData, RefundFlowData, RefundSyncData, RefundsData, RefundsResponseData,
        ResponseId, SetupMandateRequestData,
    },
    merchant_authentication_flow_data::MerchantAuthenticationFlowData,
    payment_method_data::{
        BankRedirectData, PayLaterData, PaymentMethodData, PaymentMethodDataTypes, RawCardNumber,
        WalletData,
    },
    router_data::ConnectorSpecificConfig,
    router_data_v2::RouterDataV2,
    router_response_types::RedirectForm,
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, PeekInterface, Secret};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct MollieAuthType {
    pub api_key: Secret<String>,
    pub profile_token: Option<Secret<String>>,
}

impl TryFrom<&ConnectorSpecificConfig> for MollieAuthType {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Mollie {
                api_key,
                profile_token,
                ..
            } => Ok(Self {
                api_key: api_key.to_owned(),
                profile_token: profile_token.to_owned(),
            }),
            _ => Err(error_stack::report!(
                IntegrationError::FailedToObtainAuthType {
                    context: Default::default()
                }
            )),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MollieErrorResponse {
    pub status: u16,
    pub title: String,
    pub detail: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
    #[serde(rename = "_links", skip_serializing_if = "Option::is_none")]
    pub links: Option<serde_json::Value>,
}

// Mollie Amount structure - uses object format with currency and value
#[derive(Debug, Serialize, Deserialize)]
pub struct MollieAmount {
    pub currency: common_enums::Currency,
    pub value: StringMajorUnit,
}

// Mollie Metadata structure - used in payments and refunds
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MollieMetadata {
    pub order_id: String,
}

// Mollie Payment Request structure
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MolliePaymentsRequest {
    pub amount: MollieAmount,
    pub description: String,
    pub redirect_url: String,
    pub webhook_url: String,
    pub metadata: serde_json::Value,
    #[serde(flatten)]
    pub payment_method_data: MolliePaymentMethodData,
    pub sequence_type: SequenceType,
    // captureMode is only accepted by Mollie for `oneoff` payments; it must be
    // omitted for `first` / `recurring` (mandate) sequences.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capture_mode: Option<MollieCaptureMode>,
    // These fields are always null in Hyperswitch but must be present
    pub locale: Option<String>,
    pub cancel_url: Option<String>,
    // `customerId` is required for `first`/`recurring` payments and omitted for
    // one-time payments.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub customer_id: Option<String>,
}

// Mollie Payment Method Data enum
#[derive(Debug, Serialize)]
#[serde(tag = "method")]
#[serde(rename_all = "lowercase")]
pub enum MolliePaymentMethodData {
    #[serde(rename = "creditcard")]
    CreditCard(Box<CreditCardMethodData>),
    Klarna(Box<KlarnaMethodData>),
    /// PayPal wallet via Mollie. Emitted as `"method": "paypal"` (from the
    /// container `rename_all = "lowercase"`). PayPal is a redirect flow — Mollie
    /// returns a checkout URL for the customer to approve the payment. Only the
    /// optional billing/shipping addresses are forwarded; PayPal itself collects
    /// the account details on its hosted page.
    Paypal(Box<PaypalMethodData>),
    /// Apple Pay wallet via Mollie. Emitted as `"method": "applepay"` (from the
    /// container `rename_all = "lowercase"`). Mollie is handed the Apple Pay
    /// payment token string (the decrypted/serialized `PKPaymentToken`) via the
    /// `applePayPaymentToken` field and settles the wallet payment directly, so
    /// no redirect and no address forwarding is required here.
    Applepay(Box<ApplePayMethodData>),
    /// iDeal bank redirect via Mollie. Emitted as `"method": "ideal"` (from the
    /// container `rename_all = "lowercase"`, no magic string). iDeal is a redirect
    /// flow — Mollie returns a checkout URL for the customer to authenticate at
    /// their bank. The issuer bank is optional; when omitted Mollie shows its own
    /// bank picker on the hosted checkout page.
    Ideal(Box<IdealMethodData>),
    // Recurring / Merchant-Initiated charge that references an existing mandate.
    // Serialized untagged so it emits only `mandateId` (no `method`
    // discriminator) — Mollie infers the method from the mandate.
    #[serde(untagged)]
    MandatePayment(Box<MandatePaymentMethodData>),
}

// Mandate (MIT) payment body — carries only the mandate id.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MandatePaymentMethodData {
    pub mandate_id: Secret<String>,
}

// PayPal (wallet) payment body. Mollie collects the PayPal account details on
// its hosted checkout page, so the request only forwards the optional
// billing/shipping addresses for risk assessment.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaypalMethodData {
    pub billing_address: Option<MollieAddress>,
    pub shipping_address: Option<MollieAddress>,
}

// Apple Pay (wallet) payment body. Mollie expects the Apple Pay payment token —
// the decrypted/serialized `PKPaymentToken` produced by the Apple Pay session —
// under `applePayPaymentToken` (from the container `rename_all = "camelCase"`).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplePayMethodData {
    pub apple_pay_payment_token: Secret<String>,
}

// iDeal (bank redirect) payment body. The optional `issuer` selects a specific
// iDeal issuer bank up front; when it is `None` the field is omitted and Mollie
// presents its own bank picker on the hosted checkout page. The issuer is not
// currently sourced from prism's `BankRedirectData::Ideal` request, so it is
// left as `None` (mirrors the reference hyperswitch connector).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IdealMethodData {
    pub issuer: Option<Secret<String>>,
}

// Klarna (PayLater) Method Data
// For Klarna pay-later, Mollie requires a billing address and order lines.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KlarnaMethodData {
    pub billing_address: MollieKlarnaAddress,
    pub lines: Vec<MollieLineItem>,
}

// Mollie Klarna billing address structure
// Klarna requires name + email in addition to the postal address fields.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MollieKlarnaAddress {
    pub street_and_number: Secret<String>,
    pub postal_code: Secret<String>,
    pub city: String,
    pub region: Option<Secret<String>>,
    pub country: common_enums::CountryAlpha2,
    pub given_name: Secret<String>,
    pub family_name: Secret<String>,
    pub email: Email,
}

// Mollie order line item type (Klarna risk assessment).
// Mollie accepts: physical, digital, shipping_fee, discount, store_credit,
// gift_card, surcharge. See https://docs.mollie.com/reference/extra-payment-parameters
#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MollieLineItemType {
    Physical,
    Digital,
    ShippingFee,
    Discount,
    StoreCredit,
    GiftCard,
    Surcharge,
}

// Mollie order line item (required by Klarna)
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MollieLineItem {
    #[serde(rename = "type")]
    pub line_type: MollieLineItemType,
    pub description: String,
    pub quantity: i32,
    pub unit_price: MollieAmount,
    pub total_amount: MollieAmount,
}

// Credit Card Method Data
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreditCardMethodData {
    pub card_token: Option<Secret<String>>,
    pub billing_address: Option<MollieAddress>,
    pub shipping_address: Option<MollieAddress>,
}

// Mollie Address structure
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MollieAddress {
    pub street_and_number: Secret<String>,
    pub postal_code: Secret<String>,
    pub city: String,
    pub region: Option<Secret<String>>,
    pub country: common_enums::CountryAlpha2,
}

// Sequence Type enum
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum SequenceType {
    Oneoff,
    First,
    Recurring,
}

// Capture Mode enum
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "lowercase")]
pub enum MollieCaptureMode {
    Manual,
    Automatic,
}

// Build a Mollie address object from a Hyperswitch address wrapper, matching the
// existing card-payment formatting (comma-joined street/line, no region field).
// Returns `None` when the address or any required postal field is absent so the
// caller can simply omit the address block.
fn build_mollie_address(
    address: Option<&domain_types::payment_address::Address>,
) -> Option<MollieAddress> {
    let address = address?.address.as_ref()?;
    let line1 = address.line1.as_ref()?.peek().to_string();
    let street_and_number = match address.line2.as_ref() {
        Some(line2) => format!("{},{}", line1, line2.peek().as_str()),
        None => line1,
    };
    Some(MollieAddress {
        street_and_number: Secret::new(street_and_number),
        postal_code: Secret::new(address.zip.as_ref()?.peek().to_string()),
        city: address.city.as_ref()?.peek().to_string(),
        region: None,
        country: address.country?,
    })
}

// Map a wallet payment method to the corresponding Mollie payment body. PayPal
// (redirect) and Apple Pay are supported; every other wallet returns
// NotImplemented so the caller surfaces a precise, method-named error.
fn mollie_wallet_payment_method(
    wallet_data: &WalletData,
    billing_address: Option<&domain_types::payment_address::Address>,
    shipping_address: Option<&domain_types::payment_address::Address>,
) -> Result<MolliePaymentMethodData, error_stack::Report<IntegrationError>> {
    match wallet_data {
        WalletData::PaypalRedirect(_) => Ok(MolliePaymentMethodData::Paypal(Box::new(
            PaypalMethodData {
                billing_address: build_mollie_address(billing_address),
                shipping_address: build_mollie_address(shipping_address),
            },
        ))),
        WalletData::ApplePay(applepay_wallet_data) => {
            // Mollie settles Apple Pay directly (no redirect); it only needs the
            // encrypted Apple Pay payment token string. Require it explicitly so a
            // missing token surfaces as a field-named error rather than a silent
            // default.
            let apple_pay_payment_token = applepay_wallet_data
                .payment_data
                .get_encrypted_apple_pay_payment_data_mandatory()
                .change_context(IntegrationError::MissingRequiredField {
                    field_name: "apple_pay_payment_token",
                    context: IntegrationErrorContext {
                        additional_context: Some(
                            "Mollie Apple Pay requires the encrypted Apple Pay payment token (applePayPaymentToken)."
                                .to_string(),
                        ),
                        ..Default::default()
                    },
                })?
                .to_owned();
            Ok(MolliePaymentMethodData::Applepay(Box::new(
                ApplePayMethodData {
                    apple_pay_payment_token: Secret::new(apple_pay_payment_token),
                },
            )))
        }
        _ => Err(IntegrationError::NotImplemented(
            "Selected wallet payment method is not implemented for Mollie".to_string(),
            Default::default(),
        )
        .into()),
    }
}

// Map a bank-redirect payment method to the corresponding Mollie payment body.
// iDeal is supported; every other bank redirect returns a method-named
// NotImplemented (segregated from NotSupported) so the caller surfaces a precise
// error. This helper mirrors `mollie_wallet_payment_method` and is the extension
// point for the later Sofort/Bancontact/EPS/Giropay/Przelewy24 tasks.
fn mollie_bank_redirect_payment_method(
    bank_redirect_data: &BankRedirectData,
) -> Result<MolliePaymentMethodData, error_stack::Report<IntegrationError>> {
    match bank_redirect_data {
        // iDeal redirect flow. The issuer bank is optional and not currently
        // sourced from the request, so it is omitted (`issuer: None`) and Mollie
        // shows its own bank picker — matches the reference hyperswitch connector.
        // Fields are intentionally ignored (`{ .. }`) because no issuer is sourced.
        BankRedirectData::Ideal { .. } => Ok(MolliePaymentMethodData::Ideal(Box::new(
            IdealMethodData { issuer: None },
        ))),
        // Bank redirects Mollie supports but which are not built yet — name the
        // attempted method so the error is precise and actionable.
        BankRedirectData::Sofort { .. } => Err(IntegrationError::NotImplemented(
            "sofort bank redirect is not yet implemented for Mollie".to_string(),
            Default::default(),
        )
        .into()),
        BankRedirectData::BancontactCard { .. } => Err(IntegrationError::NotImplemented(
            "bancontact bank redirect is not yet implemented for Mollie".to_string(),
            Default::default(),
        )
        .into()),
        BankRedirectData::Eps { .. } => Err(IntegrationError::NotImplemented(
            "eps bank redirect is not yet implemented for Mollie".to_string(),
            Default::default(),
        )
        .into()),
        BankRedirectData::Giropay { .. } => Err(IntegrationError::NotImplemented(
            "giropay bank redirect is not yet implemented for Mollie".to_string(),
            Default::default(),
        )
        .into()),
        BankRedirectData::Przelewy24 { .. } => Err(IntegrationError::NotImplemented(
            "przelewy24 bank redirect is not yet implemented for Mollie".to_string(),
            Default::default(),
        )
        .into()),
        // Any other bank redirect is not implemented for Mollie.
        _ => Err(IntegrationError::NotImplemented(
            "Selected bank redirect payment method is not implemented for Mollie".to_string(),
            Default::default(),
        )
        .into()),
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        MollieRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for MolliePaymentsRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: MollieRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let item = item.router_data;
        // Convert amount to string major unit format (e.g., "10.00" for $10.00)
        let converter = StringMajorUnitForConnector;
        let amount_value = converter
            .convert(item.request.amount, item.request.currency)
            .change_context(IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            })
            .attach_printable("Failed to convert amount to string major unit")?;

        // Extract payment method data based on payment method type
        let payment_method_data = match &item.request.payment_method_data {
            PaymentMethodData::PaymentMethodToken(t) => {
                MolliePaymentMethodData::CreditCard(Box::new(CreditCardMethodData {
                    card_token: Some(t.token.clone()),
                    billing_address: None,
                    shipping_address: None,
                }))
            }
            PaymentMethodData::Card(_card_data) => {
                let card_token = None;

                // Extract billing address if available
                // Match Hyperswitch format: comma separator, no region
                let billing_address = item
                    .resource_common_data
                    .address
                    .get_payment_method_billing()
                    .and_then(|billing| {
                        let address = billing.address.as_ref()?;
                        let line1 = address.line1.as_ref()?.peek().to_string();
                        let street_and_number = match address.line2.as_ref() {
                            Some(line2) => format!("{},{}", line1, line2.peek().as_str()),
                            None => line1,
                        };

                        Some(MollieAddress {
                            street_and_number: Secret::new(street_and_number),
                            postal_code: Secret::new(address.zip.as_ref()?.peek().to_string()),
                            city: address.city.as_ref()?.peek().to_string(),
                            region: None, // Match Hyperswitch: always null
                            country: address.country?,
                        })
                    });

                MolliePaymentMethodData::CreditCard(Box::new(CreditCardMethodData {
                    card_token,
                    billing_address,
                    shipping_address: None,
                }))
            }
            PaymentMethodData::PayLater(PayLaterData::KlarnaRedirect {}) => {
                // Klarna pay-later requires a billing address (name + email + postal
                // address) and order line items. Build the billing address from the
                // payment-method billing details and the email from the request.
                let billing = item
                    .resource_common_data
                    .address
                    .get_payment_method_billing()
                    .ok_or(IntegrationError::MissingRequiredField {
                        field_name: "billing_address",
                        context: IntegrationErrorContext {
                            additional_context: Some(
                                "Mollie Klarna (PayLater) requires billing details (name, email and postal address). Provide payment_method_billing in the request."
                                    .to_string(),
                            ),
                            ..Default::default()
                        },
                    })?;
                let address =
                    billing
                        .address
                        .as_ref()
                        .ok_or(IntegrationError::MissingRequiredField {
                            field_name: "billing_address.address",
                            context: IntegrationErrorContext {
                                additional_context: Some(
                                    "Mollie Klarna requires a postal address (line1, city, postal_code, country) in the billing address."
                                        .to_string(),
                                ),
                                ..Default::default()
                            },
                        })?;

                let line1 = address
                    .line1
                    .as_ref()
                    .ok_or(IntegrationError::MissingRequiredField {
                        field_name: "billing_address.line1",
                        context: IntegrationErrorContext {
                            additional_context: Some(
                                "Mollie Klarna requires the street address (line1) in the billing address."
                                    .to_string(),
                            ),
                            ..Default::default()
                        },
                    })?
                    .peek()
                    .to_string();
                let street_and_number = match address.line2.as_ref() {
                    Some(line2) => format!("{},{}", line1, line2.peek().as_str()),
                    None => line1,
                };

                let email = billing
                    .email
                    .clone()
                    .or_else(|| item.request.email.clone())
                    .ok_or(IntegrationError::MissingRequiredField {
                        field_name: "email",
                        context: IntegrationErrorContext {
                            additional_context: Some(
                                "Mollie Klarna requires the customer email. Provide it in billing.email or the payment request email."
                                    .to_string(),
                            ),
                            ..Default::default()
                        },
                    })?;

                let billing_address = MollieKlarnaAddress {
                    street_and_number: Secret::new(street_and_number),
                    postal_code: Secret::new(
                        address
                            .zip
                            .as_ref()
                            .ok_or(IntegrationError::MissingRequiredField {
                                field_name: "billing_address.zip",
                                context: IntegrationErrorContext {
                                    additional_context: Some(
                                        "Mollie Klarna requires the postal code (zip) in the billing address."
                                            .to_string(),
                                    ),
                                    ..Default::default()
                                },
                            })?
                            .peek()
                            .to_string(),
                    ),
                    city: address
                        .city
                        .as_ref()
                        .ok_or(IntegrationError::MissingRequiredField {
                            field_name: "billing_address.city",
                            context: IntegrationErrorContext {
                                additional_context: Some(
                                    "Mollie Klarna requires the city in the billing address."
                                        .to_string(),
                                ),
                                ..Default::default()
                            },
                        })?
                        .peek()
                        .to_string(),
                    // Klarna risk assessment uses the billing state/region when
                    // available; mirror the reference connector which forwards
                    // the optional state rather than always sending null.
                    region: address.state.clone(),
                    country: address
                        .country
                        .ok_or(IntegrationError::MissingRequiredField {
                            field_name: "billing_address.country",
                            context: IntegrationErrorContext {
                                additional_context: Some(
                                    "Mollie Klarna requires the country in the billing address."
                                        .to_string(),
                                ),
                                ..Default::default()
                            },
                        })?,
                    given_name: address.first_name.clone().ok_or(
                        IntegrationError::MissingRequiredField {
                            field_name: "billing_address.first_name",
                            context: IntegrationErrorContext {
                                additional_context: Some(
                                    "Mollie Klarna requires the customer's given name (first_name) in the billing address."
                                        .to_string(),
                                ),
                                ..Default::default()
                            },
                        },
                    )?,
                    family_name: address.last_name.clone().ok_or(
                        IntegrationError::MissingRequiredField {
                            field_name: "billing_address.last_name",
                            context: IntegrationErrorContext {
                                additional_context: Some(
                                    "Mollie Klarna requires the customer's family name (last_name) in the billing address."
                                        .to_string(),
                                ),
                                ..Default::default()
                            },
                        },
                    )?,
                    email,
                };

                // Mollie requires at least one order line for Klarna. Per-product
                // line details (and their goods type) are not available in
                // PaymentsAuthorizeData, so build a single line for the full payment
                // amount and default its type to `Physical` — the most general type
                // Klarna accepts for a generic goods purchase. This can be refined
                // once line-item/product-type data is plumbed through the request.
                let line = MollieLineItem {
                    line_type: MollieLineItemType::Physical,
                    description: item
                        .resource_common_data
                        .description
                        .clone()
                        .unwrap_or_else(|| "Payment".to_string()),
                    quantity: 1,
                    unit_price: MollieAmount {
                        currency: item.request.currency,
                        value: amount_value.clone(),
                    },
                    total_amount: MollieAmount {
                        currency: item.request.currency,
                        value: amount_value.clone(),
                    },
                };

                MolliePaymentMethodData::Klarna(Box::new(KlarnaMethodData {
                    billing_address,
                    lines: vec![line],
                }))
            }
            PaymentMethodData::Wallet(wallet_data) => {
                // PayPal (redirect) wallet. Mollie returns a 201 with a checkout
                // URL for the customer to approve the payment; forward the
                // optional billing/shipping addresses for risk assessment.
                let billing_address = item
                    .resource_common_data
                    .address
                    .get_payment_method_billing();
                let shipping_address = item.resource_common_data.address.get_shipping();
                mollie_wallet_payment_method(wallet_data, billing_address, shipping_address)?
            }
            PaymentMethodData::BankRedirect(bank_redirect_data) => {
                // iDeal (redirect) bank redirect. Mollie returns a 201 with a
                // checkout URL for the customer to authenticate at their bank.
                mollie_bank_redirect_payment_method(bank_redirect_data)?
            }
            PaymentMethodData::MandatePayment => {
                // MIT / recurring charge referencing a stored mandate.
                MolliePaymentMethodData::MandatePayment(Box::new(MandatePaymentMethodData {
                    mandate_id: Secret::new(
                        item.request.get_connector_mandate_id().change_context(
                            IntegrationError::MissingRequiredField {
                                field_name: "connector_mandate_id",
                                context: Default::default(),
                            },
                        )?,
                    ),
                }))
            }
            _ => {
                return Err(IntegrationError::NotImplemented(
                    "Payment method not yet implemented for Mollie".to_string(),
                    Default::default(),
                )
                .into());
            }
        };

        // Derive the Mollie sequenceType:
        //   First     -> CIT that creates a mandate (customer_acceptance + OffSession)
        //   Recurring -> MIT using a stored mandate
        //   Oneoff    -> normal one-time payment (existing behavior)
        let sequence_type = if item.request.is_customer_initiated_mandate_payment() {
            SequenceType::First
        } else if item.request.is_mandate_payment() {
            SequenceType::Recurring
        } else {
            SequenceType::Oneoff
        };

        // captureMode is only sent for oneoff payments; omitted for first/recurring.
        let capture_mode = if sequence_type == SequenceType::Oneoff {
            Some(
                if item.request.capture_method == Some(common_enums::CaptureMethod::Automatic) {
                    MollieCaptureMode::Automatic
                } else {
                    MollieCaptureMode::Manual
                },
            )
        } else {
            None
        };

        // customerId is required whenever the payment is part of a mandate
        // (first or recurring); read it from the connector customer created by
        // the CreateConnectorCustomer flow.
        let customer_id = if item.request.is_mandate_payment() {
            Some(
                item.resource_common_data
                    .get_connector_customer_id()
                    .change_context(IntegrationError::MissingRequiredField {
                        field_name: "connector_customer_id",
                        context: Default::default(),
                    })?,
            )
        } else {
            None
        };

        // Build metadata - match Hyperswitch format with orderId
        // Always use orderId format, not connector_meta_data
        let mut metadata_map = serde_json::Map::new();
        metadata_map.insert(
            "orderId".to_string(),
            serde_json::Value::String(
                item.resource_common_data
                    .connector_request_reference_id
                    .clone(),
            ),
        );

        Ok(Self {
            amount: MollieAmount {
                currency: item.request.currency,
                value: amount_value,
            },
            description: item.resource_common_data.description.clone().ok_or(
                IntegrationError::MissingRequiredField {
                    field_name: "description",
                    context: Default::default(),
                },
            )?,
            redirect_url: item.request.router_return_url.clone().unwrap_or_default(),
            // Use empty string for webhook_url since we can't support webhook callbacks
            // in test environment (localhost is unreachable from Mollie's servers).
            // This matches Hyperswitch implementation pattern.
            webhook_url: "".to_string(),
            metadata: serde_json::Value::Object(metadata_map),
            payment_method_data,
            sequence_type,
            capture_mode,
            // Match Hyperswitch: these are always null
            locale: None,
            cancel_url: None,
            customer_id,
        })
    }
}

// Mollie Payment Status enum
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "lowercase")]
pub enum MolliePaymentStatus {
    Open,
    Pending,
    Authorized,
    Paid,
    Canceled,
    Expired,
    Failed,
}

// Mollie Link structure
#[derive(Debug, Serialize, Deserialize)]
pub struct MollieLink {
    pub href: String,
    #[serde(rename = "type")]
    pub link_type: String,
}

// Mollie Links structure
#[derive(Debug, Serialize, Deserialize)]
pub struct MollieLinks {
    #[serde(rename = "self")]
    pub self_link: MollieLink,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checkout: Option<MollieLink>,
}

// Mollie Payment Response structure
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MolliePaymentsResponse {
    pub id: String,
    pub resource: String,
    pub mode: String,
    pub status: MolliePaymentStatus,
    pub amount: MollieAmount,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    pub created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
    #[serde(rename = "_links")]
    pub links: MollieLinks,
    // Present when the payment created/uses a mandate (first / recurring).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mandate_id: Option<Secret<String>>,
    // `sequenceType` is echoed back by Mollie for mandate payments.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sequence_type: Option<SequenceType>,
}

impl MolliePaymentsResponse {
    // Build the Hyperswitch mandate reference from Mollie's `mandateId`, which
    // is surfaced back so later MIT charges can select the mandate.
    fn mandate_reference(&self) -> Option<Box<MandateReference>> {
        self.mandate_id.as_ref().map(|id| {
            Box::new(MandateReference {
                connector_mandate_id: Some(id.clone().expose()),
                payment_method_id: None,
                connector_mandate_request_reference_id: None,
            })
        })
    }
}

// Status mapping implementation - CRITICAL: NEVER HARDCODE STATUS VALUES
impl MolliePaymentStatus {
    fn to_attempt_status(&self) -> common_enums::AttemptStatus {
        match self {
            Self::Open => common_enums::AttemptStatus::AuthenticationPending,
            Self::Pending => common_enums::AttemptStatus::Pending,
            Self::Authorized => common_enums::AttemptStatus::Authorized,
            Self::Paid => common_enums::AttemptStatus::Charged,
            Self::Canceled => common_enums::AttemptStatus::Voided,
            Self::Expired => common_enums::AttemptStatus::Failure,
            Self::Failed => common_enums::AttemptStatus::Failure,
        }
    }
}

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<MolliePaymentsResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<MolliePaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // Map status from Mollie response - NEVER HARDCODE
        let status = item.response.status.to_attempt_status();

        // Extract redirection URL if available
        let redirection_data = item.response.links.checkout.as_ref().and_then(|checkout| {
            url::Url::parse(&checkout.href).ok().map(|url| {
                Box::new(RedirectForm::from((
                    url,
                    common_utils::request::Method::Get,
                )))
            })
        });

        let mandate_reference = item.response.mandate_reference();

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.id.clone()),
                redirection_data,
                mandate_reference,
                connector_metadata: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: Some(item.response.id),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
                splits: None,
            }),
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// PSync Response Transformer - Reuses MolliePaymentsResponse
impl TryFrom<ResponseRouterData<MolliePaymentsResponse, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<MolliePaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // Map status from Mollie response - NEVER HARDCODE
        let status = item.response.status.to_attempt_status();

        // Extract redirection URL if available
        let redirection_data = item.response.links.checkout.as_ref().and_then(|checkout| {
            url::Url::parse(&checkout.href).ok().map(|url| {
                Box::new(RedirectForm::from((
                    url,
                    common_utils::request::Method::Get,
                )))
            })
        });

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.id.clone()),
                redirection_data,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: Some(item.response.id),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
                splits: None,
            }),
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// ===== REFUND FLOW TYPES AND TRANSFORMERS =====

// Mollie Refund Request structure
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MollieRefundRequest {
    pub amount: MollieAmount,
    pub description: Option<String>,
    pub metadata: MollieMetadata,
}

// Mollie Refund Status enum
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MollieRefundStatus {
    Queued,
    Pending,
    Processing,
    Refunded,
    Failed,
    Canceled,
}

// Mollie Refund Links structure
#[derive(Debug, Serialize, Deserialize)]
pub struct MollieRefundLinks {
    #[serde(rename = "self")]
    pub self_link: MollieLink,
    pub payment: MollieLink,
}

// Mollie Refund Response structure
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MollieRefundResponse {
    pub id: String,
    pub resource: String,
    pub mode: String,
    pub status: MollieRefundStatus,
    pub amount: MollieAmount,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
    pub payment_id: String,
    pub created_at: String,
    #[serde(rename = "_links")]
    pub links: MollieRefundLinks,
}

// Refund status mapping implementation - CRITICAL: NEVER HARDCODE STATUS VALUES
impl MollieRefundStatus {
    fn to_refund_status(&self) -> common_enums::RefundStatus {
        match self {
            Self::Queued => common_enums::RefundStatus::Pending,
            Self::Pending => common_enums::RefundStatus::Pending,
            Self::Processing => common_enums::RefundStatus::Pending,
            Self::Refunded => common_enums::RefundStatus::Success,
            Self::Failed => common_enums::RefundStatus::Failure,
            Self::Canceled => common_enums::RefundStatus::Failure,
        }
    }
}

// Request transformer for Refund flow
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        MollieRouterData<RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>, T>,
    > for MollieRefundRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: MollieRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let item = item.router_data;
        // Convert amount to string major unit format (e.g., "10.00" for $10.00)
        let converter = StringMajorUnitForConnector;
        let amount_value = converter
            .convert(item.request.minor_refund_amount, item.request.currency)
            .change_context(IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            })?;

        Ok(Self {
            amount: MollieAmount {
                currency: item.request.currency,
                value: amount_value,
            },
            description: item.request.reason.to_owned(),
            metadata: MollieMetadata {
                order_id: item.request.refund_id.clone(),
            },
        })
    }
}

// Response transformer for Refund flow
impl TryFrom<ResponseRouterData<MollieRefundResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<MollieRefundResponse, Self>) -> Result<Self, Self::Error> {
        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: item.response.id.clone(),
                refund_status: item.response.status.to_refund_status(),
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

// Response transformer for RSync flow - reuses MollieRefundResponse
impl TryFrom<ResponseRouterData<MollieRefundResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<MollieRefundResponse, Self>) -> Result<Self, Self::Error> {
        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: item.response.id.clone(),
                refund_status: item.response.status.to_refund_status(),
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

// ===== VOID FLOW TRANSFORMER =====

// Void Response Transformer - Reuses MolliePaymentsResponse
// No request structure needed - DELETE endpoint with path parameter only
impl TryFrom<ResponseRouterData<MolliePaymentsResponse, Self>>
    for RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<MolliePaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // Map status from Mollie response - NEVER HARDCODE
        // Status "canceled" maps to AttemptStatus::Voided
        let status = item.response.status.to_attempt_status();

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.id.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: Some(item.response.id),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
                splits: None,
            }),
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// ===== PAYMENT METHOD TOKEN FLOW TYPES AND TRANSFORMERS =====

// Mollie Customer Request structure (CreateConnectorCustomer flow)
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MollieCustomerRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<Email>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

// Request transformer for CreateConnectorCustomer flow — POST /customers
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        MollieRouterData<
            RouterDataV2<
                CreateConnectorCustomer,
                PaymentFlowData,
                ConnectorCustomerData,
                ConnectorCustomerResponse,
            >,
            T,
        >,
    > for MollieCustomerRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: MollieRouterData<
            RouterDataV2<
                CreateConnectorCustomer,
                PaymentFlowData,
                ConnectorCustomerData,
                ConnectorCustomerResponse,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let request = &item.router_data.request;
        Ok(Self {
            name: request.name.clone(),
            email: request.email.clone().map(|email| email.expose()),
            metadata: None,
        })
    }
}

// Mollie Customer Response structure
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MollieCustomerResponse {
    pub id: String,       // cst_xxx format
    pub resource: String, // "customer"
    pub mode: String,     // "test" or "live"
    pub name: Option<Secret<String>>,
    pub email: Option<Email>, // Optional - can be null
    pub created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

// Response transformer for CreateConnectorCustomer flow — stores `id` as the
// connector customer id consumed by SetupMandate / recurring Authorize.
impl<F, T> TryFrom<ResponseRouterData<MollieCustomerResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, T, ConnectorCustomerResponse>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<MollieCustomerResponse, Self>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            response: Ok(ConnectorCustomerResponse {
                connector_customer_id: item.response.id,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

// Mollie Card Token Request structure (for /card-tokens endpoint)
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MollieCardTokenRequest<T: PaymentMethodDataTypes> {
    pub card_holder: Secret<String>,
    pub card_number: RawCardNumber<T>,
    pub card_cvv: Secret<String>,
    pub card_expiry_date: Secret<String>, // Format: "MM/YY" (e.g., "12/25")
    pub locale: String,
    pub testmode: bool,
    pub profile_token: Secret<String>,
}

// Mollie Card Token Response structure
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MollieCardTokenResponse {
    pub card_token: Secret<String>, // tkn_xxx format
}

// Request transformer for PaymentMethodToken flow - Card Token Request
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        MollieRouterData<
            RouterDataV2<
                PaymentMethodToken,
                PaymentFlowData,
                PaymentMethodTokenizationData<T>,
                PaymentMethodTokenResponse,
            >,
            T,
        >,
    > for MollieCardTokenRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: MollieRouterData<
            RouterDataV2<
                PaymentMethodToken,
                PaymentFlowData,
                PaymentMethodTokenizationData<T>,
                PaymentMethodTokenResponse,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let item = item.router_data;
        // Extract card data from payment method
        let card_data = match &item.request.payment_method_data {
            PaymentMethodData::Card(card) => Ok(card),
            _ => Err(error_stack::report!(IntegrationError::NotSupported {
                message: "Only card payment method is supported for tokenization".to_string(),
                connector: "Mollie",
                context: Default::default(),
            })),
        }?;

        // Get profile token from auth
        let auth = MollieAuthType::try_from(&item.connector_config)?;
        let profile_token = auth
            .profile_token
            .ok_or(IntegrationError::InvalidConnectorConfig {
                config: "profile_token",
                context: Default::default(),
            })?;

        // Format expiry date as "MM/YY" (required by Mollie Components API)
        // Using CardData util for consistent formatting
        let card_expiry_date =
            card_data.get_card_expiry_month_year_2_digit_with_delimiter("/".to_string())?;

        // Extract browser info and get language - use default if not available
        // Note: When called via UCS gRPC from Hyperswitch, browser_info may not be passed
        // in the PaymentMethodServiceTokenizeRequest proto (it doesn't have that field)
        let locale = item
            .request
            .browser_info
            .as_ref()
            .and_then(|bi| bi.language.clone())
            .unwrap_or_else(|| "en-US".to_string());

        // test_mode is required - error if not provided (matching Hyperswitch)
        let testmode =
            item.resource_common_data
                .test_mode
                .ok_or(IntegrationError::MissingRequiredField {
                    field_name: "test_mode",
                    context: Default::default(),
                })?;

        Ok(Self {
            card_holder: card_data
                .card_holder_name
                .clone()
                .unwrap_or_else(|| Secret::new("Cardholder".to_string())),
            card_number: card_data.card_number.clone(),
            card_cvv: card_data.card_cvc.clone(),
            card_expiry_date,
            locale,
            testmode,
            profile_token,
        })
    }
}

// Response transformer for PaymentMethodToken flow - Card Token Response
impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<MollieCardTokenResponse, Self>>
    for RouterDataV2<
        PaymentMethodToken,
        PaymentFlowData,
        PaymentMethodTokenizationData<T>,
        PaymentMethodTokenResponse,
    >
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<MollieCardTokenResponse, Self>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            response: Ok(PaymentMethodTokenResponse {
                token: item.response.card_token.expose(), // Return tkn_ token
            }),
            resource_common_data: PaymentFlowData {
                status: common_enums::AttemptStatus::Charged, // Tokenization successful
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// ===== CAPTURE FLOW TYPES AND TRANSFORMERS =====

// Mollie Capture Request structure
// POST /payments/{id}/captures
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MollieCaptureRequest {
    pub amount: Option<MollieAmount>,
    pub description: String,
}

// Request transformer for Capture flow
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        MollieRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for MollieCaptureRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: MollieRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let item = item.router_data;
        // Convert amount to string major unit format (e.g., "10.00" for $10.00)
        let converter = StringMajorUnitForConnector;
        let amount_value = converter
            .convert(item.request.minor_amount_to_capture, item.request.currency)
            .change_context(IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            })?;

        Ok(Self {
            amount: Some(MollieAmount {
                currency: item.request.currency,
                value: amount_value,
            }),
            description: item.resource_common_data.description.clone().ok_or(
                IntegrationError::MissingRequiredField {
                    field_name: "description",
                    context: Default::default(),
                },
            )?,
        })
    }
}

// Response transformer for Capture flow - reuses MolliePaymentsResponse
impl TryFrom<ResponseRouterData<MolliePaymentsResponse, Self>>
    for RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<MolliePaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // Map status from Mollie response - NEVER HARDCODE
        let status = item.response.status.to_attempt_status();

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.id.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: Some(item.response.id),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
                splits: None,
            }),
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// Type aliases for reused response types to avoid macro redefinition errors
pub type MollieCaptureResponse = MolliePaymentsResponse;
pub type MolliePSyncResponse = MolliePaymentsResponse;
pub type MollieVoidResponse = MolliePaymentsResponse;
pub type MollieRSyncResponse = MollieRefundResponse;
pub type MollieSetupMandateResponse = MolliePaymentsResponse;
// Distinct alias so the connector macro generates a unique bridge/templating
// marker while reusing the shared MolliePaymentsRequest body + TryFrom.
pub type MollieSetupMandateRequest = MolliePaymentsRequest;

// ===== SETUP MANDATE FLOW (first payment that creates the mandate) =====
// Reuses the shared MolliePaymentsRequest / MolliePaymentsResponse against the
// same POST /payments endpoint, but forces `sequenceType=first`, a zero amount
// and a required `customerId` (created by CreateConnectorCustomer).
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        MollieRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for MolliePaymentsRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: MollieRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let item = item.router_data;

        // SetupMandate sends a zero (near-zero) verification amount.
        let converter = StringMajorUnitForConnector;
        let amount_value = converter
            .convert(MinorUnit::new(0), item.request.currency)
            .change_context(IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            })
            .attach_printable("Failed to convert setup-mandate amount to string major unit")?;

        // Only Card / pre-tokenized card is supported for mandate creation.
        let payment_method_data = match &item.request.payment_method_data {
            PaymentMethodData::PaymentMethodToken(t) => {
                MolliePaymentMethodData::CreditCard(Box::new(CreditCardMethodData {
                    card_token: Some(t.token.clone()),
                    billing_address: None,
                    shipping_address: None,
                }))
            }
            PaymentMethodData::Card(_card_data) => {
                let billing_address = item
                    .resource_common_data
                    .address
                    .get_payment_method_billing()
                    .and_then(|billing| {
                        let address = billing.address.as_ref()?;
                        let line1 = address.line1.as_ref()?.peek().to_string();
                        let street_and_number = match address.line2.as_ref() {
                            Some(line2) => format!("{},{}", line1, line2.peek().as_str()),
                            None => line1,
                        };
                        Some(MollieAddress {
                            street_and_number: Secret::new(street_and_number),
                            postal_code: Secret::new(address.zip.as_ref()?.peek().to_string()),
                            city: address.city.as_ref()?.peek().to_string(),
                            region: None,
                            country: address.country?,
                        })
                    });
                MolliePaymentMethodData::CreditCard(Box::new(CreditCardMethodData {
                    card_token: None,
                    billing_address,
                    shipping_address: None,
                }))
            }
            _ => {
                return Err(IntegrationError::NotImplemented(
                    "Only card payments are supported for Mollie mandate setup".to_string(),
                    Default::default(),
                )
                .into());
            }
        };

        let mut metadata_map = serde_json::Map::new();
        metadata_map.insert(
            "orderId".to_string(),
            serde_json::Value::String(
                item.resource_common_data
                    .connector_request_reference_id
                    .clone(),
            ),
        );

        Ok(Self {
            amount: MollieAmount {
                currency: item.request.currency,
                value: amount_value,
            },
            description: item
                .resource_common_data
                .description
                .clone()
                .unwrap_or_else(|| "Mandate setup".to_string()),
            redirect_url: item.request.router_return_url.clone().unwrap_or_default(),
            webhook_url: "".to_string(),
            metadata: serde_json::Value::Object(metadata_map),
            payment_method_data,
            sequence_type: SequenceType::First,
            capture_mode: None,
            locale: None,
            cancel_url: None,
            customer_id: Some(
                item.resource_common_data
                    .get_connector_customer_id()
                    .change_context(IntegrationError::MissingRequiredField {
                        field_name: "connector_customer_id",
                        context: Default::default(),
                    })?,
            ),
        })
    }
}

// Response transformer for SetupMandate — surfaces the created mandate via
// `mandate_reference` and the 3DS checkout URL for cardholder authentication.
impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<MolliePaymentsResponse, Self>>
    for RouterDataV2<
        SetupMandate,
        PaymentFlowData,
        SetupMandateRequestData<T>,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<MolliePaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status = item.response.status.to_attempt_status();

        let redirection_data = item.response.links.checkout.as_ref().and_then(|checkout| {
            url::Url::parse(&checkout.href).ok().map(|url| {
                Box::new(RedirectForm::from((
                    url,
                    common_utils::request::Method::Get,
                )))
            })
        });

        let mandate_reference = item.response.mandate_reference();

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.id.clone()),
                redirection_data,
                mandate_reference,
                connector_metadata: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: Some(item.response.id),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
                splits: None,
            }),
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// ---- ClientAuthenticationToken flow types ----

/// Creates a Mollie payment for client-side SDK initialization.
/// The checkout URL is returned to the frontend for client-side redirect
/// to complete payment via Mollie's hosted checkout page.
#[serde_with::skip_serializing_none]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MollieClientAuthRequest {
    pub amount: MollieAmount,
    pub description: String,
    pub redirect_url: String,
    pub metadata: serde_json::Value,
}

/// Mollie payment response for ClientAuthenticationToken flow.
/// Reuses the same response format as the Authorize flow.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MollieClientAuthResponse {
    pub id: String,
    pub resource: String,
    pub mode: String,
    pub status: MolliePaymentStatus,
    pub amount: MollieAmount,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    pub created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
    #[serde(rename = "_links")]
    pub links: MollieLinks,
}

// ClientAuthenticationToken Request Transformation
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        MollieRouterData<
            RouterDataV2<
                ClientAuthenticationToken,
                MerchantAuthenticationFlowData,
                ClientAuthenticationTokenRequestData,
                PaymentsResponseData,
            >,
            T,
        >,
    > for MollieClientAuthRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: MollieRouterData<
            RouterDataV2<
                ClientAuthenticationToken,
                MerchantAuthenticationFlowData,
                ClientAuthenticationTokenRequestData,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = item.router_data;

        // Convert amount to string major unit format (e.g., "10.00" for $10.00)
        let converter = StringMajorUnitForConnector;
        let amount_value = converter
            .convert(router_data.request.amount, router_data.request.currency)
            .change_context(IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            })
            .attach_printable("Failed to convert amount to string major unit")?;

        // Build metadata with orderId
        let mut metadata_map = serde_json::Map::new();
        metadata_map.insert(
            "orderId".to_string(),
            serde_json::Value::String(
                router_data
                    .resource_common_data
                    .connector_request_reference_id
                    .clone(),
            ),
        );

        Ok(Self {
            amount: MollieAmount {
                currency: router_data.request.currency,
                value: amount_value,
            },
            description: format!(
                "Payment {}",
                router_data
                    .resource_common_data
                    .connector_request_reference_id
            ),
            redirect_url: router_data
                .resource_common_data
                .return_url
                .clone()
                .unwrap_or_else(|| "https://example.com/return".to_string()),
            metadata: serde_json::Value::Object(metadata_map),
        })
    }
}

// ClientAuthenticationToken Response Transformation
impl TryFrom<ResponseRouterData<MollieClientAuthResponse, Self>>
    for RouterDataV2<
        ClientAuthenticationToken,
        MerchantAuthenticationFlowData,
        ClientAuthenticationTokenRequestData,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<MollieClientAuthResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;

        // Extract checkout URL from _links — this is the client-side redirect URL
        let checkout_url = response
            .links
            .checkout
            .as_ref()
            .map(|link| Secret::new(link.href.clone()))
            .ok_or(ConnectorError::ResponseDeserializationFailed {
                context: Default::default(),
            })?;

        let session_data = ClientAuthenticationTokenData::ConnectorSpecific(Box::new(
            ConnectorSpecificClientAuthenticationResponse::Mollie(
                MollieClientAuthenticationResponseDomain {
                    payment_id: response.id,
                    checkout_url,
                },
            ),
        ));

        Ok(Self {
            response: Ok(PaymentsResponseData::ClientAuthenticationTokenResponse {
                session_data,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}
