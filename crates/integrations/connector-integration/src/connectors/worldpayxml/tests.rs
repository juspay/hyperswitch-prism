use common_enums::{
    CardNetwork, CardSegmentType, CardType, CountryAlpha2, FundingSource, PaymentMethodType,
};
use domain_types::router_data::AdditionalPaymentMethodConnectorResponse;

use super::{
    responses::WorldpayxmlAuthorizeResponse, transformers::get_worldpayxml_connector_response,
};

const CARD_WITH_BIN: &str = r#"
<paymentService version="1.4" merchantCode="fixture-merchant">
  <reply>
    <orderStatus orderCode="card-with-bin">
      <payment>
        <paymentMethod>VISA_DEBIT-SSL</paymentMethod>
        <amount value="5000" currencyCode="GBP" exponent="2" debitCreditIndicator="credit" />
        <lastEvent>AUTHORISED</lastEvent>
        <AuthorisationId id="123456" />
        <cardBin cardClass="D" productType="CP" issuerCountryCode="826" issuerName="BIN issuer" />
      </payment>
    </orderStatus>
  </reply>
</paymentService>
"#;

fn parse_connector_response(
    xml: &str,
    payment_method_type: PaymentMethodType,
) -> AdditionalPaymentMethodConnectorResponse {
    let response: WorldpayxmlAuthorizeResponse = quick_xml::de::from_str(xml)
        .expect("fixture must deserialize as a Worldpay payment response");
    let order = response.reply.order_status.expect("fixture order status");
    let payment = order.payment.as_ref().expect("fixture payment");
    get_worldpayxml_connector_response(payment, order.token.as_ref(), Some(payment_method_type))
        .expect("fixture connector response")
        .additional_payment_method_data
        .expect("fixture additional payment method data")
}

#[test]
fn plain_card_uses_card_bin_attributes() {
    let AdditionalPaymentMethodConnectorResponse::Card {
        processor_card_network,
        card_type,
        funding_source,
        card_segment_type,
        issuer_name,
        issuer_country,
        auth_code,
        ..
    } = parse_connector_response(CARD_WITH_BIN, PaymentMethodType::Card)
    else {
        panic!("expected plain-card response");
    };

    assert_eq!(processor_card_network, Some(CardNetwork::Visa));
    // The amount says credit to the merchant account, but cardClass D identifies a debit card.
    assert_eq!(card_type, Some(CardType::Debit));
    assert_eq!(funding_source, Some(FundingSource::Debit));
    assert_eq!(card_segment_type, Some(CardSegmentType::Commercial));
    assert_eq!(issuer_name.as_deref(), Some("BIN issuer"));
    assert_eq!(issuer_country, Some(CountryAlpha2::GB));
    assert_eq!(auth_code.as_deref(), Some("123456"));
}

const CARD_WITHOUT_BIN: &str = r#"
<paymentService version="1.4" merchantCode="fixture-merchant">
  <reply>
    <orderStatus orderCode="card-without-bin">
      <payment>
        <paymentMethod>VISA_CREDIT-SSL</paymentMethod>
        <amount value="5000" currencyCode="AUD" exponent="2" debitCreditIndicator="credit" />
        <lastEvent>AUTHORISED</lastEvent>
        <AuthorisationId id="123456" />
      </payment>
    </orderStatus>
  </reply>
</paymentService>
"#;

#[test]
fn plain_card_without_bin_leaves_optional_attributes_absent() {
    let AdditionalPaymentMethodConnectorResponse::Card {
        processor_card_network,
        card_type,
        funding_source,
        issuer_name,
        issuer_country,
        ..
    } = parse_connector_response(CARD_WITHOUT_BIN, PaymentMethodType::Card)
    else {
        panic!("expected plain-card response");
    };

    assert_eq!(processor_card_network, Some(CardNetwork::Visa));
    assert_eq!(issuer_name, None);
    assert_eq!(issuer_country, None);
    // Neither the amount's accounting direction nor the scheme code supplies card_type.
    assert_eq!(card_type, None);
    assert_eq!(funding_source, None);
}

const UNKNOWN_CARD_METADATA: &str = r#"
<paymentService version="1.4" merchantCode="fixture-merchant">
  <reply>
    <orderStatus orderCode="unknown-card-metadata">
      <payment>
        <paymentMethod>UNKNOWN-SCHEME-SSL</paymentMethod>
        <lastEvent>AUTHORISED</lastEvent>
        <AuthorisationId id="123456" />
        <cardBin cardClass="UNKNOWN" productType="UNKNOWN" issuerCountryCode="-1" />
      </payment>
    </orderStatus>
  </reply>
</paymentService>
"#;

#[test]
fn unknown_scheme_and_card_class_preserve_the_payment_without_inventing_metadata() {
    let AdditionalPaymentMethodConnectorResponse::Card {
        processor_card_network,
        card_type,
        funding_source,
        card_segment_type,
        issuer_country,
        auth_code,
        ..
    } = parse_connector_response(UNKNOWN_CARD_METADATA, PaymentMethodType::Card)
    else {
        panic!("expected plain-card response");
    };

    assert_eq!(processor_card_network, None);
    assert_eq!(card_type, None);
    assert_eq!(funding_source, None);
    assert_eq!(card_segment_type, None);
    assert_eq!(issuer_country, None);
    assert_eq!(auth_code.as_deref(), Some("123456"));
}

const GOOGLE_PAY_WITH_TOKEN: &str = r#"
<paymentService version="1.4" merchantCode="fixture-merchant">
  <reply>
    <orderStatus orderCode="google-pay-with-token">
      <payment>
        <paymentMethod>GOOGLEPAY-SSL</paymentMethod>
        <amount value="5000" currencyCode="GBP" exponent="2" debitCreditIndicator="credit" />
        <lastEvent>AUTHORISED</lastEvent>
        <AuthorisationId id="123456" />
        <cardBin cardClass="C" productType="CN" issuerCountryCode="826" issuerName="Wallet BIN issuer" />
      </payment>
      <token>
        <tokenDetails tokenEvent="NEW">
          <paymentTokenID>fixture-token</paymentTokenID>
        </tokenDetails>
        <paymentInstrument>
          <emvcoTokenDetails>
            <derived>
              <cardSubBrand>GOLD</cardSubBrand>
            </derived>
          </emvcoTokenDetails>
        </paymentInstrument>
      </token>
    </orderStatus>
  </reply>
</paymentService>
"#;

#[test]
fn google_pay_preserves_token_subtype_and_uses_bin_issuer_attributes() {
    let AdditionalPaymentMethodConnectorResponse::GooglePay {
        auth_code,
        card_subtype,
        card_segment_type,
        funding_source,
        card_type,
        issuer_name,
        issuer_country,
        ..
    } = parse_connector_response(GOOGLE_PAY_WITH_TOKEN, PaymentMethodType::GooglePay)
    else {
        panic!("expected Google Pay response");
    };

    assert_eq!(auth_code.as_deref(), Some("123456"));
    assert_eq!(card_subtype.as_deref(), Some("GOLD"));
    assert_eq!(card_segment_type, Some(CardSegmentType::Consumer));
    assert_eq!(funding_source, Some(FundingSource::Credit));
    assert_eq!(card_type, Some(CardType::Credit));
    assert_eq!(issuer_name.as_deref(), Some("Wallet BIN issuer"));
    assert_eq!(issuer_country, Some(CountryAlpha2::GB));
}

#[test]
fn refused_and_pending_cards_keep_metadata_without_an_authorisation_id() {
    for last_event in ["REFUSED", "SENT_FOR_AUTHORISATION"] {
        let xml = CARD_WITH_BIN
            .replace("<AuthorisationId id=\"123456\" />", "")
            .replace(
                "<lastEvent>AUTHORISED</lastEvent>",
                &format!("<lastEvent>{last_event}</lastEvent>"),
            );
        let AdditionalPaymentMethodConnectorResponse::Card {
            auth_code,
            processor_card_network,
            card_type,
            funding_source,
            card_segment_type,
            issuer_name,
            issuer_country,
            ..
        } = parse_connector_response(&xml, PaymentMethodType::Card)
        else {
            panic!("expected plain-card response");
        };

        assert_eq!(auth_code, None);
        assert_eq!(processor_card_network, Some(CardNetwork::Visa));
        assert_eq!(card_type, Some(CardType::Debit));
        assert_eq!(funding_source, Some(FundingSource::Debit));
        assert_eq!(card_segment_type, Some(CardSegmentType::Commercial));
        assert_eq!(issuer_name.as_deref(), Some("BIN issuer"));
        assert_eq!(issuer_country, Some(CountryAlpha2::GB));
    }
}

#[test]
fn pending_wallets_keep_token_and_bin_metadata_without_an_authorisation_id() {
    for (scheme, payment_method_type) in [
        ("GOOGLEPAY-SSL", PaymentMethodType::GooglePay),
        ("APPLEPAY-SSL", PaymentMethodType::ApplePay),
    ] {
        let xml = GOOGLE_PAY_WITH_TOKEN
            .replace("<AuthorisationId id=\"123456\" />", "")
            .replace("GOOGLEPAY-SSL", scheme)
            .replace(
                "<lastEvent>AUTHORISED</lastEvent>",
                "<lastEvent>SENT_FOR_AUTHORISATION</lastEvent>",
            );
        let response = parse_connector_response(&xml, payment_method_type);
        let (auth_code, subtype, segment, funding, issuer, country) = match (scheme, response) {
            (
                "GOOGLEPAY-SSL",
                AdditionalPaymentMethodConnectorResponse::GooglePay {
                    auth_code,
                    card_subtype,
                    card_segment_type,
                    funding_source,
                    card_type,
                    issuer_name,
                    issuer_country,
                    ..
                },
            ) => {
                assert_eq!(card_type, Some(CardType::Credit));
                (
                    auth_code,
                    card_subtype,
                    card_segment_type,
                    funding_source,
                    issuer_name,
                    issuer_country,
                )
            }
            (
                "APPLEPAY-SSL",
                AdditionalPaymentMethodConnectorResponse::ApplePay {
                    auth_code,
                    card_subtype,
                    card_segment_type,
                    funding_source,
                    issuer_name,
                    issuer_country,
                    ..
                },
            ) => (
                auth_code,
                card_subtype,
                card_segment_type,
                funding_source,
                issuer_name,
                issuer_country,
            ),
            (_, other) => panic!("unexpected wallet response: {other:?}"),
        };

        assert_eq!(auth_code, None);
        assert_eq!(subtype.as_deref(), Some("GOLD"));
        assert_eq!(segment, Some(CardSegmentType::Consumer));
        assert_eq!(funding, Some(FundingSource::Credit));
        assert_eq!(issuer.as_deref(), Some("Wallet BIN issuer"));
        assert_eq!(country, Some(CountryAlpha2::GB));
    }
}

#[test]
fn card_bin_parses_country_codes_and_leaves_unknown_codes_absent() {
    for (country_code, expected_country) in [
        ("036", Some(CountryAlpha2::AU)),
        ("AU", Some(CountryAlpha2::AU)),
        ("-1", None),
        ("999", None),
    ] {
        let xml = CARD_WITH_BIN.replace(
            "issuerCountryCode=\"826\"",
            &format!("issuerCountryCode=\"{country_code}\""),
        );
        let AdditionalPaymentMethodConnectorResponse::Card { issuer_country, .. } =
            parse_connector_response(&xml, PaymentMethodType::Card)
        else {
            panic!("expected plain-card response");
        };

        assert_eq!(issuer_country, expected_country);
    }
}

mod sender_party {
    use domain_types::{
        connector_types::{
            CorporateSenderDetails, CustomerInfo, IndividualSenderDetails, PaymentFlowData,
            SenderAddress, SenderDetails,
        },
        errors::IntegrationError,
        payment_address::{Address, AddressDetails, PaymentAddress, PhoneDetails},
        types::Connectors,
    };
    use hyperswitch_masking::Secret;

    use super::super::transformers::build_worldpayxml_sender_party;

    const ACCOUNT_REFERENCE: &str =
        r#"<accountReference accountType="03">4111111111111111</accountReference>"#;
    const SENDER_ADDRESS_XML: &str = "<fundingAddress><address1>10 Downing St</address1>\
        <postalCode>SW1A 2AA</postalCode><city>London</city><state>Westminster</state>\
        <countryCode>GB</countryCode></fundingAddress>";
    const BILLING_ADDRESS_XML: &str = "<fundingAddress><address1>1 Billing St</address1>\
        <postalCode>10001</postalCode><city>New York</city><state>NY</state>\
        <countryCode>US</countryCode></fundingAddress>";

    fn secret(value: &str) -> Secret<String> {
        Secret::new(value.to_string())
    }

    /// A complete US billing address (Jane Doe, New York) with a phone number.
    fn billing_address() -> Address {
        Address {
            address: Some(AddressDetails {
                first_name: Some(secret("Jane")),
                last_name: Some(secret("Doe")),
                line1: Some(secret("1 Billing St")),
                line2: None,
                line3: None,
                city: Some(secret("New York")),
                state: Some(secret("NY")),
                zip: Some(secret("10001")),
                country: Some(common_enums::CountryAlpha2::US),
                origin_zip: None,
            }),
            phone: Some(PhoneDetails {
                number: Some(secret("2125550100")),
                country_code: Some("+1".to_string()),
            }),
            email: None,
        }
    }

    /// Payment flow data whose only meaningful content is the given billing address.
    fn payment_flow_data(billing: Option<Address>) -> PaymentFlowData {
        PaymentFlowData {
            merchant_id: common_utils::id_type::MerchantId::default(),
            customer_id: None,
            connector_customer: None,
            payment_id: "pay_test".to_string(),
            attempt_id: "attempt_test".to_string(),
            status: common_enums::AttemptStatus::Pending,
            payment_method: common_enums::PaymentMethod::Card,
            payment_method_type: None,
            description: None,
            return_url: None,
            address: PaymentAddress::new(None, billing, None, None),
            auth_type: common_enums::AuthenticationType::NoThreeDs,
            connector_feature_data: None,
            amount_captured: None,
            amount_capturable: None,
            amount: None,
            access_token: None,
            session_token: None,
            reference_id: None,
            connector_order_id: None,
            preprocessing_id: None,
            connector_api_version: None,
            connector_request_reference_id: "ref_test".to_string(),
            test_mode: None,
            connector_http_status_code: None,
            connector_response_headers: None,
            external_latency: None,
            connectors: Connectors::default().into(),
            raw_connector_response: None,
            typed_connector_response: None,
            raw_connector_request: None,
            typed_connector_request: None,
            vault_headers: None,
            connector_response: None,
            recurring_mandate_payment_data: None,
            order_details: None,
            amount_authorized: None,
            l2_l3_data: None,
            merchant_request_id: None,
            sender_payment_instrument_id: None,
            connector_returned_payment_method_details: None,
            settlement_status: None,
            raw_connector_status: None,
        }
    }

    fn sender_address() -> SenderAddress {
        SenderAddress {
            line1: Some(secret("10 Downing St")),
            line2: None,
            line3: None,
            city: Some(secret("London")),
            state: Some(secret("Westminster")),
            zip: Some(secret("SW1A 2AA")),
            country: Some(common_enums::CountryAlpha2::GB),
        }
    }

    fn customer(date_of_birth: Option<time::Date>) -> CustomerInfo {
        CustomerInfo {
            customer_id: None,
            customer_email: None,
            customer_name: None,
            first_name: None,
            last_name: None,
            customer_phone_number: None,
            customer_phone_country_code: None,
            salutation: None,
            date_of_birth: date_of_birth.map(Secret::new),
        }
    }

    fn corporate(company_name: &str, phone_number: Option<&str>) -> SenderDetails {
        SenderDetails::Corporate(CorporateSenderDetails {
            company_name: secret(company_name),
            address: Some(sender_address()),
            email: None,
            phone_number: phone_number.map(secret),
            phone_country_code: None,
        })
    }

    fn sender_party_xml(
        sender_details: Option<&SenderDetails>,
        customer: Option<&CustomerInfo>,
    ) -> Result<String, error_stack::Report<IntegrationError>> {
        build_worldpayxml_sender_party(
            &payment_flow_data(Some(billing_address())),
            sender_details,
            customer,
            secret("4111111111111111"),
        )
        .map(|party| {
            quick_xml::se::to_string_with_root("fundingParty", &party)
                .expect("funding party serializes")
        })
    }

    fn expected_xml(full_name: &str, funding_address: &str, funding_data: &str) -> String {
        format!(
            r#"<fundingParty type="sender">{ACCOUNT_REFERENCE}<fullName>{full_name}</fullName>{funding_address}{funding_data}</fundingParty>"#
        )
    }

    fn missing_field(error: &error_stack::Report<IntegrationError>) -> &'static str {
        match error.current_context() {
            IntegrationError::MissingRequiredField { field_name, .. } => field_name,
            other => panic!("expected a missing field error, got {other:?}"),
        }
    }

    #[test]
    fn individual_from_sender_details() {
        let sender = SenderDetails::Individual(IndividualSenderDetails {
            first_name: secret("John"),
            last_name: secret("Smith"),
            date_of_birth: Some(Secret::new(time::macros::date!(1985 - 04 - 12))),
            address: Some(sender_address()),
            email: None,
            phone_number: Some(secret("7700900000")),
            phone_country_code: None,
        });
        assert_eq!(
            sender_party_xml(Some(&sender), None).expect("sender builds"),
            expected_xml(
                "<first>John</first><last>Smith</last>",
                SENDER_ADDRESS_XML,
                r#"<fundingData><birthDate><date dayOfMonth="12" month="4" year="1985"/></birthDate><telephoneNumber>7700900000</telephoneNumber></fundingData>"#,
            )
        );
    }

    #[test]
    fn individual_falls_back_to_customer_date_of_birth_and_billing_phone() {
        let sender = SenderDetails::Individual(IndividualSenderDetails {
            first_name: secret("John"),
            last_name: secret("Smith"),
            date_of_birth: None,
            address: Some(sender_address()),
            email: None,
            phone_number: None,
            phone_country_code: None,
        });
        let customer = customer(Some(time::macros::date!(1970 - 01 - 02)));
        assert_eq!(
            sender_party_xml(Some(&sender), Some(&customer)).expect("sender builds"),
            expected_xml(
                "<first>John</first><last>Smith</last>",
                SENDER_ADDRESS_XML,
                r#"<fundingData><birthDate><date dayOfMonth="2" month="1" year="1970"/></birthDate><telephoneNumber>2125550100</telephoneNumber></fundingData>"#,
            )
        );
    }

    #[test]
    fn corporate_with_phone_number() {
        assert_eq!(
            sender_party_xml(
                Some(&corporate("Example Trading Ltd", Some("7700900000"))),
                None
            )
            .expect("sender builds"),
            expected_xml(
                "<first>Example</first><middle>Trading</middle><last>Ltd</last>",
                SENDER_ADDRESS_XML,
                "<fundingData><telephoneNumber>7700900000</telephoneNumber></fundingData>",
            )
        );
    }

    #[test]
    fn corporate_without_phone_number_has_no_funding_data() {
        // No billing phone fallback and no date of birth for corporate senders.
        let customer = customer(Some(time::macros::date!(1970 - 01 - 02)));
        assert_eq!(
            sender_party_xml(
                Some(&corporate("Example  Global   Trading Ltd", None)),
                Some(&customer)
            )
            .expect("sender builds"),
            expected_xml(
                "<first>Example</first><middle>Global Trading</middle><last>Ltd</last>",
                SENDER_ADDRESS_XML,
                "",
            )
        );
    }

    #[test]
    fn single_word_company_name_is_rejected_as_invalid_format() {
        for company_name in ["Acme", "   "] {
            let error = sender_party_xml(Some(&corporate(company_name, None)), None)
                .expect_err("a last name is required");
            assert!(matches!(
                error.current_context(),
                IntegrationError::InvalidDataFormat {
                    field_name: "sender_details.company_name",
                    ..
                }
            ));
        }
    }

    #[test]
    fn without_sender_details_the_request_is_unchanged() {
        let customer = customer(Some(time::macros::date!(1985 - 04 - 12)));
        assert_eq!(
            sender_party_xml(None, Some(&customer)).expect("sender builds"),
            expected_xml(
                "<first>Jane</first><last>Doe</last>",
                BILLING_ADDRESS_XML,
                r#"<fundingData><birthDate><date dayOfMonth="12" month="4" year="1985"/></birthDate><telephoneNumber>2125550100</telephoneNumber></fundingData>"#,
            )
        );
        // The date of birth stays optional.
        assert_eq!(
            sender_party_xml(None, None).expect("sender builds"),
            expected_xml(
                "<first>Jane</first><last>Doe</last>",
                BILLING_ADDRESS_XML,
                "<fundingData><telephoneNumber>2125550100</telephoneNumber></fundingData>",
            )
        );
    }

    #[test]
    fn sender_details_require_an_address() {
        let sender = SenderDetails::Corporate(CorporateSenderDetails {
            company_name: secret("Acme Ltd"),
            address: None,
            email: None,
            phone_number: None,
            phone_country_code: None,
        });
        let error = sender_party_xml(Some(&sender), None).expect_err("address is required");
        assert_eq!(missing_field(&error), "sender_details.address");
    }
}
