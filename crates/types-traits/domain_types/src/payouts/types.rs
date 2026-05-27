use crate::errors::{IntegrationError, IntegrationErrorContext};
use crate::payouts;
use crate::types::Connectors;
use crate::utils::{extract_merchant_id_from_metadata, ForeignFrom, ForeignTryFrom};
use common_utils::metadata::MaskedMetadata;
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, PeekInterface};
use payouts::payouts_types::PayoutFlowData;

impl
    ForeignTryFrom<(
        grpc_api_types::payouts::PayoutServiceCreateRequest,
        Connectors,
        &MaskedMetadata,
    )> for PayoutFlowData
{
    type Error = IntegrationError;

    fn foreign_try_from(
        (value, connectors, metadata): (
            grpc_api_types::payouts::PayoutServiceCreateRequest,
            Connectors,
            &MaskedMetadata,
        ),
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        let merchant_id = extract_merchant_id_from_metadata(metadata)?;

        Ok(Self {
            merchant_id,
            payout_id: value.merchant_payout_id.clone().unwrap_or_default(),
            connectors,
            connector_request_reference_id: crate::utils::extract_connector_request_reference_id(
                &value.merchant_payout_id,
            ),
            raw_connector_response: None,
            connector_response_headers: None,
            raw_connector_request: None,
            access_token: value.access_token.map(|token| {
                crate::connector_types::ServerAuthenticationTokenResponseData {
                    access_token: token,
                    token_type: None,
                    expires_in: None,
                }
            }),
            test_mode: None,
            description: value.description.clone(),
        })
    }
}

impl ForeignTryFrom<grpc_api_types::payouts::PayoutServiceCreateRequest>
    for payouts::payouts_types::PayoutCreateRequest
{
    type Error = IntegrationError;

    fn foreign_try_from(
        value: grpc_api_types::payouts::PayoutServiceCreateRequest,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        let amount = match value.amount {
            Some(amount) => amount,
            None => {
                return Err(error_stack::report!(
                    IntegrationError::MissingRequiredField {
                        field_name: "amount",
                        context: IntegrationErrorContext {
                            additional_context: Some("Amount is required".to_owned()),
                            ..Default::default()
                        },
                    }
                ));
            }
        };

        let source_currency = {
            let curr = grpc_api_types::payments::Currency::try_from(amount.currency)
                .change_context(IntegrationError::InvalidDataFormat {
                    field_name: "currency",
                    context: IntegrationErrorContext {
                        additional_context: Some("Invalid currency".to_owned()),
                        ..Default::default()
                    },
                })?;
            common_enums::Currency::foreign_try_from(curr)?
        };

        let destination_currency = {
            let curr = grpc_api_types::payments::Currency::try_from(value.destination_currency)
                .change_context(IntegrationError::InvalidDataFormat {
                    field_name: "destination_currency",
                    context: IntegrationErrorContext {
                        additional_context: Some("Invalid destination currency".to_owned()),
                        ..Default::default()
                    },
                })?;
            common_enums::Currency::foreign_try_from(curr)?
        };

        let payout_method_data = value
            .payout_method_data
            .map(payouts::payout_method_data::PayoutMethodData::foreign_try_from)
            .transpose()?;

        Ok(Self {
            merchant_payout_id: value.merchant_payout_id.clone(),
            connector_quote_id: value.connector_quote_id.clone(),
            connector_payout_id: value.connector_payout_id.clone(),
            amount: common_utils::types::MinorUnit::new(amount.minor_amount),
            source_currency,
            destination_currency,
            priority: value
                .priority
                .map(|p| {
                    let pp = grpc_api_types::payouts::payout_enums::PayoutPriority::try_from(p)
                        .change_context(IntegrationError::InvalidDataFormat {
                            field_name: "priority",
                            context: IntegrationErrorContext {
                                additional_context: Some("Invalid payout priority".to_owned()),
                                ..Default::default()
                            },
                        })?;
                    common_enums::PayoutPriority::foreign_try_from(pp)
                })
                .transpose()?,
            connector_payout_method_id: value.connector_payout_method_id.clone(),
            webhook_url: value.webhook_url.clone(),
            payout_method_data,
            source_bank_data: value
                .source_bank_data
                .map(payouts::payout_method_data::Bank::foreign_try_from)
                .transpose()?,
        })
    }
}

impl crate::utils::ForeignFrom<common_enums::PayoutStatus>
    for grpc_api_types::payouts::payout_enums::PayoutStatus
{
    fn foreign_from(status: common_enums::PayoutStatus) -> Self {
        match status {
            common_enums::PayoutStatus::Success => Self::Success,
            common_enums::PayoutStatus::Failure => Self::Failed,
            common_enums::PayoutStatus::Cancelled => Self::Cancelled,
            common_enums::PayoutStatus::Initiated => Self::Initiated,
            common_enums::PayoutStatus::Expired => Self::Expired,
            common_enums::PayoutStatus::Reversed => Self::Reversed,
            common_enums::PayoutStatus::Pending => Self::Pending,
            common_enums::PayoutStatus::Ineligible => Self::Ineligible,
            common_enums::PayoutStatus::RequiresCreation => Self::RequiresCreation,
            common_enums::PayoutStatus::RequiresConfirmation => Self::RequiresConfirmation,
            common_enums::PayoutStatus::RequiresPayoutMethodData => Self::RequiresPayoutMethodData,
            common_enums::PayoutStatus::RequiresFulfillment => Self::RequiresFulfillment,
            common_enums::PayoutStatus::RequiresVendorAccountCreation => {
                Self::RequiresVendorAccountCreation
            }
        }
    }
}

impl From<payouts::payouts_types::PayoutCreateResponse>
    for grpc_api_types::payouts::PayoutServiceCreateResponse
{
    fn from(response: payouts::payouts_types::PayoutCreateResponse) -> Self {
        let payout_status = grpc_api_types::payouts::payout_enums::PayoutStatus::foreign_from(
            response.payout_status,
        ) as i32;

        Self {
            merchant_payout_id: response.merchant_payout_id,
            payout_status: Some(payout_status),
            connector_payout_id: response.connector_payout_id,
            error: None,
            status_code: u32::from(response.status_code),
        }
    }
}

impl ForeignTryFrom<grpc_api_types::payouts::payout_enums::PayoutPriority>
    for common_enums::PayoutPriority
{
    type Error = IntegrationError;

    fn foreign_try_from(
        value: grpc_api_types::payouts::payout_enums::PayoutPriority,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        match value {
            grpc_api_types::payouts::payout_enums::PayoutPriority::Instant => Ok(Self::Instant),
            grpc_api_types::payouts::payout_enums::PayoutPriority::Fast => Ok(Self::Fast),
            grpc_api_types::payouts::payout_enums::PayoutPriority::Regular => Ok(Self::Regular),
            grpc_api_types::payouts::payout_enums::PayoutPriority::Wire => Ok(Self::Wire),
            grpc_api_types::payouts::payout_enums::PayoutPriority::CrossBorder => {
                Ok(Self::CrossBorder)
            }
            grpc_api_types::payouts::payout_enums::PayoutPriority::Internal => Ok(Self::Internal),
            grpc_api_types::payouts::payout_enums::PayoutPriority::Unspecified => {
                Err(error_stack::report!(IntegrationError::InvalidDataFormat {
                    field_name: "priority",
                    context: IntegrationErrorContext {
                        additional_context: Some(
                            "Payout priority unspecified is not allowed".to_owned()
                        ),
                        ..Default::default()
                    },
                }))
            }
        }
    }
}

impl ForeignTryFrom<grpc_api_types::payouts::payout_enums::PayoutRecipientType>
    for common_enums::PayoutRecipientType
{
    type Error = IntegrationError;

    fn foreign_try_from(
        value: grpc_api_types::payouts::payout_enums::PayoutRecipientType,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        match value {
            grpc_api_types::payouts::payout_enums::PayoutRecipientType::Individual => {
                Ok(Self::Individual)
            }
            grpc_api_types::payouts::payout_enums::PayoutRecipientType::Company => {
                Ok(Self::Company)
            }
            grpc_api_types::payouts::payout_enums::PayoutRecipientType::NonProfit => {
                Ok(Self::NonProfit)
            }
            grpc_api_types::payouts::payout_enums::PayoutRecipientType::PublicSector => {
                Ok(Self::PublicSector)
            }
            grpc_api_types::payouts::payout_enums::PayoutRecipientType::NaturalPerson => {
                Ok(Self::NaturalPerson)
            }
            grpc_api_types::payouts::payout_enums::PayoutRecipientType::Business => {
                Ok(Self::Business)
            }
            grpc_api_types::payouts::payout_enums::PayoutRecipientType::Personal => {
                Ok(Self::Personal)
            }
            grpc_api_types::payouts::payout_enums::PayoutRecipientType::Unspecified => {
                Err(error_stack::report!(IntegrationError::InvalidDataFormat {
                    field_name: "recipient_type",
                    context: IntegrationErrorContext {
                        additional_context: Some(
                            "Payout recipient type unspecified is not allowed".to_owned()
                        ),
                        ..Default::default()
                    },
                }))
            }
        }
    }
}

impl ForeignTryFrom<grpc_api_types::payouts::CardPayout>
    for payouts::payout_method_data::CardPayout
{
    type Error = IntegrationError;
    fn foreign_try_from(
        card: grpc_api_types::payouts::CardPayout,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        let card_network =
            card.card_network
                .map(|n| {
                    let network = grpc_api_types::payments::CardNetwork::try_from(n)
                        .change_context(IntegrationError::InvalidDataFormat {
                            field_name: "card_network",
                            context: IntegrationErrorContext {
                                additional_context: Some("Invalid card network".to_owned()),
                                ..Default::default()
                            },
                        })?;
                    common_enums::CardNetwork::foreign_try_from(network)
                })
                .transpose()?;
        Ok(payouts::payout_method_data::CardPayout {
            card_number: std::str::FromStr::from_str(
                &card
                    .card_number
                    .ok_or_else(|| {
                        error_stack::report!(IntegrationError::MissingRequiredField {
                            field_name: "card_number",
                            context: IntegrationErrorContext {
                                additional_context: Some(
                                    "Card number is required for card payout".to_owned()
                                ),
                                ..Default::default()
                            },
                        })
                    })?
                    .peek()
                    .clone(),
            )
            .map_err(|e| {
                error_stack::Report::new(IntegrationError::InvalidDataFormat {
                    field_name: "card_number",
                    context: IntegrationErrorContext {
                        additional_context: Some("Invalid card number".to_owned()),
                        ..Default::default()
                    },
                })
                .attach_printable(format!("{e:?}"))
            })?,
            expiry_month: ::hyperswitch_masking::Secret::new(
                card.card_exp_month
                    .ok_or_else(|| {
                        error_stack::report!(IntegrationError::MissingRequiredField {
                            field_name: "card_exp_month",
                            context: IntegrationErrorContext {
                                additional_context: Some(
                                    "Card expiry month is required".to_owned()
                                ),
                                ..Default::default()
                            },
                        })
                    })?
                    .peek()
                    .to_string(),
            ),
            expiry_year: ::hyperswitch_masking::Secret::new(
                card.card_exp_year
                    .ok_or_else(|| {
                        error_stack::report!(IntegrationError::MissingRequiredField {
                            field_name: "card_exp_year",
                            context: IntegrationErrorContext {
                                additional_context: Some("Card expiry year is required".to_owned()),
                                ..Default::default()
                            },
                        })
                    })?
                    .peek()
                    .to_string(),
            ),
            card_holder_name: card
                .card_holder_name
                .map(|m| ::hyperswitch_masking::Secret::new(m.peek().to_string())),
            card_network,
        })
    }
}

impl ForeignTryFrom<grpc_api_types::payouts::AchBankTransferPayout>
    for payouts::payout_method_data::AchBankTransfer
{
    type Error = IntegrationError;
    fn foreign_try_from(
        ach: grpc_api_types::payouts::AchBankTransferPayout,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        let bank_name = ach
            .bank_name
            .map(|bn| {
                common_enums::BankNames::try_from(
                    grpc_api_types::payouts::BankNames::try_from(bn)
                        .map(|b| b.as_str_name())
                        .unwrap_or_default(),
                )
                .change_context(IntegrationError::InvalidDataFormat {
                    field_name: "bank_name",
                    context: IntegrationErrorContext {
                        additional_context: Some("Invalid bank name".to_owned()),
                        ..Default::default()
                    },
                })
            })
            .transpose()?;
        let bank_country_code =
            ach.bank_country_code
                .map(|bcc| {
                    let cc = grpc_api_types::payments::CountryAlpha2::try_from(bcc)
                        .change_context(IntegrationError::InvalidDataFormat {
                            field_name: "country_code",
                            context: IntegrationErrorContext {
                                additional_context: Some("Invalid bank country code".to_owned()),
                                ..Default::default()
                            },
                        })?;
                    common_enums::CountryAlpha2::foreign_try_from(cc)
                })
                .transpose()?;

        Ok(payouts::payout_method_data::AchBankTransfer {
            bank_name,
            bank_country_code,
            bank_city: ach.bank_city,
            bank_account_number: ach
                .bank_account_number
                .map(|acc| ::hyperswitch_masking::Secret::new(acc.peek().to_string()))
                .ok_or_else(|| {
                    error_stack::report!(IntegrationError::MissingRequiredField {
                        field_name: "bank_account_number",
                        context: IntegrationErrorContext {
                            additional_context: Some(
                                "Bank account number is required for ACH".to_owned()
                            ),
                            ..Default::default()
                        },
                    })
                })?,
            bank_routing_number: ach
                .bank_routing_number
                .map(|r| ::hyperswitch_masking::Secret::new(r.peek().to_string()))
                .ok_or_else(|| {
                    error_stack::report!(IntegrationError::MissingRequiredField {
                        field_name: "bank_routing_number",
                        context: IntegrationErrorContext {
                            additional_context: Some(
                                "Bank routing number is required for ACH".to_owned()
                            ),
                            ..Default::default()
                        },
                    })
                })?,
        })
    }
}

impl ForeignTryFrom<grpc_api_types::payouts::BacsBankTransferPayout>
    for payouts::payout_method_data::BacsBankTransfer
{
    type Error = IntegrationError;
    fn foreign_try_from(
        bacs: grpc_api_types::payouts::BacsBankTransferPayout,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        let bank_name = bacs
            .bank_name
            .map(|bn| {
                common_enums::BankNames::try_from(
                    grpc_api_types::payouts::BankNames::try_from(bn)
                        .map(|b| b.as_str_name())
                        .unwrap_or_default(),
                )
                .change_context(IntegrationError::InvalidDataFormat {
                    field_name: "bank_name",
                    context: IntegrationErrorContext {
                        additional_context: Some("Invalid bank name".to_owned()),
                        ..Default::default()
                    },
                })
            })
            .transpose()?;
        let bank_country_code =
            bacs.bank_country_code
                .map(|bcc| {
                    let cc = grpc_api_types::payments::CountryAlpha2::try_from(bcc)
                        .change_context(IntegrationError::InvalidDataFormat {
                            field_name: "country_code",
                            context: IntegrationErrorContext {
                                additional_context: Some("Invalid bank country code".to_owned()),
                                ..Default::default()
                            },
                        })?;
                    common_enums::CountryAlpha2::foreign_try_from(cc)
                })
                .transpose()?;
        Ok(payouts::payout_method_data::BacsBankTransfer {
            bank_name,
            bank_country_code,
            bank_city: bacs.bank_city,
            bank_account_number: bacs
                .bank_account_number
                .map(|acc| ::hyperswitch_masking::Secret::new(acc.peek().to_string()))
                .ok_or_else(|| {
                    error_stack::report!(IntegrationError::MissingRequiredField {
                        field_name: "bank_account_number",
                        context: IntegrationErrorContext {
                            additional_context: Some(
                                "Bank account number is required for Bacs".to_owned()
                            ),
                            ..Default::default()
                        },
                    })
                })?,
            bank_sort_code: bacs
                .bank_sort_code
                .map(|sc| ::hyperswitch_masking::Secret::new(sc.peek().to_string()))
                .ok_or_else(|| {
                    error_stack::report!(IntegrationError::MissingRequiredField {
                        field_name: "bank_sort_code",
                        context: IntegrationErrorContext {
                            additional_context: Some(
                                "Bank sort code is required for Bacs".to_owned()
                            ),
                            ..Default::default()
                        },
                    })
                })?,
        })
    }
}

impl ForeignTryFrom<grpc_api_types::payouts::SepaBankTransferPayout>
    for payouts::payout_method_data::SepaBankTransfer
{
    type Error = IntegrationError;
    fn foreign_try_from(
        sepa: grpc_api_types::payouts::SepaBankTransferPayout,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        let bank_name = sepa
            .bank_name
            .map(|bn| {
                common_enums::BankNames::try_from(
                    grpc_api_types::payouts::BankNames::try_from(bn)
                        .map(|b| b.as_str_name())
                        .unwrap_or_default(),
                )
                .change_context(IntegrationError::InvalidDataFormat {
                    field_name: "bank_name",
                    context: IntegrationErrorContext {
                        additional_context: Some("Invalid bank name".to_owned()),
                        ..Default::default()
                    },
                })
            })
            .transpose()?;
        let bank_country_code =
            sepa.bank_country_code
                .map(|bcc| {
                    let cc = grpc_api_types::payments::CountryAlpha2::try_from(bcc)
                        .change_context(IntegrationError::InvalidDataFormat {
                            field_name: "country_code",
                            context: IntegrationErrorContext {
                                additional_context: Some("Invalid bank country code".to_owned()),
                                ..Default::default()
                            },
                        })?;
                    common_enums::CountryAlpha2::foreign_try_from(cc)
                })
                .transpose()?;
        Ok(payouts::payout_method_data::SepaBankTransfer {
            bank_name,
            bank_country_code,
            bank_city: sepa.bank_city,
            iban: ::hyperswitch_masking::Secret::new(
                sepa.iban
                    .ok_or_else(|| {
                        error_stack::report!(IntegrationError::MissingRequiredField {
                            field_name: "iban",
                            context: IntegrationErrorContext {
                                additional_context: Some(
                                    "IBAN is required for SEPA bank transfer".to_owned()
                                ),
                                ..Default::default()
                            },
                        })
                    })?
                    .peek()
                    .to_string(),
            ),
            bic: sepa
                .bic
                .map(|b| ::hyperswitch_masking::Secret::new(b.peek().to_string())),
        })
    }
}

impl ForeignTryFrom<grpc_api_types::payouts::PixBankTransferPayout>
    for payouts::payout_method_data::PixBankTransfer
{
    type Error = IntegrationError;
    fn foreign_try_from(
        pix: grpc_api_types::payouts::PixBankTransferPayout,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        let bank_name = pix
            .bank_name
            .map(|bn| {
                common_enums::BankNames::try_from(
                    grpc_api_types::payouts::BankNames::try_from(bn)
                        .map(|b| b.as_str_name())
                        .unwrap_or_default(),
                )
                .change_context(IntegrationError::InvalidDataFormat {
                    field_name: "bank_name",
                    context: IntegrationErrorContext {
                        additional_context: Some("Invalid bank name".to_owned()),
                        ..Default::default()
                    },
                })
            })
            .transpose()?;
        Ok(payouts::payout_method_data::PixBankTransfer {
            bank_name,
            bank_branch: pix.bank_branch,
            bank_account_number: pix.bank_account_number.ok_or_else(|| {
                error_stack::report!(IntegrationError::MissingRequiredField {
                    field_name: "bank_account_number",
                    context: IntegrationErrorContext {
                        additional_context: Some(
                            "Bank account number is required for Pix bank transfer".to_owned()
                        ),
                        suggested_action: Some(
                            "Provide the recipient's bank account number in the `bank_account_number` field of the Pix payout method data".to_owned()
                        ),
                        doc_url: None,
                    },
                })
            })?,
            tax_id: pix.tax_id,
            ispb: pix.ispb,
        })
    }
}

impl ForeignTryFrom<grpc_api_types::payouts::PixKeyBankTransferPayout>
    for payouts::payout_method_data::PixKeyBankTransfer
{
    type Error = IntegrationError;
    fn foreign_try_from(
        pix: grpc_api_types::payouts::PixKeyBankTransferPayout,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        Ok(payouts::payout_method_data::PixKeyBankTransfer {
            pix_key: pix.pix_key.ok_or_else(|| {
                error_stack::report!(IntegrationError::MissingRequiredField {
                    field_name: "pix_key",
                    context: IntegrationErrorContext {
                        additional_context: Some(
                            "Pix key is required for Pix key bank transfer".to_owned()
                        ),
                        suggested_action: Some(
                            "Provide the recipient's Pix key (CPF, CNPJ, phone, email, or random key) in the `pix_key` field of the Pix key payout method data".to_owned()
                        ),
                        doc_url: None,
                    },
                })
            })?,
        })
    }
}

impl ForeignTryFrom<grpc_api_types::payouts::PixEmvBankTransferPayout>
    for payouts::payout_method_data::PixEmvBankTransfer
{
    type Error = IntegrationError;
    fn foreign_try_from(
        pix: grpc_api_types::payouts::PixEmvBankTransferPayout,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        Ok(payouts::payout_method_data::PixEmvBankTransfer {
            emv: pix.emv.ok_or_else(|| {
                error_stack::report!(IntegrationError::MissingRequiredField {
                    field_name: "emv",
                    context: IntegrationErrorContext {
                        additional_context: Some(
                            "EMV is required for Pix EMV bank transfer".to_owned()
                        ),
                        suggested_action: Some(
                            "Provide the EMV QR code payload in the `emv` field of the Pix EMV payout method data".to_owned()
                        ),
                        doc_url: None,
                    },
                })
            })?,
        })
    }
}

impl ForeignTryFrom<grpc_api_types::payouts::ApplePayDecrypt>
    for payouts::payout_method_data::ApplePayDecrypt
{
    type Error = IntegrationError;
    fn foreign_try_from(
        apple: grpc_api_types::payouts::ApplePayDecrypt,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        let card_network = apple
            .card_network
            .map(|n| {
                let network = grpc_api_types::payments::CardNetwork::try_from(n).change_context(
                    IntegrationError::InvalidDataFormat {
                        field_name: "card_network",
                        context: IntegrationErrorContext {
                            additional_context: Some("Invalid card network".to_owned()),
                            ..Default::default()
                        },
                    },
                )?;
                common_enums::CardNetwork::foreign_try_from(network)
            })
            .transpose()?;
        Ok(payouts::payout_method_data::ApplePayDecrypt {
            dpan: std::str::FromStr::from_str(
                &apple
                    .dpan
                    .ok_or_else(|| {
                        error_stack::report!(IntegrationError::MissingRequiredField {
                            field_name: "dpan",
                            context: IntegrationErrorContext {
                                additional_context: Some(
                                    "DPAN (device primary account number) is required for Apple Pay decrypt"
                                        .to_owned()
                                ),
                                ..Default::default()
                            },
                        })
                    })?
                    .peek()
                    .clone(),
            )
            .map_err(|e| {
                error_stack::Report::new(IntegrationError::InvalidDataFormat {
                    field_name: "dpan",
                    context: IntegrationErrorContext {
                        additional_context: Some("Invalid dpan".to_owned()),
                        ..Default::default()
                    },
                })
                .attach_printable(format!("{e:?}"))
            })?,
            expiry_month: ::hyperswitch_masking::Secret::new(
                apple
                    .expiry_month
                    .ok_or_else(|| {
                        error_stack::report!(IntegrationError::MissingRequiredField {
                            field_name: "card_exp_month",
                            context: IntegrationErrorContext {
                                additional_context: Some(
                                    "Card expiry month is required".to_owned()
                                ),
                                ..Default::default()
                            },
                        })
                    })?
                    .peek()
                    .to_string(),
            ),
            expiry_year: ::hyperswitch_masking::Secret::new(
                apple
                    .expiry_year
                    .ok_or_else(|| {
                        error_stack::report!(IntegrationError::MissingRequiredField {
                            field_name: "card_exp_year",
                            context: IntegrationErrorContext {
                                additional_context: Some("Card expiry year is required".to_owned()),
                                ..Default::default()
                            },
                        })
                    })?
                    .peek()
                    .to_string(),
            ),
            card_holder_name: apple
                .card_holder_name
                .map(|n| ::hyperswitch_masking::Secret::new(n.peek().to_string())),
            card_network,
        })
    }
}

impl ForeignTryFrom<grpc_api_types::payouts::Paypal> for payouts::payout_method_data::Paypal {
    type Error = IntegrationError;
    fn foreign_try_from(
        paypal: grpc_api_types::payouts::Paypal,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        Ok(payouts::payout_method_data::Paypal {
            email: paypal
                .email
                .map(|e| {
                    e.peek().to_string().parse().map_err(|err| {
                        error_stack::Report::new(IntegrationError::InvalidDataFormat {
                            field_name: "email",
                            context: IntegrationErrorContext {
                                additional_context: Some("Invalid email".to_owned()),
                                ..Default::default()
                            },
                        })
                        .attach_printable(format!("{err:?}"))
                    })
                })
                .transpose()?,
            telephone_number: paypal
                .telephone_number
                .map(|t| ::hyperswitch_masking::Secret::new(t.peek().to_string())),
            paypal_id: paypal
                .paypal_id
                .map(|p| ::hyperswitch_masking::Secret::new(p.peek().to_string())),
        })
    }
}

impl ForeignTryFrom<grpc_api_types::payouts::Venmo> for payouts::payout_method_data::Venmo {
    type Error = IntegrationError;
    fn foreign_try_from(
        venmo: grpc_api_types::payouts::Venmo,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        Ok(payouts::payout_method_data::Venmo {
            telephone_number: venmo
                .telephone_number
                .map(|t| ::hyperswitch_masking::Secret::new(t.peek().to_string())),
        })
    }
}

impl ForeignTryFrom<grpc_api_types::payouts::InteracPayout>
    for payouts::payout_method_data::Interac
{
    type Error = IntegrationError;
    fn foreign_try_from(
        interac: grpc_api_types::payouts::InteracPayout,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        Ok(payouts::payout_method_data::Interac {
            email: interac
                .email
                .ok_or_else(|| {
                    error_stack::report!(IntegrationError::MissingRequiredField {
                        field_name: "email",
                        context: IntegrationErrorContext {
                            additional_context: Some("Email is required for Interac".to_owned()),
                            ..Default::default()
                        },
                    })
                })?
                .peek()
                .to_string()
                .parse()
                .map_err(|e| {
                    error_stack::Report::new(IntegrationError::InvalidDataFormat {
                        field_name: "email",
                        context: IntegrationErrorContext {
                            additional_context: Some("Invalid email".to_owned()),
                            ..Default::default()
                        },
                    })
                    .attach_printable(format!("{e:?}"))
                })?,
        })
    }
}

impl ForeignTryFrom<grpc_api_types::payouts::OpenBankingUkPayout>
    for payouts::payout_method_data::OpenBankingUk
{
    type Error = IntegrationError;
    fn foreign_try_from(
        obuk: grpc_api_types::payouts::OpenBankingUkPayout,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        Ok(payouts::payout_method_data::OpenBankingUk {
            account_holder_name: ::hyperswitch_masking::Secret::new(
                obuk.account_holder_name
                    .ok_or_else(|| {
                        error_stack::report!(IntegrationError::MissingRequiredField {
                            field_name: "account_holder_name",
                            context: IntegrationErrorContext {
                                additional_context: Some(
                                    "Account holder name is required for OpenBankingUK".to_owned()
                                ),
                                ..Default::default()
                            },
                        })
                    })?
                    .peek()
                    .to_string(),
            ),
            iban: ::hyperswitch_masking::Secret::new(
                obuk.iban
                    .ok_or_else(|| {
                        error_stack::report!(IntegrationError::MissingRequiredField {
                            field_name: "iban",
                            context: IntegrationErrorContext {
                                additional_context: Some("IBAN is required for SEPA".to_owned()),
                                ..Default::default()
                            },
                        })
                    })?
                    .peek()
                    .to_string(),
            ),
        })
    }
}

impl ForeignTryFrom<grpc_api_types::payouts::Passthrough>
    for payouts::payout_method_data::Passthrough
{
    type Error = IntegrationError;
    fn foreign_try_from(
        pt: grpc_api_types::payouts::Passthrough,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        let token_type = grpc_api_types::payments::PaymentMethodType::try_from(pt.token_type)
            .change_context(IntegrationError::InvalidDataFormat {
                field_name: "token_type",
                context: IntegrationErrorContext {
                    additional_context: Some("Invalid pass through token type".to_owned()),
                    ..Default::default()
                },
            })?;
        let token_type_opt =
            Option::<common_enums::PaymentMethodType>::foreign_try_from(token_type)?;
        let token_type = token_type_opt.ok_or_else(|| {
            error_stack::report!(IntegrationError::InvalidDataFormat {
                field_name: "token_type",
                context: IntegrationErrorContext {
                    additional_context: Some("Invalid pass through token type".to_owned()),
                    ..Default::default()
                },
            })
        })?;
        Ok(payouts::payout_method_data::Passthrough {
            psp_token: ::hyperswitch_masking::Secret::new(pt.psp_token),
            token_type,
        })
    }
}

impl ForeignTryFrom<grpc_api_types::payouts::PayoutMethod>
    for payouts::payout_method_data::PayoutMethodData
{
    type Error = IntegrationError;
    fn foreign_try_from(
        value: grpc_api_types::payouts::PayoutMethod,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        let data = value.payout_method_data.ok_or_else(|| {
            error_stack::report!(IntegrationError::MissingRequiredField {
                field_name: "payout_method_data",
                context: IntegrationErrorContext {
                    additional_context: Some("Payout method data is required".to_owned()),
                    ..Default::default()
                },
            })
        })?;

        match data {
            grpc_api_types::payouts::payout_method::PayoutMethodData::Card(card) => Ok(Self::Card(
                payouts::payout_method_data::CardPayout::foreign_try_from(card)?,
            )),
            grpc_api_types::payouts::payout_method::PayoutMethodData::Ach(ach) => {
                Ok(Self::Bank(payouts::payout_method_data::Bank::Ach(
                    payouts::payout_method_data::AchBankTransfer::foreign_try_from(ach)?,
                )))
            }
            grpc_api_types::payouts::payout_method::PayoutMethodData::Bacs(bacs) => {
                Ok(Self::Bank(payouts::payout_method_data::Bank::Bacs(
                    payouts::payout_method_data::BacsBankTransfer::foreign_try_from(bacs)?,
                )))
            }
            grpc_api_types::payouts::payout_method::PayoutMethodData::Sepa(sepa) => {
                Ok(Self::Bank(payouts::payout_method_data::Bank::Sepa(
                    payouts::payout_method_data::SepaBankTransfer::foreign_try_from(sepa)?,
                )))
            }
            grpc_api_types::payouts::payout_method::PayoutMethodData::Pix(pix) => {
                Ok(Self::Bank(payouts::payout_method_data::Bank::Pix(
                    payouts::payout_method_data::PixBankTransfer::foreign_try_from(pix)?,
                )))
            }
            grpc_api_types::payouts::payout_method::PayoutMethodData::PixKey(pix_key) => {
                Ok(Self::Bank(payouts::payout_method_data::Bank::PixKey(
                    payouts::payout_method_data::PixKeyBankTransfer::foreign_try_from(pix_key)?,
                )))
            }
            grpc_api_types::payouts::payout_method::PayoutMethodData::PixEmv(pix_emv) => {
                Ok(Self::Bank(payouts::payout_method_data::Bank::PixEmv(
                    payouts::payout_method_data::PixEmvBankTransfer::foreign_try_from(pix_emv)?,
                )))
            }
            grpc_api_types::payouts::payout_method::PayoutMethodData::ApplePayDecrypt(
                apple_pay_decrypt,
            ) => Ok(Self::Wallet(
                payouts::payout_method_data::Wallet::ApplePayDecrypt(
                    payouts::payout_method_data::ApplePayDecrypt::foreign_try_from(
                        apple_pay_decrypt,
                    )?,
                ),
            )),
            grpc_api_types::payouts::payout_method::PayoutMethodData::Paypal(paypal) => {
                Ok(Self::Wallet(payouts::payout_method_data::Wallet::Paypal(
                    payouts::payout_method_data::Paypal::foreign_try_from(paypal)?,
                )))
            }
            grpc_api_types::payouts::payout_method::PayoutMethodData::Venmo(venmo) => {
                Ok(Self::Wallet(payouts::payout_method_data::Wallet::Venmo(
                    payouts::payout_method_data::Venmo::foreign_try_from(venmo)?,
                )))
            }
            grpc_api_types::payouts::payout_method::PayoutMethodData::Interac(interac) => Ok(
                Self::BankRedirect(payouts::payout_method_data::BankRedirect::Interac(
                    payouts::payout_method_data::Interac::foreign_try_from(interac)?,
                )),
            ),
            grpc_api_types::payouts::payout_method::PayoutMethodData::OpenBankingUk(
                open_banking_uk,
            ) => Ok(Self::BankRedirect(
                payouts::payout_method_data::BankRedirect::OpenBankingUk(
                    payouts::payout_method_data::OpenBankingUk::foreign_try_from(open_banking_uk)?,
                ),
            )),
            grpc_api_types::payouts::payout_method::PayoutMethodData::Passthrough(passthrough) => {
                Ok(Self::Passthrough(
                    payouts::payout_method_data::Passthrough::foreign_try_from(passthrough)?,
                ))
            }
        }
    }
}

impl ForeignTryFrom<grpc_api_types::payouts::SourceBankData> for payouts::payout_method_data::Bank {
    type Error = IntegrationError;
    fn foreign_try_from(
        value: grpc_api_types::payouts::SourceBankData,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        let data = value.source_bank_data.ok_or_else(|| {
            error_stack::report!(IntegrationError::MissingRequiredField {
                field_name: "source_bank_data",
                context: IntegrationErrorContext {
                    additional_context: Some("Source bank data is required".to_owned()),
                    suggested_action: Some(
                        "Provide the source bank details (ACH, BACS, SEPA, Pix, etc.) in the `source_bank_data` field of the payout request".to_owned()
                    ),
                    doc_url: None,
                },
            })
        })?;

        match data {
            grpc_api_types::payouts::source_bank_data::SourceBankData::Ach(ach) => Ok(Self::Ach(
                payouts::payout_method_data::AchBankTransfer::foreign_try_from(ach)?,
            )),
            grpc_api_types::payouts::source_bank_data::SourceBankData::Bacs(bacs) => Ok(
                Self::Bacs(payouts::payout_method_data::BacsBankTransfer::foreign_try_from(bacs)?),
            ),
            grpc_api_types::payouts::source_bank_data::SourceBankData::Sepa(sepa) => Ok(
                Self::Sepa(payouts::payout_method_data::SepaBankTransfer::foreign_try_from(sepa)?),
            ),
            grpc_api_types::payouts::source_bank_data::SourceBankData::Pix(pix) => Ok(Self::Pix(
                payouts::payout_method_data::PixBankTransfer::foreign_try_from(pix)?,
            )),
            grpc_api_types::payouts::source_bank_data::SourceBankData::PixKey(pix_key) => {
                Ok(Self::PixKey(
                    payouts::payout_method_data::PixKeyBankTransfer::foreign_try_from(pix_key)?,
                ))
            }
            grpc_api_types::payouts::source_bank_data::SourceBankData::PixEmv(pix_emv) => {
                Ok(Self::PixEmv(
                    payouts::payout_method_data::PixEmvBankTransfer::foreign_try_from(pix_emv)?,
                ))
            }
        }
    }
}

impl ForeignTryFrom<grpc_api_types::payouts::PayoutServiceTransferRequest>
    for payouts::payouts_types::PayoutTransferRequest
{
    type Error = IntegrationError;

    fn foreign_try_from(
        value: grpc_api_types::payouts::PayoutServiceTransferRequest,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        let amount = match value.amount {
            Some(amount) => amount,
            None => {
                return Err(error_stack::report!(
                    IntegrationError::MissingRequiredField {
                        field_name: "amount",
                        context: IntegrationErrorContext {
                            additional_context: Some("Amount is required".to_owned()),
                            ..Default::default()
                        },
                    }
                ));
            }
        };

        let source_currency = {
            let curr = grpc_api_types::payments::Currency::try_from(amount.currency)
                .change_context(IntegrationError::InvalidDataFormat {
                    field_name: "currency",
                    context: IntegrationErrorContext {
                        additional_context: Some("Invalid currency".to_owned()),
                        ..Default::default()
                    },
                })?;
            common_enums::Currency::foreign_try_from(curr)?
        };

        let destination_currency = {
            let curr = grpc_api_types::payments::Currency::try_from(value.destination_currency)
                .change_context(IntegrationError::InvalidDataFormat {
                    field_name: "destination_currency",
                    context: IntegrationErrorContext {
                        additional_context: Some("Invalid destination currency".to_owned()),
                        ..Default::default()
                    },
                })?;
            common_enums::Currency::foreign_try_from(curr)?
        };

        let payout_method_data = value
            .payout_method_data
            .map(payouts::payout_method_data::PayoutMethodData::foreign_try_from)
            .transpose()?;

        let priority = value
            .priority
            .map(|priority| {
                grpc_api_types::payouts::payout_enums::PayoutPriority::try_from(priority)
                    .change_context(IntegrationError::InvalidDataFormat {
                        field_name: "priority",
                        context: IntegrationErrorContext {
                            additional_context: Some("Invalid payout priority".to_owned()),
                            ..Default::default()
                        },
                    })
            })
            .transpose()?
            .map(common_enums::PayoutPriority::foreign_try_from)
            .transpose()?;

        let customer = value
            .customer
            .map(
                |customer| -> Result<_, error_stack::Report<IntegrationError>> {
                    let email = customer
                        .email
                        .map(|email_str| {
                            common_utils::pii::Email::try_from(email_str.expose()).map_err(|e| {
                                error_stack::Report::new(IntegrationError::InvalidDataFormat {
                                    field_name: "email",
                                    context: IntegrationErrorContext {
                                        additional_context: Some("Invalid email".to_owned()),
                                        ..Default::default()
                                    },
                                })
                                .attach_printable(format!("{e:?}"))
                            })
                        })
                        .transpose()?;

                    Ok(payouts::payouts_types::PayoutCustomer {
                        name: customer.name,
                        email,
                        merchant_customer_id: customer.id,
                        connector_customer_id: customer.connector_customer_id,
                        phone_number: customer
                            .phone_number
                            .map(::hyperswitch_masking::Secret::new),
                        phone_country_code: customer.phone_country_code,
                    })
                },
            )
            .transpose()?;

        let address = value
            .address
            .map(payouts::payouts_types::PayoutAddress::foreign_try_from)
            .transpose()?;

        Ok(Self {
            merchant_payout_id: value.merchant_payout_id.clone(),
            connector_quote_id: value.connector_quote_id.clone(),
            connector_payout_id: value.connector_payout_id.clone(),
            amount: common_utils::types::MinorUnit::new(amount.minor_amount),
            source_currency,
            destination_currency,
            priority,
            connector_payout_method_id: value.connector_payout_method_id,
            webhook_url: value.webhook_url,
            payout_method_data,
            source_bank_data: value
                .source_bank_data
                .map(payouts::payout_method_data::Bank::foreign_try_from)
                .transpose()?,
            customer,
            address,
        })
    }
}

impl ForeignTryFrom<grpc_api_types::payouts::PayoutAddress>
    for payouts::payouts_types::PayoutAddress
{
    type Error = IntegrationError;

    fn foreign_try_from(
        value: grpc_api_types::payouts::PayoutAddress,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        Ok(Self {
            shipping_address: value
                .shipping_address
                .map(convert_payouts_address_to_domain)
                .transpose()?,
            billing_address: value
                .billing_address
                .map(convert_payouts_address_to_domain)
                .transpose()?,
        })
    }
}

fn convert_payouts_address_to_domain(
    addr: grpc_api_types::payouts::Address,
) -> Result<crate::payment_address::Address, error_stack::Report<IntegrationError>> {
    let payments_addr = grpc_api_types::payments::Address {
        first_name: addr.first_name,
        last_name: addr.last_name,
        line1: addr.line1,
        line2: addr.line2,
        line3: addr.line3,
        city: addr.city,
        state: addr.state,
        zip_code: addr.zip_code,
        country_alpha2_code: addr.country_alpha2_code,
        email: addr.email,
        phone_number: addr.phone_number,
        phone_country_code: addr.phone_country_code,
    };
    crate::payment_address::Address::foreign_try_from(payments_addr)
}

impl ForeignTryFrom<grpc_api_types::payouts::PayoutServiceGetRequest>
    for payouts::payouts_types::PayoutGetRequest
{
    type Error = IntegrationError;

    fn foreign_try_from(
        value: grpc_api_types::payouts::PayoutServiceGetRequest,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        Ok(Self {
            merchant_payout_id: value.merchant_payout_id,
            connector_payout_id: value.connector_payout_id,
        })
    }
}

impl ForeignTryFrom<grpc_api_types::payouts::PayoutServiceVoidRequest>
    for payouts::payouts_types::PayoutVoidRequest
{
    type Error = IntegrationError;

    fn foreign_try_from(
        value: grpc_api_types::payouts::PayoutServiceVoidRequest,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        Ok(Self {
            merchant_payout_id: value.merchant_payout_id,
            connector_payout_id: value.connector_payout_id,
        })
    }
}

impl ForeignTryFrom<grpc_api_types::payouts::PayoutServiceStageRequest>
    for payouts::payouts_types::PayoutStageRequest
{
    type Error = IntegrationError;

    fn foreign_try_from(
        value: grpc_api_types::payouts::PayoutServiceStageRequest,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        let amount = match value.amount {
            Some(amount) => amount,
            None => {
                return Err(error_stack::report!(
                    IntegrationError::MissingRequiredField {
                        field_name: "amount",
                        context: IntegrationErrorContext {
                            additional_context: Some("Amount is required".to_owned()),
                            ..Default::default()
                        },
                    }
                ));
            }
        };

        let source_currency = {
            let curr = grpc_api_types::payments::Currency::try_from(amount.currency)
                .change_context(IntegrationError::InvalidDataFormat {
                    field_name: "currency",
                    context: IntegrationErrorContext {
                        additional_context: Some("Invalid currency".to_owned()),
                        ..Default::default()
                    },
                })?;
            common_enums::Currency::foreign_try_from(curr)?
        };

        let destination_currency = {
            let curr = grpc_api_types::payments::Currency::try_from(value.destination_currency)
                .change_context(IntegrationError::InvalidDataFormat {
                    field_name: "destination_currency",
                    context: IntegrationErrorContext {
                        additional_context: Some("Invalid destination currency".to_owned()),
                        ..Default::default()
                    },
                })?;
            common_enums::Currency::foreign_try_from(curr)?
        };

        Ok(Self {
            merchant_quote_id: value.merchant_quote_id.clone(),
            amount: common_utils::types::MinorUnit::new(amount.minor_amount),
            source_currency,
            destination_currency,
        })
    }
}

impl ForeignTryFrom<grpc_api_types::payouts::PayoutServiceCreateLinkRequest>
    for payouts::payouts_types::PayoutCreateLinkRequest
{
    type Error = IntegrationError;

    fn foreign_try_from(
        value: grpc_api_types::payouts::PayoutServiceCreateLinkRequest,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        let amount = match value.amount {
            Some(amount) => amount,
            None => {
                return Err(error_stack::report!(
                    IntegrationError::MissingRequiredField {
                        field_name: "amount",
                        context: IntegrationErrorContext {
                            additional_context: Some("Amount is required".to_owned()),
                            ..Default::default()
                        },
                    }
                ));
            }
        };

        let source_currency = {
            let curr = grpc_api_types::payments::Currency::try_from(amount.currency)
                .change_context(IntegrationError::InvalidDataFormat {
                    field_name: "currency",
                    context: IntegrationErrorContext {
                        additional_context: Some("Invalid currency".to_owned()),
                        ..Default::default()
                    },
                })?;
            common_enums::Currency::foreign_try_from(curr)?
        };

        let destination_currency = {
            let curr = grpc_api_types::payments::Currency::try_from(value.destination_currency)
                .change_context(IntegrationError::InvalidDataFormat {
                    field_name: "destination_currency",
                    context: IntegrationErrorContext {
                        additional_context: Some("Invalid destination currency".to_owned()),
                        ..Default::default()
                    },
                })?;
            common_enums::Currency::foreign_try_from(curr)?
        };

        let payout_method_data = value
            .payout_method_data
            .map(payouts::payout_method_data::PayoutMethodData::foreign_try_from)
            .transpose()?;

        let priority = value
            .priority
            .map(|priority| {
                grpc_api_types::payouts::payout_enums::PayoutPriority::try_from(priority)
                    .change_context(IntegrationError::InvalidDataFormat {
                        field_name: "priority",
                        context: IntegrationErrorContext {
                            additional_context: Some("Invalid payout priority".to_owned()),
                            ..Default::default()
                        },
                    })
            })
            .transpose()?
            .map(common_enums::PayoutPriority::foreign_try_from)
            .transpose()?;

        Ok(Self {
            merchant_payout_id: value.merchant_payout_id.clone(),
            connector_quote_id: value.connector_quote_id.clone(),
            connector_payout_id: value.connector_payout_id.clone(),
            amount: common_utils::types::MinorUnit::new(amount.minor_amount),
            source_currency,
            destination_currency,
            priority,
            connector_payout_method_id: value.connector_payout_method_id,
            webhook_url: value.webhook_url,
            payout_method_data,
        })
    }
}

impl ForeignTryFrom<grpc_api_types::payouts::PayoutServiceCreateRecipientRequest>
    for payouts::payouts_types::PayoutCreateRecipientRequest
{
    type Error = IntegrationError;

    fn foreign_try_from(
        value: grpc_api_types::payouts::PayoutServiceCreateRecipientRequest,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        let amount = match value.amount {
            Some(amount) => amount,
            None => {
                return Err(error_stack::report!(
                    IntegrationError::MissingRequiredField {
                        field_name: "amount",
                        context: IntegrationErrorContext {
                            additional_context: Some("Amount is required".to_owned()),
                            ..Default::default()
                        },
                    }
                ));
            }
        };

        let source_currency = {
            let curr = grpc_api_types::payments::Currency::try_from(amount.currency)
                .change_context(IntegrationError::InvalidDataFormat {
                    field_name: "currency",
                    context: IntegrationErrorContext {
                        additional_context: Some("Invalid currency".to_owned()),
                        ..Default::default()
                    },
                })?;
            common_enums::Currency::foreign_try_from(curr)?
        };

        let payout_method_data = value
            .payout_method_data
            .map(payouts::payout_method_data::PayoutMethodData::foreign_try_from)
            .transpose()?;

        let payout_recipient_type =
            grpc_api_types::payouts::payout_enums::PayoutRecipientType::try_from(
                value.recipient_type,
            )
            .change_context(IntegrationError::InvalidDataFormat {
                field_name: "recipient_type",
                context: IntegrationErrorContext {
                    additional_context: Some("Invalid payout recipient type".to_owned()),
                    ..Default::default()
                },
            })?;

        Ok(Self {
            merchant_payout_id: value.merchant_payout_id.clone(),
            amount: common_utils::types::MinorUnit::new(amount.minor_amount),
            source_currency,
            payout_method_data,
            recipient_type: common_enums::PayoutRecipientType::foreign_try_from(
                payout_recipient_type,
            )?,
        })
    }
}

impl ForeignTryFrom<grpc_api_types::payouts::PayoutServiceEnrollDisburseAccountRequest>
    for payouts::payouts_types::PayoutEnrollDisburseAccountRequest
{
    type Error = IntegrationError;

    fn foreign_try_from(
        value: grpc_api_types::payouts::PayoutServiceEnrollDisburseAccountRequest,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        let amount = match value.amount {
            Some(amount) => amount,
            None => {
                return Err(error_stack::report!(
                    IntegrationError::MissingRequiredField {
                        field_name: "amount",
                        context: IntegrationErrorContext {
                            additional_context: Some("Amount is required".to_owned()),
                            ..Default::default()
                        },
                    }
                ));
            }
        };

        let source_currency = {
            let curr = grpc_api_types::payments::Currency::try_from(amount.currency)
                .change_context(IntegrationError::InvalidDataFormat {
                    field_name: "currency",
                    context: IntegrationErrorContext {
                        additional_context: Some("Invalid currency".to_owned()),
                        ..Default::default()
                    },
                })?;
            common_enums::Currency::foreign_try_from(curr)?
        };

        let payout_method_data = value
            .payout_method_data
            .map(payouts::payout_method_data::PayoutMethodData::foreign_try_from)
            .transpose()?;

        Ok(Self {
            merchant_payout_id: value.merchant_payout_id.clone(),
            amount: common_utils::types::MinorUnit::new(amount.minor_amount),
            source_currency,
            payout_method_data,
        })
    }
}

impl
    ForeignTryFrom<(
        grpc_api_types::payouts::PayoutServiceTransferRequest,
        Connectors,
        &MaskedMetadata,
    )> for payouts::payouts_types::PayoutFlowData
{
    type Error = IntegrationError;

    fn foreign_try_from(
        (value, connectors, metadata): (
            grpc_api_types::payouts::PayoutServiceTransferRequest,
            Connectors,
            &MaskedMetadata,
        ),
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        let merchant_id = extract_merchant_id_from_metadata(metadata)?;

        Ok(Self {
            merchant_id,
            payout_id: value.merchant_payout_id.clone().unwrap_or_default(),
            connectors,
            connector_request_reference_id: crate::utils::extract_connector_request_reference_id(
                &value.merchant_payout_id,
            ),
            raw_connector_response: None,
            connector_response_headers: None,
            raw_connector_request: None,
            access_token: value.access_token.map(|token| {
                crate::connector_types::ServerAuthenticationTokenResponseData {
                    access_token: token,
                    token_type: None,
                    expires_in: None,
                }
            }),
            test_mode: None,
            description: value.description.clone(),
        })
    }
}

impl
    ForeignTryFrom<(
        grpc_api_types::payouts::PayoutServiceGetRequest,
        Connectors,
        &MaskedMetadata,
    )> for payouts::payouts_types::PayoutFlowData
{
    type Error = IntegrationError;

    fn foreign_try_from(
        (value, connectors, metadata): (
            grpc_api_types::payouts::PayoutServiceGetRequest,
            Connectors,
            &MaskedMetadata,
        ),
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        let merchant_id = extract_merchant_id_from_metadata(metadata)?;

        Ok(Self {
            merchant_id,
            payout_id: value.merchant_payout_id.clone().unwrap_or_default(),
            connectors,
            connector_request_reference_id: crate::utils::extract_connector_request_reference_id(
                &value.merchant_payout_id,
            ),
            raw_connector_response: None,
            connector_response_headers: None,
            raw_connector_request: None,
            access_token: value.access_token.map(|token| {
                crate::connector_types::ServerAuthenticationTokenResponseData {
                    access_token: token,
                    token_type: None,
                    expires_in: None,
                }
            }),
            test_mode: None,
            description: None,
        })
    }
}

impl
    ForeignTryFrom<(
        grpc_api_types::payouts::PayoutServiceVoidRequest,
        Connectors,
        &MaskedMetadata,
    )> for payouts::payouts_types::PayoutFlowData
{
    type Error = IntegrationError;

    fn foreign_try_from(
        (value, connectors, metadata): (
            grpc_api_types::payouts::PayoutServiceVoidRequest,
            Connectors,
            &MaskedMetadata,
        ),
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        let merchant_id = extract_merchant_id_from_metadata(metadata)?;

        Ok(Self {
            merchant_id,
            payout_id: value.merchant_payout_id.clone().unwrap_or_default(),
            connectors,
            connector_request_reference_id: crate::utils::extract_connector_request_reference_id(
                &value.merchant_payout_id,
            ),
            raw_connector_response: None,
            connector_response_headers: None,
            raw_connector_request: None,
            access_token: value.access_token.map(|token| {
                crate::connector_types::ServerAuthenticationTokenResponseData {
                    access_token: token,
                    token_type: None,
                    expires_in: None,
                }
            }),
            test_mode: None,
            description: None,
        })
    }
}

impl
    ForeignTryFrom<(
        grpc_api_types::payouts::PayoutServiceStageRequest,
        Connectors,
        &MaskedMetadata,
    )> for payouts::payouts_types::PayoutFlowData
{
    type Error = IntegrationError;

    fn foreign_try_from(
        (value, connectors, metadata): (
            grpc_api_types::payouts::PayoutServiceStageRequest,
            Connectors,
            &MaskedMetadata,
        ),
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        let merchant_id = extract_merchant_id_from_metadata(metadata)?;

        Ok(Self {
            merchant_id,
            payout_id: value.merchant_quote_id.clone().unwrap_or_default(),
            connectors,
            connector_request_reference_id: crate::utils::extract_connector_request_reference_id(
                &value.merchant_quote_id,
            ),
            raw_connector_response: None,
            connector_response_headers: None,
            raw_connector_request: None,
            access_token: value.access_token.map(|token| {
                crate::connector_types::ServerAuthenticationTokenResponseData {
                    access_token: token,
                    token_type: None,
                    expires_in: None,
                }
            }),
            test_mode: None,
            description: None,
        })
    }
}

impl
    ForeignTryFrom<(
        grpc_api_types::payouts::PayoutServiceCreateLinkRequest,
        Connectors,
        &MaskedMetadata,
    )> for payouts::payouts_types::PayoutFlowData
{
    type Error = IntegrationError;

    fn foreign_try_from(
        (value, connectors, metadata): (
            grpc_api_types::payouts::PayoutServiceCreateLinkRequest,
            Connectors,
            &MaskedMetadata,
        ),
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        let merchant_id = extract_merchant_id_from_metadata(metadata)?;

        Ok(Self {
            merchant_id,
            payout_id: value.merchant_payout_id.clone().unwrap_or_default(),
            connectors,
            connector_request_reference_id: crate::utils::extract_connector_request_reference_id(
                &value.merchant_payout_id,
            ),
            raw_connector_response: None,
            connector_response_headers: None,
            raw_connector_request: None,
            access_token: value.access_token.map(|token| {
                crate::connector_types::ServerAuthenticationTokenResponseData {
                    access_token: token,
                    token_type: None,
                    expires_in: None,
                }
            }),
            test_mode: None,
            description: None,
        })
    }
}

impl
    ForeignTryFrom<(
        grpc_api_types::payouts::PayoutServiceCreateRecipientRequest,
        Connectors,
        &MaskedMetadata,
    )> for payouts::payouts_types::PayoutFlowData
{
    type Error = IntegrationError;

    fn foreign_try_from(
        (value, connectors, metadata): (
            grpc_api_types::payouts::PayoutServiceCreateRecipientRequest,
            Connectors,
            &MaskedMetadata,
        ),
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        let merchant_id = extract_merchant_id_from_metadata(metadata)?;

        Ok(Self {
            merchant_id,
            payout_id: value.merchant_payout_id.clone().unwrap_or_default(),
            connectors,
            connector_request_reference_id: crate::utils::extract_connector_request_reference_id(
                &value.merchant_payout_id,
            ),
            raw_connector_response: None,
            connector_response_headers: None,
            raw_connector_request: None,
            access_token: value.access_token.map(|token| {
                crate::connector_types::ServerAuthenticationTokenResponseData {
                    access_token: token,
                    token_type: None,
                    expires_in: None,
                }
            }),
            test_mode: None,
            description: None,
        })
    }
}

impl
    ForeignTryFrom<(
        grpc_api_types::payouts::PayoutServiceEnrollDisburseAccountRequest,
        Connectors,
        &MaskedMetadata,
    )> for payouts::payouts_types::PayoutFlowData
{
    type Error = IntegrationError;

    fn foreign_try_from(
        (value, connectors, metadata): (
            grpc_api_types::payouts::PayoutServiceEnrollDisburseAccountRequest,
            Connectors,
            &MaskedMetadata,
        ),
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        let merchant_id = extract_merchant_id_from_metadata(metadata)?;

        Ok(Self {
            merchant_id,
            payout_id: value.merchant_payout_id.clone().unwrap_or_default(),
            connectors,
            connector_request_reference_id: crate::utils::extract_connector_request_reference_id(
                &value.merchant_payout_id,
            ),
            raw_connector_response: None,
            connector_response_headers: None,
            raw_connector_request: None,
            access_token: value.access_token.map(|token| {
                crate::connector_types::ServerAuthenticationTokenResponseData {
                    access_token: token,
                    token_type: None,
                    expires_in: None,
                }
            }),
            test_mode: None,
            description: None,
        })
    }
}

pub fn generate_payout_create_response(
    router_data_v2: crate::router_data_v2::RouterDataV2<
        crate::connector_flow::PayoutCreate,
        super::payouts_types::PayoutFlowData,
        super::payouts_types::PayoutCreateRequest,
        super::payouts_types::PayoutCreateResponse,
    >,
) -> Result<
    grpc_api_types::payouts::PayoutServiceCreateResponse,
    error_stack::Report<crate::errors::ConnectorError>,
> {
    match router_data_v2.response {
        Ok(response) => Ok(grpc_api_types::payouts::PayoutServiceCreateResponse::from(
            response,
        )),
        Err(err) => Ok(grpc_api_types::payouts::PayoutServiceCreateResponse {
            merchant_payout_id: Some(router_data_v2.resource_common_data.payout_id),
            payout_status: Some(
                grpc_api_types::payouts::payout_enums::PayoutStatus::Pending as i32,
            ),
            connector_payout_id: err.connector_transaction_id.clone(),
            error: Some(grpc_api_types::payouts::ErrorInfo {
                unified_details: None,
                connector_details: Some(grpc_api_types::payouts::ConnectorErrorDetails {
                    code: Some(err.code.clone()),
                    message: Some(err.message.clone()),
                    reason: err.reason.clone(),
                    connector_transaction_id: err.connector_transaction_id.clone(),
                }),
                issuer_details: None,
            }),
            status_code: u32::from(err.status_code),
        }),
    }
}

pub fn generate_payout_transfer_response(
    router_data_v2: crate::router_data_v2::RouterDataV2<
        crate::connector_flow::PayoutTransfer,
        super::payouts_types::PayoutFlowData,
        super::payouts_types::PayoutTransferRequest,
        super::payouts_types::PayoutTransferResponse,
    >,
) -> Result<
    grpc_api_types::payouts::PayoutServiceTransferResponse,
    error_stack::Report<crate::errors::ConnectorError>,
> {
    match router_data_v2.response {
        Ok(response) => {
            let payout_status = grpc_api_types::payouts::payout_enums::PayoutStatus::foreign_from(
                response.payout_status,
            ) as i32;
            Ok(grpc_api_types::payouts::PayoutServiceTransferResponse {
                merchant_payout_id: response.merchant_payout_id,
                payout_status: Some(payout_status),
                connector_payout_id: response.connector_payout_id,
                error: None,
                status_code: u32::from(response.status_code),
            })
        }
        Err(err) => Ok(grpc_api_types::payouts::PayoutServiceTransferResponse {
            merchant_payout_id: Some(router_data_v2.resource_common_data.payout_id),
            payout_status: Some(
                grpc_api_types::payouts::payout_enums::PayoutStatus::Pending as i32,
            ),
            connector_payout_id: err.connector_transaction_id.clone(),
            error: Some(grpc_api_types::payouts::ErrorInfo {
                unified_details: None,
                connector_details: Some(grpc_api_types::payouts::ConnectorErrorDetails {
                    code: Some(err.code.clone()),
                    message: Some(err.message.clone()),
                    reason: err.reason.clone(),
                    connector_transaction_id: err.connector_transaction_id.clone(),
                }),
                issuer_details: None,
            }),
            status_code: u32::from(err.status_code),
        }),
    }
}

pub fn generate_payout_get_response(
    router_data_v2: crate::router_data_v2::RouterDataV2<
        crate::connector_flow::PayoutGet,
        super::payouts_types::PayoutFlowData,
        super::payouts_types::PayoutGetRequest,
        super::payouts_types::PayoutGetResponse,
    >,
) -> Result<
    grpc_api_types::payouts::PayoutServiceGetResponse,
    error_stack::Report<crate::errors::ConnectorError>,
> {
    match router_data_v2.response {
        Ok(response) => {
            let payout_status = grpc_api_types::payouts::payout_enums::PayoutStatus::foreign_from(
                response.payout_status,
            ) as i32;
            Ok(grpc_api_types::payouts::PayoutServiceGetResponse {
                merchant_payout_id: response.merchant_payout_id,
                payout_status: Some(payout_status),
                connector_payout_id: response.connector_payout_id,
                error: None,
                status_code: u32::from(response.status_code),
            })
        }
        Err(err) => Ok(grpc_api_types::payouts::PayoutServiceGetResponse {
            merchant_payout_id: Some(router_data_v2.resource_common_data.payout_id),
            payout_status: Some(
                grpc_api_types::payouts::payout_enums::PayoutStatus::Pending as i32,
            ),
            connector_payout_id: err.connector_transaction_id.clone(),
            error: Some(grpc_api_types::payouts::ErrorInfo {
                unified_details: None,
                connector_details: Some(grpc_api_types::payouts::ConnectorErrorDetails {
                    code: Some(err.code.clone()),
                    message: Some(err.message.clone()),
                    reason: err.reason.clone(),
                    connector_transaction_id: err.connector_transaction_id.clone(),
                }),
                issuer_details: None,
            }),
            status_code: u32::from(err.status_code),
        }),
    }
}

pub fn generate_payout_void_response(
    router_data_v2: crate::router_data_v2::RouterDataV2<
        crate::connector_flow::PayoutVoid,
        super::payouts_types::PayoutFlowData,
        super::payouts_types::PayoutVoidRequest,
        super::payouts_types::PayoutVoidResponse,
    >,
) -> Result<
    grpc_api_types::payouts::PayoutServiceVoidResponse,
    error_stack::Report<crate::errors::ConnectorError>,
> {
    match router_data_v2.response {
        Ok(response) => {
            let payout_status = grpc_api_types::payouts::payout_enums::PayoutStatus::foreign_from(
                response.payout_status,
            ) as i32;
            Ok(grpc_api_types::payouts::PayoutServiceVoidResponse {
                merchant_payout_id: response.merchant_payout_id,
                payout_status: Some(payout_status),
                connector_payout_id: response.connector_payout_id,
                error: None,
                status_code: u32::from(response.status_code),
            })
        }
        Err(err) => Ok(grpc_api_types::payouts::PayoutServiceVoidResponse {
            merchant_payout_id: Some(router_data_v2.resource_common_data.payout_id),
            payout_status: Some(
                grpc_api_types::payouts::payout_enums::PayoutStatus::Pending as i32,
            ),
            connector_payout_id: err.connector_transaction_id.clone(),
            error: Some(grpc_api_types::payouts::ErrorInfo {
                unified_details: None,
                connector_details: Some(grpc_api_types::payouts::ConnectorErrorDetails {
                    code: Some(err.code.clone()),
                    message: Some(err.message.clone()),
                    reason: err.reason.clone(),
                    connector_transaction_id: err.connector_transaction_id.clone(),
                }),
                issuer_details: None,
            }),
            status_code: u32::from(err.status_code),
        }),
    }
}

pub fn generate_payout_stage_response(
    router_data_v2: crate::router_data_v2::RouterDataV2<
        crate::connector_flow::PayoutStage,
        super::payouts_types::PayoutFlowData,
        super::payouts_types::PayoutStageRequest,
        super::payouts_types::PayoutStageResponse,
    >,
) -> Result<
    grpc_api_types::payouts::PayoutServiceStageResponse,
    error_stack::Report<crate::errors::ConnectorError>,
> {
    match router_data_v2.response {
        Ok(response) => {
            let payout_status = grpc_api_types::payouts::payout_enums::PayoutStatus::foreign_from(
                response.payout_status,
            ) as i32;
            Ok(grpc_api_types::payouts::PayoutServiceStageResponse {
                merchant_payout_id: response.merchant_payout_id,
                payout_status: Some(payout_status),
                connector_payout_id: response.connector_payout_id,
                error: None,
                status_code: u32::from(response.status_code),
            })
        }
        Err(err) => Ok(grpc_api_types::payouts::PayoutServiceStageResponse {
            merchant_payout_id: Some(router_data_v2.resource_common_data.payout_id),
            payout_status: Some(
                grpc_api_types::payouts::payout_enums::PayoutStatus::Pending as i32,
            ),
            connector_payout_id: err.connector_transaction_id.clone(),
            error: Some(grpc_api_types::payouts::ErrorInfo {
                unified_details: None,
                connector_details: Some(grpc_api_types::payouts::ConnectorErrorDetails {
                    code: Some(err.code.clone()),
                    message: Some(err.message.clone()),
                    reason: err.reason.clone(),
                    connector_transaction_id: err.connector_transaction_id.clone(),
                }),
                issuer_details: None,
            }),
            status_code: u32::from(err.status_code),
        }),
    }
}

pub fn generate_payout_create_link_response(
    router_data_v2: crate::router_data_v2::RouterDataV2<
        crate::connector_flow::PayoutCreateLink,
        super::payouts_types::PayoutFlowData,
        super::payouts_types::PayoutCreateLinkRequest,
        super::payouts_types::PayoutCreateLinkResponse,
    >,
) -> Result<
    grpc_api_types::payouts::PayoutServiceCreateLinkResponse,
    error_stack::Report<crate::errors::ConnectorError>,
> {
    match router_data_v2.response {
        Ok(response) => {
            let payout_status = grpc_api_types::payouts::payout_enums::PayoutStatus::foreign_from(
                response.payout_status,
            ) as i32;
            Ok(grpc_api_types::payouts::PayoutServiceCreateLinkResponse {
                merchant_payout_id: response.merchant_payout_id,
                payout_status: Some(payout_status),
                connector_payout_id: response.connector_payout_id,
                error: None,
                status_code: u32::from(response.status_code),
            })
        }
        Err(err) => Ok(grpc_api_types::payouts::PayoutServiceCreateLinkResponse {
            merchant_payout_id: Some(router_data_v2.resource_common_data.payout_id),
            payout_status: Some(
                grpc_api_types::payouts::payout_enums::PayoutStatus::Pending as i32,
            ),
            connector_payout_id: err.connector_transaction_id.clone(),
            error: Some(grpc_api_types::payouts::ErrorInfo {
                unified_details: None,
                connector_details: Some(grpc_api_types::payouts::ConnectorErrorDetails {
                    code: Some(err.code.clone()),
                    message: Some(err.message.clone()),
                    reason: err.reason.clone(),
                    connector_transaction_id: err.connector_transaction_id.clone(),
                }),
                issuer_details: None,
            }),
            status_code: u32::from(err.status_code),
        }),
    }
}

pub fn generate_payout_create_recipient_response(
    router_data_v2: crate::router_data_v2::RouterDataV2<
        crate::connector_flow::PayoutCreateRecipient,
        super::payouts_types::PayoutFlowData,
        super::payouts_types::PayoutCreateRecipientRequest,
        super::payouts_types::PayoutCreateRecipientResponse,
    >,
) -> Result<
    grpc_api_types::payouts::PayoutServiceCreateRecipientResponse,
    error_stack::Report<crate::errors::ConnectorError>,
> {
    match router_data_v2.response {
        Ok(response) => {
            let payout_status = grpc_api_types::payouts::payout_enums::PayoutStatus::foreign_from(
                response.payout_status,
            ) as i32;
            Ok(
                grpc_api_types::payouts::PayoutServiceCreateRecipientResponse {
                    merchant_payout_id: response.merchant_payout_id,
                    payout_status: Some(payout_status),
                    connector_payout_id: response.connector_payout_id,
                    error: None,
                    status_code: u32::from(response.status_code),
                },
            )
        }
        Err(err) => Ok(
            grpc_api_types::payouts::PayoutServiceCreateRecipientResponse {
                merchant_payout_id: Some(router_data_v2.resource_common_data.payout_id),
                payout_status: Some(
                    grpc_api_types::payouts::payout_enums::PayoutStatus::Pending as i32,
                ),
                connector_payout_id: err.connector_transaction_id.clone(),
                error: Some(grpc_api_types::payouts::ErrorInfo {
                    unified_details: None,
                    connector_details: Some(grpc_api_types::payouts::ConnectorErrorDetails {
                        code: Some(err.code.clone()),
                        message: Some(err.message.clone()),
                        reason: err.reason.clone(),
                        connector_transaction_id: err.connector_transaction_id.clone(),
                    }),
                    issuer_details: None,
                }),
                status_code: u32::from(err.status_code),
            },
        ),
    }
}

pub fn generate_payout_enroll_disburse_account_response(
    router_data_v2: crate::router_data_v2::RouterDataV2<
        crate::connector_flow::PayoutEnrollDisburseAccount,
        super::payouts_types::PayoutFlowData,
        super::payouts_types::PayoutEnrollDisburseAccountRequest,
        super::payouts_types::PayoutEnrollDisburseAccountResponse,
    >,
) -> Result<
    grpc_api_types::payouts::PayoutServiceEnrollDisburseAccountResponse,
    error_stack::Report<crate::errors::ConnectorError>,
> {
    match router_data_v2.response {
        Ok(response) => {
            let payout_status = grpc_api_types::payouts::payout_enums::PayoutStatus::foreign_from(
                response.payout_status,
            ) as i32;
            Ok(
                grpc_api_types::payouts::PayoutServiceEnrollDisburseAccountResponse {
                    merchant_payout_id: response.merchant_payout_id,
                    payout_status: Some(payout_status),
                    connector_payout_id: response.connector_payout_id,
                    error: None,
                    status_code: u32::from(response.status_code),
                },
            )
        }
        Err(err) => Ok(
            grpc_api_types::payouts::PayoutServiceEnrollDisburseAccountResponse {
                merchant_payout_id: Some(router_data_v2.resource_common_data.payout_id),
                payout_status: Some(
                    grpc_api_types::payouts::payout_enums::PayoutStatus::Pending as i32,
                ),
                connector_payout_id: err.connector_transaction_id.clone(),
                error: Some(grpc_api_types::payouts::ErrorInfo {
                    unified_details: None,
                    connector_details: Some(grpc_api_types::payouts::ConnectorErrorDetails {
                        code: Some(err.code.clone()),
                        message: Some(err.message.clone()),
                        reason: err.reason.clone(),
                        connector_transaction_id: err.connector_transaction_id.clone(),
                    }),
                    issuer_details: None,
                }),
                status_code: u32::from(err.status_code),
            },
        ),
    }
}
