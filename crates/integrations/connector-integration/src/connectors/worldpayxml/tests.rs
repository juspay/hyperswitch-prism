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
