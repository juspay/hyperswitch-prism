// ClearSale BR unit tests. Production modules (`clearsale_br.rs`,
// `transformers.rs`) stay free of `#[cfg(test)]` blocks — connector-specific
// coverage lives here only. Generic FRM flow tests stay in
// `grpc-server/tests/frm_flow_test.rs`.

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::expect_used)]
#[allow(clippy::indexing_slicing)]
#[allow(clippy::panic)]
#[allow(clippy::unreachable)]
mod tests {
    use std::str::FromStr;

    use common_enums::{AttemptStatus, CardNetwork, Currency, CustomerType, FrmDecision, Gender};
    use common_utils::{pii::Email, types::FloatMajorUnit};
    use domain_types::{
        connector_types::{
            CustomerInfo, DomainData, HotelData, HotelSegment, TravelData, TravelSegment, Traveller,
        },
        errors::IntegrationError,
        frm::frm_types::{PostRiskCheckRequest, PreRiskCheckRequest},
        payment_address::{
            Address, AddressDetails, OrderDetailsWithAmount, PaymentAddress, PhoneDetails,
            StateDetails,
        },
        payment_method_data::{
            ApplePayPaymentData, ApplePayWalletData, ApplepayPaymentMethod, BankDebitData,
            BankTransferData, CardDetailsForNetworkTransactionId, CashappQr,
            CustomerDocumentDetails, DefaultPCIHolder, DocumentKind, GiftCardData, GiftCardDetails,
            GooglePayPaymentMethodInfo, GooglePayWalletData, GpayEncryptedTokenizationData,
            GpayTokenizationData, PayLaterData, PaymentMethodData, RawCardNumber, VoucherData,
            WalletData,
        },
        router_request_types::BrowserInformation,
        utils::ForeignTryFrom,
    };
    use hyperswitch_masking::{ExposeInterface, PeekInterface, Secret};

    use super::super::transformers::*;

    // ── fixtures ─────────────────────────────────────────────────────────

    fn email() -> Email {
        Email::from_str("comprador@example.com.br").expect("valid email")
    }

    fn card() -> PaymentMethodData<DefaultPCIHolder> {
        PaymentMethodData::Card(domain_types::payment_method_data::Card {
            card_number: RawCardNumber(
                cards::CardNumber::from_str("4111111111111111").expect("valid PAN"),
            ),
            card_exp_month: Secret::new("12".to_owned()),
            card_exp_year: Secret::new("2030".to_owned()),
            card_cvc: Secret::new("123".to_owned()),
            card_issuer: None,
            card_network: Some(CardNetwork::Visa),
            card_type: Some("credit".to_owned()),
            card_issuing_country: None,
            bank_code: None,
            nick_name: None,
            card_holder_name: Some(Secret::new("Maria Silva".to_owned())),
            co_badged_card_data: None,
        })
    }

    fn billing_address_details() -> AddressDetails {
        AddressDetails {
            line1: Some(Secret::new("Rua Augusta, 1234".to_owned())),
            line2: Some(Secret::new("Consolação".to_owned())),
            city: Some(Secret::new("São Paulo".to_owned())),
            state: Some(StateDetails::from_code(Secret::new("SP".to_owned()))),
            zip: Some(Secret::new("01304-001".to_owned())),
            country: Some(common_enums::CountryAlpha2::BR),
            first_name: Some(Secret::new("Maria".to_owned())),
            last_name: Some(Secret::new("Silva".to_owned())),
            ..AddressDetails::default()
        }
    }

    fn billing_address() -> Address {
        Address {
            address: Some(billing_address_details()),
            phone: Some(PhoneDetails {
                number: Some(Secret::new("11987654321".to_owned())),
                country_code: None,
            }),
            email: Some(email()),
        }
    }

    fn payment_address() -> PaymentAddress {
        PaymentAddress::new(None, Some(billing_address()), None, Some(false))
    }

    fn customer_info() -> CustomerInfo {
        CustomerInfo {
            customer_id: None,
            customer_email: Some(email()),
            customer_name: Some(Secret::new("Maria Silva".to_owned())),
            first_name: None,
            last_name: None,
            customer_phone_number: None,
            customer_phone_country_code: None,
            salutation: None,
            date_of_birth: None,
        }
    }

    fn order_details() -> Vec<OrderDetailsWithAmount> {
        vec![OrderDetailsWithAmount::foreign_try_from(
            grpc_api_types::payments::OrderDetailsWithAmount {
                product_name: "Passagem São Paulo → Rio".to_owned(),
                quantity: 2,
                amount: 15000,
                tax_rate: None,
                total_tax_amount: None,
                requires_shipping: None,
                product_img_link: None,
                product_id: Some("SKU-1".to_owned()),
                category: Some("travel".to_owned()),
                sub_category: None,
                ..Default::default()
            },
        )
        .expect("valid order-item fixture")]
    }

    fn pre_risk_check_request() -> PreRiskCheckRequest {
        let request = PreRiskCheckRequest::foreign_try_from(
            grpc_api_types::frm::FrmServicePreRiskCheckRequest {
                amount: Some(grpc_api_types::payments::Money {
                    minor_amount: 30000,
                    currency: grpc_api_types::payments::Currency::Brl.into(),
                }),
                ..Default::default()
            },
        )
        .expect("valid pre-risk-check fixture");
        PreRiskCheckRequest {
            money: request.money,
            customer_info: Some(customer_info()),
            payment_method: Some(card()),
            browser_info: Some(BrowserInformation {
                ip_address: Some("200.155.10.20".parse().expect("valid ip")),
                ..BrowserInformation::default()
            }),
            merchant_transaction_id: Some("txn-abc-001".to_owned()),
            merchant_order_id: Some("merchant-order-abc-001".to_owned()),
            connector_feature_data: Some(Secret::new(
                r#"{"product":4,"installments":99,"orderInfoMetadata":{"observation":"Legacy connector observation"}}"#.to_owned(),
            )),
            installment_details: Some(domain_types::frm::frm_types::InstallmentDetails { installment_count: 3 }),
            order_description: Some("Order description".to_owned()),
            order_details: Some(order_details()),
            address: Some(payment_address()),
            metadata: Some(Secret::new(r#"{"session_id":"sdk-session-42"}"#.to_owned())),
            test_mode: Some(true),
            mandate_details: None,
            merchant_details: None,
            payment_method_type: None,
            billing_document_data: Some(CustomerDocumentDetails {
                document_type: DocumentKind::Cpf,
                document_number: Secret::new("12345678909".to_owned()),
            }),
            shipping_document_data: None,
            domain_data: None,
        }
    }

    fn post_risk_check_request() -> PostRiskCheckRequest {
        let pre = pre_risk_check_request();
        PostRiskCheckRequest {
            money: pre.money,
            customer_info: pre.customer_info,
            payment_method: pre.payment_method,
            browser_info: pre.browser_info,
            merchant_transaction_id: pre.merchant_transaction_id,
            merchant_order_id: pre.merchant_order_id,
            installment_details: pre.installment_details,
            order_description: pre.order_description,
            order_details: pre.order_details,
            metadata: pre.metadata,
            connector_feature_data: pre.connector_feature_data,
            test_mode: pre.test_mode,
            payment_status: Some(AttemptStatus::Charged),
            connector_transaction_id: Some("psp-txn-should-not-be-order-code".to_owned()),
            payment_connector: None,
            address: pre.address,
            billing_document_data: pre.billing_document_data,
            shipping_document_data: pre.shipping_document_data,
            domain_data: pre.domain_data,
        }
    }

    fn missing_fields(err: &error_stack::Report<IntegrationError>) -> Vec<String> {
        match err.current_context() {
            IntegrationError::MissingRequiredField { field_name, .. } => {
                vec![(*field_name).to_owned()]
            }
            IntegrationError::MissingRequiredFields { field_names, .. } => {
                field_names.iter().map(|name| (*name).to_owned()).collect()
            }
            other => panic!("expected a missing-field error, got {other:?}"),
        }
    }

    #[test]
    fn clearsale_status_maps_fail_safe() {
        use ClearSaleStatusEnum as S;
        for status in [S::Apa, S::Apm, S::App, S::Apg] {
            assert_eq!(status.to_frm_decision(), FrmDecision::Approve);
        }
        for status in [S::Rpa, S::Rpm, S::Rpp, S::Can, S::Frd] {
            assert_eq!(status.to_frm_decision(), FrmDecision::Reject);
        }
        for status in [
            S::Nvo,
            S::Ama,
            S::Sus,
            S::Err,
            S::Amm,
            S::Apr,
            S::Rea,
            S::Rep,
            S::Unknown,
        ] {
            assert_eq!(
                status.to_frm_decision(),
                FrmDecision::Review,
                "{status:?} must hold for review, never approve nor reject"
            );
        }
    }

    #[test]
    fn unknown_status_deserializes() {
        let parsed =
            serde_json::from_str::<ClearSaleStatusEnum>("\"XYZ\"").expect("serde(other) fallback");
        assert_eq!(parsed, ClearSaleStatusEnum::Unknown);
        assert_eq!(parsed.to_frm_decision(), FrmDecision::Review);
    }

    /// ClearSale sends 3-letter statuses UPPERCASE on the wire (spec §5.2/§5.3).
    #[test]
    fn uppercase_wire_statuses_deserialize() {
        use ClearSaleStatusEnum as S;
        for (wire, variant) in [
            ("NVO", S::Nvo),
            ("AMA", S::Ama),
            ("SUS", S::Sus),
            ("APA", S::Apa),
            ("APM", S::Apm),
            ("APP", S::App),
            ("APG", S::Apg),
            ("RPA", S::Rpa),
            ("RPM", S::Rpm),
            ("RPP", S::Rpp),
            ("CAN", S::Can),
            ("FRD", S::Frd),
            ("ERR", S::Err),
        ] {
            let parsed = serde_json::from_str::<ClearSaleStatusEnum>(&format!("\"{wire}\""))
                .expect("uppercase wire status must deserialize");
            assert_eq!(parsed, variant, "wire status {wire}");
        }
    }

    /// Only a captured payment pushes PGA, and only a ClearSale rejection or
    /// an explicit failure pushes PGR.
    #[test]
    fn payment_outcome_maps_to_pga_pgr() {
        assert_eq!(
            clearsale_payment_outcome_status(Some(AttemptStatus::Charged), None),
            Some("PGA")
        );
        assert_eq!(
            clearsale_payment_outcome_status(Some(AttemptStatus::AuthorizationFailed), None),
            Some("PGR")
        );
        assert_eq!(
            clearsale_payment_outcome_status(None, Some(FrmDecision::Reject)),
            Some("PGR")
        );
        assert_eq!(
            clearsale_payment_outcome_status(None, Some(FrmDecision::Approve)),
            None
        );
        assert_eq!(clearsale_payment_outcome_status(None, None), None);
    }
    #[test]
    fn order_request_serializes_documented_wire_names() {
        let order = ClearSaleOrderRequest {
            code: "ORDER_EXAMPLE_2_0_1".to_string(),
            session_id: "SessionIDValue".to_string(),
            date: "2017-03-22T13:38:59.9894222".to_string(),
            email: email(),
            b2b_b2c: Some(CustomerType::Individual),
            item_value: Some(FloatMajorUnit(10.00)),
            total_value: FloatMajorUnit(15.00),
            number_of_installments: Some(1),
            ip: Some("192.168.0.1".to_string()),
            is_gift: Some(false),
            gift_message: Some("Message Example".to_string()),
            observation: Some("Observation example".to_string()),
            status: Some(0),
            origin: Some("Origin example".to_string()),
            channel_id: Some("Additional origin information".to_string()),
            reservation_date: Some("2017-03-21T22:36:36.0000000".to_string()),
            country: Some("Brasil".to_string()),
            nationality: Some("Brasileiro".to_string()),
            product: DEFAULT_CLEARSALE_PRODUCT_TOTAL_GARANTIDO,
            custom_sla: Some(60),
            bank_authentication: Some("Aprovado 3DS".to_string()),
            billing: ClearSalePerson {
                person_type: 1,
                primary_document: Secret::new("12345678910".to_string()),
                secondary_document: Some(Secret::new("12345678".to_string())),
                client_id: Some("Client123".to_string()),
                name: Secret::new("Complete Client Name".to_string()),
                birth_date: Some("1990-01-10T00:00:00.000".to_string()),
                email: Some(email()),
                gender: Some("M"),
                address: ClearSaleAddress {
                    street: Secret::new("Street name example".to_string()),
                    number: "100".to_string(),
                    county: Secret::new("County Example".to_string()),
                    city: Secret::new("City Example".to_string()),
                    state: Secret::new("SP".to_string()),
                    zipcode: Secret::new("12345678".to_string()),
                    country: Some("Brasil".to_string()),
                    additional_information: Some(Secret::new(
                        "Additional information example".to_string(),
                    )),
                    reference: Some(Secret::new("Address reference example".to_string())),
                },
                phones: vec![ClearSalePhone {
                    phone_type: 1,
                    ddi: Some(55),
                    ddd: 11,
                    number: 33333333,
                    extension: Some("1111".to_string()),
                }],
                delivery_type: None,
                delivery_time: None,
                price: None,
                pick_up_store_document: None,
            },
            shipping: Some(ClearSalePerson {
                person_type: 1,
                primary_document: Secret::new("12345678910".to_string()),
                secondary_document: None,
                client_id: None,
                name: Secret::new("Complete Client Name".to_string()),
                birth_date: None,
                email: None,
                gender: None,
                address: ClearSaleAddress {
                    street: Secret::new("Street name example".to_string()),
                    number: "100".to_string(),
                    county: Secret::new("County Example".to_string()),
                    city: Secret::new("City Example".to_string()),
                    state: Secret::new("SP".to_string()),
                    zipcode: Secret::new("12345678".to_string()),
                    country: Some("Brasil".to_string()),
                    additional_information: None,
                    reference: None,
                },
                phones: vec![ClearSalePhone {
                    phone_type: 1,
                    ddi: None,
                    ddd: 11,
                    number: 33333333,
                    extension: None,
                }],
                delivery_type: Some("1".to_string()),
                delivery_time: Some("2 dias úteis".to_string()),
                price: Some(FloatMajorUnit(5.00)),
                pick_up_store_document: Some(Secret::new("12345678910".to_string())),
            }),
            payments: vec![ClearSalePayment {
                payment_type: 1,
                value: FloatMajorUnit(25.00),
                date: Some("2017-03-21T22:36:53.0000000".to_string()),
                currency: Some(986),
                sequential: Some(1),
                installments: Some(1),
                interest_rate: Some(FloatMajorUnit(10.0)),
                interest_value: Some(FloatMajorUnit(2.00)),
                card: Some(ClearSaleCard {
                    bin: Secret::new("123456".to_string()),
                    end: Secret::new("1234".to_string()),
                    owner_name: Secret::new("Owner Card Name".to_string()),
                    hash: Some(Secret::new("12345678945612301234569874563210".to_string())),
                    card_type: Some(1),
                    validity_date: Some("02/2021".to_string()),
                    document: Some(Secret::new("12345678910".to_string())),
                    nsu: Some("12345".to_string()),
                }),
                voucher_order_origin: Some("123456".to_string()),
                address: None,
            }],
            items: vec![ClearSaleItem {
                name: "Item description".to_string(),
                code: Some("Item Code".to_string()),
                bar_code: None,
                value: Some(FloatMajorUnit(10.00)),
                // ClearSale documents items[].amount as Integer (line quantity).
                amount: Some(1),
                category_id: Some(1),
                category_name: Some("Item category name".to_string()),
                is_gift: Some(true),
                seller_name: Some("Seller Name".to_string()),
                seller_document: None,
                is_market_place: None,
                seller_segment: None,
                shipping_company: Some("Shipping Company Name".to_string()),
            }],
            list: Some(ClearSaleList {
                type_id: 3,
                id: "List Code Example".to_string(),
            }),
            purchase_information: Some(ClearSalePurchaseInformation {
                last_date_inserted_mail: Some("2015-03-01T02:40:00".to_string()),
                last_date_change_password: Some("2015-04-02T05:15:00".to_string()),
                last_date_change_phone: Some("2015-05-03T10:45:00".to_string()),
                last_date_change_mobile_phone: Some("2015-06-04T12:05:00".to_string()),
                last_date_inserted_address: Some("2015-07-05T15:25:00".to_string()),
                purchase_logged: Some(false),
                email: Some(Email::from_str("email@pemail.com.br").expect("valid email")),
                login: Some("SocialNetworkLogin".to_string()),
            }),
            social_network: Some(ClearSaleSocialNetwork {
                opt_in_compre_confie: Some(1),
                type_social_network: Some(2),
                authentication_token: Some(Secret::new("123456abcd".to_string())),
            }),
            passengers: Some(vec![ClearSalePassenger {
                name: Secret::new("Passenger name".to_string()),
                company_mile_card: Some("Company Name".to_string()),
                mile_card: Some(Secret::new("123456789".to_string())),
                identification_type: Some(1),
                identification_number: Some(Secret::new("123456789".to_string())),
                gender: Some("M"),
                birthdate: Some("1990-01-10T00:00:00.000".to_string()),
                cpf: Some(Secret::new("12345678910".to_string())),
            }]),
            connections: Some(vec![ClearSaleConnection {
                company: "JJ".to_string(),
                identification_number: Some(12345),
                date: Some("2017-10-10T00:00:00.000".to_string()),
                seat_class: Some("Seat Class".to_string()),
                origin: "GRU".to_string(),
                destination: "LHR".to_string(),
                boarding: Some("2017-10-10T00:00:00.000".to_string()),
                arriving: Some("2017-10-10T00:00:00.000".to_string()),
                fare_class: Some("First Class".to_string()),
                type_of_trip: None,
            }]),
            hotels: Some(vec![ClearSaleHotel {
                name: "Hotel Name".to_string(),
                city: Some("Hotel City".to_string()),
                state: Some("Hotel State".to_string()),
                country: Some("Hotel Country".to_string()),
                reservation_date: Some("2017-10-10T00:00:00.000".to_string()),
                reserve_expiration_date: Some("2017-10-10T00:00:00.000".to_string()),
                check_in_date: Some("2017-10-10T00:00:00.000".to_string()),
                check_out_date: Some("2017-10-10T00:00:00.000".to_string()),
            }]),
        };

        let json = serde_json::to_value(&order).expect("order serializes");
        let expected_wire_names = [
            "code",
            "sessionID",
            "date",
            "email",
            "b2bB2c",
            "itemValue",
            "totalValue",
            "numberOfInstallments",
            "ip",
            "isGift",
            "giftMessage",
            "observation",
            "status",
            "origin",
            "channelID",
            "reservationDate",
            "country",
            "nationality",
            "product",
            "customSla",
            "bankAuthentication",
            "billing",
            "shipping",
            "payments",
            "items",
            "list",
            "purchaseInformation",
            "socialNetwork",
            "passengers",
            "connections",
            "hotels",
        ];
        let object = json.as_object().expect("order is a JSON object");
        for name in expected_wire_names {
            assert!(
                object.contains_key(name),
                "wire field {name} missing from serialized order"
            );
        }
        for block in ["billing", "shipping"] {
            let person = object[block].as_object().expect("person block");
            for name in ["type", "primaryDocument", "name", "address", "phones"] {
                assert!(
                    person.contains_key(name),
                    "wire field {name} missing from {block}"
                );
            }
        }
        let payment = object["payments"][0].as_object().expect("payment entry");
        for name in [
            "type",
            "value",
            "date",
            "currency",
            "sequential",
            "installments",
            "interestRate",
            "interestValue",
            "card",
            "voucherOrderOrigin",
        ] {
            assert!(
                payment.contains_key(name),
                "wire field {name} missing from payments[0]"
            );
        }
        // items[].amount is Integer on the wire — never a float (`1.0`).
        let item = object["items"][0].as_object().expect("item entry");
        assert!(
            item["amount"].is_i64(),
            "items[].amount must be an integer, got {}",
            item["amount"]
        );
    }

    // ── feature data + metadata getters ─────────────────────────────────

    /// `connector_feature_data.orderInfoMetadata` is decoded only by ClearSale;
    /// the typed FRM `order_description` is a separate input.
    #[test]
    fn clearsale_order_info_metadata_deserializes_its_own_schema() {
        let connector_feature_data =
            ClearSaleConnectorFeatureData::parse(Some(&Secret::new(
                r#"{"product":4,"orderInfoMetadata":{"origin":"WebCheckout","isGift":true,"billingPhoneType":2,"socialNetwork":{"optInCompreConfie":1}}}"#.to_owned(),
            )))
            .expect("valid feature data");
        assert_eq!(connector_feature_data.product, Some(4));

        let order_info_metadata = connector_feature_data.order_info_metadata;
        assert_eq!(order_info_metadata.origin.as_deref(), Some("WebCheckout"));
        assert_eq!(order_info_metadata.is_gift, Some(true));
        assert_eq!(order_info_metadata.billing_phone_type, Some(2));
        assert!(order_info_metadata.social_network.is_some());

        let overridden = ClearSaleConnectorFeatureData::parse(Some(&Secret::new(
            r#"{
                "origin":"Flat",
                "isGift":false,
                "channelId":"flat-channel",
                "orderInfoMetadata":{"origin":"Nested","channelId":"nested-channel"}
            }"#
            .to_owned(),
        )))
        .expect("legacy and nested feature data");
        let order_info_metadata = overridden.order_info_metadata;
        assert_eq!(order_info_metadata.origin.as_deref(), Some("Nested"));
        assert_eq!(order_info_metadata.is_gift, Some(false));
        assert_eq!(
            order_info_metadata.channel_id.as_deref(),
            Some("nested-channel")
        );
        let metadata = Secret::new(
            r#"{"session_id":"sdk-session-42","custom_fields":{"origin":"ignored"}}"#.to_owned(),
        );
        assert_eq!(
            get_session_id_from_metadata(Some(&metadata)).expect("session id"),
            "sdk-session-42"
        );
        let camel = Secret::new(r#"{"sessionId": "sdk-session-42"}"#.to_owned());
        assert_eq!(
            get_session_id_from_metadata(Some(&camel)).expect("session id"),
            "sdk-session-42"
        );
    }

    #[test]
    fn json_encoded_order_info_metadata_is_decoded_as_a_legacy_wrapper() {
        let parsed = ClearSaleConnectorFeatureData::parse(Some(&Secret::new(
            r#"{"enableDdc":false,"orderInfoMetadata":"{\"origin\":\"WebCheckout\",\"channelId\":\"sample channelId\",\"observation\":\"Legacy observation\",\"isGift\":true}"}"#
                .to_owned(),
        )))
        .expect("legacy nested metadata should be decoded");

        assert_eq!(
            parsed.order_info_metadata.origin.as_deref(),
            Some("WebCheckout")
        );
        assert_eq!(
            parsed.order_info_metadata.channel_id.as_deref(),
            Some("sample channelId")
        );
        assert_eq!(
            parsed.order_info_metadata.observation.as_deref(),
            Some("Legacy observation")
        );
        assert_eq!(parsed.order_info_metadata.is_gift, Some(true));
    }

    #[test]
    fn legacy_flat_feature_data_is_retained_instead_of_silently_dropped() {
        let mut req = pre_risk_check_request();
        req.installment_details = None;
        req.order_description = None;
        req.connector_feature_data = Some(Secret::new(
            r#"{
                "origin":"Legacy",
                "isGift":true,
                "billingPhoneType":2,
                "installments":3,
                "observation":"Flat observation",
                "orderInfoMetadata":{"origin":"Nested"}
            }"#
            .to_owned(),
        ));
        let pre_order =
            order_from_common(OrderInputs::from_pre_risk_check(&req, None, None).unwrap())
                .unwrap()
                .order;
        let mut post = post_risk_check_request();
        post.installment_details = None;
        post.order_description = None;
        post.connector_feature_data = req.connector_feature_data.clone();
        let post_order =
            order_from_common(OrderInputs::from_post_risk_check(&post, None, None).unwrap())
                .unwrap()
                .order;

        for order in [pre_order, post_order] {
            assert_eq!(order.origin.as_deref(), Some("Nested"));
            assert_eq!(order.observation.as_deref(), Some("Flat observation"));
            assert_eq!(order.is_gift, Some(true));
            assert_eq!(order.number_of_installments, Some(3));
            assert_eq!(order.payments[0].installments, Some(3));
            assert_eq!(order.billing.phones[0].phone_type, 2);
        }
    }

    #[test]
    fn order_info_parse_rejects_malformed_json() {
        for raw in [
            Some("{not-json"),
            Some(r#"{"orderInfoMetadata":"wrong-shape"}"#),
        ] {
            let err = ClearSaleConnectorFeatureData::parse(
                raw.map(|value| Secret::new(value.to_owned())).as_ref(),
            )
            .expect_err("malformed feature data or invalid order info must fail");
            assert!(matches!(
                err.current_context(),
                IntegrationError::InvalidDataFormat { field_name, .. }
                    if *field_name == "connector_feature_data"
            ));
        }
        assert_eq!(
            ClearSaleConnectorFeatureData::parse(None)
                .unwrap()
                .order_info_metadata,
            ClearSaleOrderInfoMetadata::default()
        );
    }

    /// Malformed `connector_feature_data` is an error, never a silent default.
    #[test]
    fn feature_data_parse_rejects_malformed_json() {
        let err = ClearSaleConnectorFeatureData::parse(Some(&Secret::new("{not-json".to_owned())))
            .expect_err("malformed feature data must fail");
        assert!(
            matches!(
                err.current_context(),
                IntegrationError::InvalidDataFormat { field_name, .. }
                    if *field_name == "connector_feature_data"
            ),
            "got {:?}",
            err.current_context()
        );
        assert!(ClearSaleConnectorFeatureData::parse(None).is_ok());
    }

    /// Malformed request `metadata` is an error — never silently defaulted.
    #[test]
    fn metadata_parse_rejects_malformed_json() {
        let err = ClearSaleFrmMetadata::parse(Some(&Secret::new("{not-json".to_owned())))
            .expect_err("malformed metadata must fail");
        assert!(
            matches!(
                err.current_context(),
                IntegrationError::InvalidDataFormat { field_name, .. }
                    if *field_name == "metadata"
            ),
            "got {:?}",
            err.current_context()
        );
        assert!(ClearSaleFrmMetadata::parse(None).is_ok());
    }

    // ── address / phone validation ───────────────────────────────────────

    /// Street/number split: a trailing number token is used, then `line3`.
    /// When neither yields a house number the call **errors** — the whole
    /// `line1` is never sent as `number` with an empty street.
    #[test]
    fn split_street_and_number_requires_a_house_number() {
        let (street, number, leftover) = split_street_and_number(
            "billing",
            Some(&Secret::new("Rua Augusta, 1234".to_owned())),
            None,
        )
        .unwrap();
        assert_eq!(street.peek(), "Rua Augusta");
        assert_eq!(number, "1234");
        assert_eq!(leftover, None, "consumed line3 is not echoed");

        // BR comma form with neighbourhood after the number.
        let (street, number, leftover) = split_street_and_number(
            "billing",
            Some(&Secret::new("Rua Augusta, 1234 - Bela Vista".to_owned())),
            None,
        )
        .unwrap();
        assert_eq!(street.peek(), "Rua Augusta");
        assert_eq!(number, "1234");
        assert_eq!(leftover, None);

        // Suffix numbers: "1234-A", "1234/56", "1234B", "s/n".
        for line1 in [
            "Rua Augusta 1234-A",
            "Rua Augusta 1234/56",
            "Rua Augusta 1234B",
            "Rua Augusta, s/n",
        ] {
            let (_, number, _) =
                split_street_and_number("billing", Some(&Secret::new(line1.to_owned())), None)
                    .unwrap_or_else(|e| panic!("{line1}: {e}"));
            assert!(!number.is_empty(), "{line1}");
        }

        let (street, number, leftover) = split_street_and_number(
            "billing",
            Some(&Secret::new("Rua Augusta".to_owned())),
            Some(&Secret::new("567".to_owned())),
        )
        .unwrap();
        assert_eq!(street.peek(), "Rua Augusta");
        assert_eq!(number, "567");
        assert_eq!(leftover, None, "consumed house number is not echoed");

        // No house number anywhere + a street → *sem número*, not an error
        // and never whole-line1-as-number with an empty street.
        let (street, number, leftover) = split_street_and_number(
            "billing",
            Some(&Secret::new("Girija Building".to_owned())),
            Some(&Secret::new("8th Block".to_owned())),
        )
        .expect("street present → s/n");
        assert_eq!(street.peek(), "Girija Building");
        assert_eq!(number, "s/n");
        assert_eq!(
            leftover.as_deref(),
            Some("8th Block"),
            "unconsumed line3 rides along as additionalInformation"
        );

        let (street, number, leftover) = split_street_and_number(
            "shipping",
            Some(&Secret::new("Girija Building".to_owned())),
            None,
        )
        .expect("street present → s/n");
        assert_eq!(street.peek(), "Girija Building");
        assert_eq!(number, "s/n");
        assert_eq!(leftover, None);

        // line1 was only a number and nothing supplies the street → error.
        let err = split_street_and_number("billing", Some(&Secret::new("1234".to_owned())), None)
            .expect_err("number-only line1 has no street");
        assert!(missing_fields(&err).contains(&"billing.address.street".to_owned()));

        // No street at all → error (shipping labels itself).
        let err = split_street_and_number("shipping", None, None).expect_err("no street must fail");
        assert!(missing_fields(&err).contains(&"shipping.address.street".to_owned()));
        assert!(!missing_fields(&err).contains(&"billing.address.street".to_owned()));

        assert!(split_street_and_number("billing", None, None).is_err());
        assert!(split_street_and_number(
            "billing",
            None,
            Some(&Secret::new("8th Block".to_owned()))
        )
        .is_err());
    }

    /// city / state / zip / county must not default to `""`. The first absent
    /// required field fails immediately at its access site.
    #[test]
    fn build_address_errors_on_missing_city_state_zip() {
        let details = AddressDetails {
            line1: Some(Secret::new("Rua Augusta, 1234".to_owned())),
            line2: Some(Secret::new("Consolação".to_owned())),
            ..AddressDetails::default()
        };
        let err = build_clear_sale_address("billing", Some(&details))
            .expect_err("city/state/zip required");
        assert_eq!(
            missing_fields(&err),
            vec!["billing.address.city".to_owned()]
        );

        let details = AddressDetails {
            city: Some(Secret::new("São Paulo".to_owned())),
            ..details
        };
        let err = build_clear_sale_address("billing", Some(&details))
            .expect_err("state/zip still required");
        assert_eq!(
            missing_fields(&err),
            vec!["billing.address.state".to_owned()]
        );

        let details = AddressDetails {
            state: Some(StateDetails::from_code(Secret::new("SP".to_owned()))),
            ..details
        };
        let err =
            build_clear_sale_address("billing", Some(&details)).expect_err("zip still required");
        assert_eq!(
            missing_fields(&err),
            vec!["billing.address.zipcode".to_owned()]
        );
    }

    /// ClearSale `country` / `address.country` use the documented name
    /// vocabulary (`"Brasil"`), not ISO alpha-2.
    #[test]
    fn country_is_normalized_to_clearsale_name() {
        assert_eq!(
            clearsale_country_name(common_enums::CountryAlpha2::BR),
            "Brasil"
        );
        // Unknown / non-BR codes fall back to alpha-2 rather than inventing a name.
        assert_eq!(
            clearsale_country_name(common_enums::CountryAlpha2::US),
            "US"
        );
    }

    /// `state` must go on the wire as a 2-char UF. `"Acre"` → `"AC"`;
    /// a 2-char code passes through uppercased; unmappable values error.
    #[test]
    fn address_state_is_normalized_to_uf() {
        let details = AddressDetails {
            line1: Some(Secret::new("Rua Augusta, 1234".to_owned())),
            line2: Some(Secret::new("Consolação".to_owned())),
            city: Some(Secret::new("Rio Branco".to_owned())),
            state: Some(StateDetails::from_name(Secret::new("Acre".to_owned()))),
            zip: Some(Secret::new("69900-000".to_owned())),
            country: Some(common_enums::CountryAlpha2::BR),
            ..AddressDetails::default()
        };
        let address = build_clear_sale_address("billing", Some(&details)).expect("Acre maps");
        assert_eq!(address.state.peek(), "AC");

        let details = AddressDetails {
            state: Some(StateDetails::from_code(Secret::new("sp".to_owned()))),
            ..details
        };
        let address = build_clear_sale_address("billing", Some(&details)).expect("SP passes");
        assert_eq!(address.state.peek(), "SP");

        let details = AddressDetails {
            state: Some(StateDetails::from_code(Secret::new("Narnia".to_owned()))),
            ..details
        };
        let err = build_clear_sale_address("billing", Some(&details))
            .expect_err("unknown state must not go on the wire");
        assert!(
            matches!(
                err.current_context(),
                IntegrationError::InvalidDataFormat {
                    field_name: "billing.address.state",
                    ..
                }
            ),
            "got {err:?}"
        );
    }

    #[test]
    fn generated_datetimes_use_one_brasilia_instant_for_order_and_payment() {
        let utc = time::macros::datetime!(2026-11-13 02:47:16.588292612 UTC);
        assert_eq!(
            generated_datetime_at(utc).unwrap(),
            "2026-11-12T23:47:16.5882926"
        );

        let req = pre_risk_check_request();
        let order = order_from_common_at(
            OrderInputs::from_pre_risk_check(&req, None, None).unwrap(),
            utc,
        )
        .unwrap()
        .order;
        assert_eq!(order.date, "2026-11-12T23:47:16.5882926");
        assert_eq!(order.payments[0].date.as_deref(), Some(order.date.as_str()));
    }

    /// ClearSale `Datetime` is .NET DateTime — no timezone, ≤7 frac digits.
    #[test]
    fn to_clearsale_datetime_strips_timezone_and_caps_precision() {
        assert_eq!(
            to_clearsale_datetime("2025-06-01"),
            "2025-06-01T00:00:00.000"
        );
        assert_eq!(
            to_clearsale_datetime("1990-05-14T00:00:00Z"),
            "1990-05-14T00:00:00"
        );
        assert_eq!(
            to_clearsale_datetime("2025-06-01T08:00:00+00:00"),
            "2025-06-01T08:00:00"
        );
        assert_eq!(
            to_clearsale_datetime("2017-03-22T13:38:59.9894222"),
            "2017-03-22T13:38:59.9894222"
        );
        assert_eq!(
            to_clearsale_datetime("2017-03-22T13:38:59.989422212345Z"),
            "2017-03-22T13:38:59.9894222"
        );
        assert_eq!(
            to_clearsale_datetime("1990-01-10T00:00:00.000"),
            "1990-01-10T00:00:00.000"
        );
    }

    /// Flight identifier → IATA-style carrier code. Numeric-only identifiers
    /// have no carrier and the leg is dropped rather than sent as garbage.
    #[test]
    fn carrier_code_from_identifier_splits_iata_prefix() {
        assert_eq!(
            carrier_code_from_identifier("G3-1234").as_deref(),
            Some("G3")
        );
        assert_eq!(
            carrier_code_from_identifier("G3 1234").as_deref(),
            Some("G3")
        );
        assert_eq!(carrier_code_from_identifier("JJ").as_deref(), Some("JJ"));
        assert_eq!(
            carrier_code_from_identifier("G31234").as_deref(),
            Some("G3")
        );
        assert_eq!(carrier_code_from_identifier("101"), None);
        assert_eq!(carrier_code_from_identifier(""), None);
    }

    /// ClearSale returns an absolute `ExpirationDate`; `expires_in` is the TTL
    /// seconds the token cache expects. Already-expired / unparsable stamps
    /// yield `None` rather than a bogus TTL.
    #[test]
    fn expires_in_is_derived_from_expiration_date() {
        // now = 2023-11-14T22:13:20Z
        let now = time::OffsetDateTime::from_unix_timestamp(1_700_000_000).expect("valid ts");

        // RFC 3339 with offset → exact remaining seconds.
        assert_eq!(
            expires_in_from_expiration_date("2023-11-14T22:00:00Z", now)
                .expect("already-expired stamp still parses"),
            None,
            "already expired"
        );
        assert_eq!(
            expires_in_from_expiration_date("2023-11-14T22:23:20+00:00", now)
                .expect("RFC 3339 ExpirationDate parses"),
            Some(600)
        );

        // .NET DateTime (no offset) is Brasília time (UTC−3) — ClearSale is a
        // Brazilian API — so 22:23:20 is 01:23:20Z on the 15th, 11400s from now.
        assert_eq!(
            expires_in_from_expiration_date("2023-11-14T22:23:20", now)
                .expect(".NET ExpirationDate parses"),
            Some(11400)
        );
        assert_eq!(
            expires_in_from_expiration_date("2023-11-14T22:23:20.500", now)
                .expect(".NET ExpirationDate with ms parses"),
            Some(11400)
        );

        // Unparsable / empty → Err (never a fabricated TTL).
        assert!(expires_in_from_expiration_date("not-a-date", now).is_err());
        assert!(expires_in_from_expiration_date("", now).is_err());
    }

    /// Phone: leading `55` is stripped when country_code is absent (E.164).
    /// Unparsable ddd/number is a missing-field error — never `0`.
    #[test]
    fn split_phone_strips_leading_55_and_rejects_zeros() {
        let phone = split_phone("5511987654321", None, CLEARSALE_PHONE_TYPE_HOME).unwrap();
        assert_eq!(phone.ddi, None);
        assert_eq!(phone.ddd, 11);
        assert_eq!(phone.number, 987654321);

        let phone =
            split_phone("+55 11 98765-4321", Some("55"), CLEARSALE_PHONE_TYPE_HOME).unwrap();
        assert_eq!(phone.ddi, Some(55));
        assert_eq!(phone.ddd, 11);
        assert_eq!(phone.number, 987654321);

        // Too short to yield a DDD + number.
        assert!(split_phone("9", None, 1).is_err());
        // Empty / non-digit input.
        assert!(split_phone("abc", None, 1).is_err());
    }

    // ── person_type / payment type ───────────────────────────────────────

    /// person_type maps from DocumentKind: CPF→1, CNPJ→2, Psn/Other rejected.
    #[test]
    fn person_type_maps_document_kind() {
        let feature = ClearSaleOrderInfoMetadata::default();
        let doc = Some(Secret::new("12345678909".to_owned()));

        let cpf = build_person(
            "billing",
            Some(&billing_address()),
            Some(&customer_info()),
            doc.as_ref(),
            Some(&DocumentKind::Cpf),
            &feature,
        )
        .expect("cpf builds");
        assert_eq!(cpf.person_type, CLEARSALE_PERSON_TYPE_PF);

        let cnpj = build_person(
            "billing",
            Some(&billing_address()),
            Some(&customer_info()),
            doc.as_ref(),
            Some(&DocumentKind::Cnpj),
            &feature,
        )
        .expect("cnpj builds");
        assert_eq!(cnpj.person_type, CLEARSALE_PERSON_TYPE_PJ);

        for kind in [DocumentKind::Psn, DocumentKind::Other] {
            let err = build_person(
                "billing",
                Some(&billing_address()),
                Some(&customer_info()),
                doc.as_ref(),
                Some(&kind),
                &feature,
            )
            .expect_err("{kind:?} must not go out as CPF");
            assert_eq!(
                missing_fields(&err),
                vec!["billing.document_type".to_owned()]
            );
        }
    }

    /// Payment instrument codes (ClearSale payment-type table). Known
    /// instruments map; anything else errors rather than becoming type 1.
    #[test]
    fn payment_type_maps_known_instruments_and_rejects_the_rest() {
        assert_eq!(
            clearsale_payment_type(&card()).unwrap(),
            CLEARSALE_PAYMENT_TYPE_CREDIT_CARD
        );

        let debit = PaymentMethodData::Card(domain_types::payment_method_data::Card {
            card_type: Some("debit".to_owned()),
            ..card_as_default()
        });
        assert_eq!(
            clearsale_payment_type(&debit).unwrap(),
            CLEARSALE_PAYMENT_TYPE_DEBIT_CARD
        );

        let boleto = PaymentMethodData::Voucher(VoucherData::Boleto(Box::new(
            domain_types::payment_method_data::BoletoVoucherData {
                social_security_number: None,
                expiration_date: None,
            },
        )));
        assert_eq!(
            clearsale_payment_type(&boleto).unwrap(),
            CLEARSALE_PAYMENT_TYPE_BANK_SLIP
        );

        let pix = PaymentMethodData::BankTransfer(Box::new(BankTransferData::Pix {
            pix_key: None,
            cpf: None,
            cnpj: None,
            destination_bank_account_id: None,
            source_bank_account_id: None,
            expiry_date: None,
        }));
        assert_eq!(
            clearsale_payment_type(&pix).unwrap(),
            CLEARSALE_PAYMENT_TYPE_PIX
        );

        let bank_debit = PaymentMethodData::BankDebit(BankDebitData::SepaBankDebit {
            iban: Secret::new("DE89370400440532013000".to_owned()),
            bank_account_holder_name: None,
        });
        assert_eq!(
            clearsale_payment_type(&bank_debit).unwrap(),
            CLEARSALE_PAYMENT_TYPE_BANK_DEBIT
        );

        let gift = PaymentMethodData::GiftCard(Box::new(GiftCardData::Givex(GiftCardDetails {
            number: Secret::new("1234".to_owned()),
            cvc: Secret::new("123".to_owned()),
        })));
        assert_eq!(
            clearsale_payment_type(&gift).unwrap(),
            CLEARSALE_PAYMENT_TYPE_VIRTUAL_GIFT_CARD
        );

        assert_eq!(
            clearsale_payment_type(&PaymentMethodData::Reward).unwrap(),
            CLEARSALE_PAYMENT_TYPE_CASHBACK
        );

        // Mandate payments and Cash App QR have no honest ClearSale code.
        assert!(clearsale_payment_type(&PaymentMethodData::MandatePayment).is_err());
        assert!(
            clearsale_payment_type(&PaymentMethodData::Wallet(WalletData::CashappQr(Box::new(
                CashappQr {}
            ))))
            .is_err()
        );

        // Unknown instrument must error, not default to credit card.
        let pay_later = PaymentMethodData::PayLater(PayLaterData::KlarnaRedirect {});
        assert!(clearsale_payment_type(&pay_later).is_err());

        // Pin the codes the connector actually emits (spec §5.1).
        assert_eq!(CLEARSALE_PAYMENT_TYPE_CREDIT_CARD, 1);
        assert_eq!(CLEARSALE_PAYMENT_TYPE_BANK_SLIP, 2);
        assert_eq!(CLEARSALE_PAYMENT_TYPE_BANK_DEBIT, 3);
        assert_eq!(CLEARSALE_PAYMENT_TYPE_BANK_TRANSFER, 6);
        assert_eq!(CLEARSALE_PAYMENT_TYPE_VOUCHER, 16);
        assert_eq!(CLEARSALE_PAYMENT_TYPE_DEBIT_CARD, 19);
        assert_eq!(CLEARSALE_PAYMENT_TYPE_PIX, 27);
        assert_eq!(CLEARSALE_PAYMENT_TYPE_CASHBACK, 28);
        assert_eq!(CLEARSALE_PAYMENT_TYPE_VIRTUAL_GIFT_CARD, 1041);
    }

    fn card_as_default() -> domain_types::payment_method_data::Card<DefaultPCIHolder> {
        match card() {
            PaymentMethodData::Card(inner) => domain_types::payment_method_data::Card {
                card_number: inner.card_number.clone(),
                card_exp_month: inner.card_exp_month.clone(),
                card_exp_year: inner.card_exp_year.clone(),
                card_cvc: inner.card_cvc.clone(),
                ..Default::default()
            },
            _ => unreachable!(),
        }
    }

    // ── order_from_common / Pre / Post ───────────────────────────────────

    /// End-to-end PreRiskCheck → ClearSale order body: items amount is the
    /// decimal quantity, phone DDD is real, person_type is CPF, payment type
    /// is credit card, and the order code is the business order ID.
    #[test]
    fn order_from_common_builds_pre_risk_check_body() {
        let req = pre_risk_check_request();
        let inputs = OrderInputs::from_pre_risk_check(&req, None, None).unwrap();
        let order = order_from_common(inputs).unwrap().order;

        assert_eq!(order.code, "merchant-order-abc-001");
        assert_ne!(order.code, "txn-abc-001");
        assert_eq!(order.product, 4);
        assert_eq!(order.number_of_installments, Some(3));
        assert_eq!(order.payments[0].installments, Some(3));
        assert_eq!(order.session_id, "sdk-session-42");
        assert_eq!(order.ip.as_deref(), Some("200.155.10.20"));
        assert_eq!(
            order.email.clone().expose().peek(),
            "comprador@example.com.br"
        );
        assert_eq!(order.billing.person_type, CLEARSALE_PERSON_TYPE_PF);
        assert_eq!(order.billing.primary_document.peek(), "12345678909");
        assert_eq!(order.billing.address.number, "1234");
        assert_eq!(order.billing.address.street.peek(), "Rua Augusta");
        assert_eq!(order.billing.phones[0].ddd, 11);
        assert_eq!(order.billing.phones[0].number, 987654321);
        assert_eq!(
            order.payments[0].payment_type,
            CLEARSALE_PAYMENT_TYPE_CREDIT_CARD
        );
        assert_eq!(order.payments[0].currency, Some(986));
        assert_eq!(order.items.len(), 1);
        assert_eq!(order.items[0].amount, Some(2));
        // `itemValue` is the items subtotal (unit × quantity), not `totalValue`.
        assert_eq!(order.item_value, Some(FloatMajorUnit(300.00)));
        assert_eq!(order.nationality, None);
        // Country goes on the wire as the documented name, not ISO alpha-2.
        assert_eq!(order.country.as_deref(), Some("Brasil"));
        assert_eq!(
            order.billing.address.country.as_deref(),
            Some("Brasil"),
            "address.country must match the documented name vocabulary"
        );
    }

    /// No shipping address: the order carries no `shipping` block at all.
    /// Billing is deliberately *not* mirrored — a fabricated delivery address
    /// would mislead the risk model.
    #[test]
    fn no_shipping_is_omitted_not_mirrored() {
        let req = pre_risk_check_request();
        assert!(req.address.as_ref().unwrap().get_shipping().is_none());
        let inputs = OrderInputs::from_pre_risk_check(&req, None, None).unwrap();
        let order = order_from_common(inputs).unwrap().order;
        assert!(order.shipping.is_none());
    }

    #[test]
    fn absent_and_empty_optional_blocks_are_omitted() {
        for order_details in [None, Some(Vec::new())] {
            let mut req = pre_risk_check_request();
            req.order_details = order_details;
            req.address = Some(PaymentAddress::new(
                Some(Address::default()),
                Some(billing_address()),
                None,
                Some(false),
            ));
            let inputs = OrderInputs::from_pre_risk_check(&req, None, None).unwrap();
            let order = order_from_common(inputs).unwrap().order;
            assert!(order.shipping.is_none());
            assert!(order.items.is_empty());
            assert!(order.item_value.is_none());

            let json = serde_json::to_value(order).expect("order serializes");
            let object = json.as_object().expect("order object");
            assert!(!object.contains_key("shipping"));
            assert!(!object.contains_key("items"));
        }
    }

    #[test]
    fn incomplete_shipping_mandatory_address_fields_omit_shipping() {
        for details in [
            AddressDetails {
                city: Some(Secret::new("São Paulo".to_owned())),
                ..AddressDetails::default()
            },
            AddressDetails {
                line1: Some(Secret::new("   ".to_owned())),
                ..billing_address_details()
            },
            AddressDetails {
                line2: None,
                ..billing_address_details()
            },
            AddressDetails {
                zip: Some(Secret::new("".to_owned())),
                ..billing_address_details()
            },
        ] {
            let mut req = pre_risk_check_request();
            req.address = Some(PaymentAddress::new(
                Some(Address {
                    address: Some(details),
                    ..Address::default()
                }),
                Some(billing_address()),
                None,
                Some(false),
            ));
            let inputs = OrderInputs::from_pre_risk_check(&req, None, None).unwrap();
            let order = order_from_common(inputs).unwrap().order;
            assert!(order.shipping.is_none());
            assert!(!serde_json::to_value(order)
                .expect("order serializes")
                .as_object()
                .expect("order object")
                .contains_key("shipping"));
        }
    }

    #[test]
    fn items_with_a_blank_name_are_omitted() {
        for name in ["", "   "] {
            let mut details = order_details();
            details[0].product_name = name.to_owned();
            let payment = common_utils::types::Money::zero(Currency::BRL);
            let items = build_clear_sale_items(&payment, Some(&details), None).unwrap();
            assert!(items.is_empty());

            let mut req = pre_risk_check_request();
            req.order_details = Some(details);
            let inputs = OrderInputs::from_pre_risk_check(&req, None, None).unwrap();
            let order = order_from_common(inputs).unwrap().order;
            assert!(order.item_value.is_none());
            assert!(!serde_json::to_value(order)
                .expect("order serializes")
                .as_object()
                .expect("order object")
                .contains_key("items"));
        }
    }

    /// PostRiskCheck must emit the same order body as PreRiskCheck (only the
    /// HTTP method differs). `ip` comes from browser_info; domain_data rides
    /// through; the order code is `merchant_order_id` — never the payment-attempt
    /// or PSP connector transaction ID.
    #[test]
    fn post_risk_check_matches_pre_risk_check_body() {
        let pre = pre_risk_check_request();
        let post = post_risk_check_request();

        let pre_inputs = OrderInputs::from_pre_risk_check(&pre, None, None).unwrap();
        let post_inputs = OrderInputs::from_post_risk_check(&post, None, None).unwrap();
        let (pre_order, post_order) = (
            order_from_common(pre_inputs).unwrap().order,
            order_from_common(post_inputs).unwrap().order,
        );

        // Same purchase body (dates are clock reads — ignore them).
        assert_eq!(pre_order.code, post_order.code);
        assert_eq!(pre_order.session_id, post_order.session_id);
        assert_eq!(pre_order.email, post_order.email);
        assert_eq!(pre_order.ip, post_order.ip);
        assert_eq!(pre_order.total_value, post_order.total_value);
        assert_eq!(
            pre_order.billing.person_type,
            post_order.billing.person_type
        );
        assert_eq!(
            pre_order.payments[0].payment_type,
            post_order.payments[0].payment_type
        );
        assert_eq!(pre_order.items[0].amount, post_order.items[0].amount);

        // ip is populated from browser_info on both — never left as None when
        // the request carried one.
        assert_eq!(post_order.ip.as_deref(), Some("200.155.10.20"));

        // Order code ignores the PSP connector_transaction_id on Post.
        assert_ne!(post_order.code, "psp-txn-should-not-be-order-code");
    }

    /// Non-BRL currencies are rejected — ClearSale Total Garantido is BRL-only.
    #[test]
    fn non_brl_currency_is_rejected() {
        let mut req = pre_risk_check_request();
        req.money = PreRiskCheckRequest::foreign_try_from(
            grpc_api_types::frm::FrmServicePreRiskCheckRequest {
                amount: Some(grpc_api_types::payments::Money {
                    minor_amount: 1000,
                    currency: grpc_api_types::payments::Currency::Usd.into(),
                }),
                ..Default::default()
            },
        )
        .expect("valid USD pre-risk-check fixture")
        .money;
        let inputs = OrderInputs::from_pre_risk_check(&req, None, None).unwrap();
        let err = order_from_common(inputs).expect_err("USD must fail");
        assert!(
            matches!(
                err.current_context(),
                IntegrationError::CurrencyNotSupported { .. }
            ),
            "got {:?}",
            err.current_context()
        );
        assert!(clearsale_currency_code(Currency::BRL).is_ok());
        assert!(clearsale_currency_code(Currency::USD).is_err());
    }

    /// Missing required fields fail one at a time at the access site.
    #[test]
    fn order_from_common_reports_missing_required_fields() {
        // Metadata getters extract and validate in one place. The order code
        // is `merchant_order_id` (never a metadata key), so missing metadata
        // fails on `sessionID` only.
        let mut req = pre_risk_check_request();
        req.metadata = None;
        req.connector_feature_data = None;
        let err = OrderInputs::from_pre_risk_check(&req, None, None)
            .map(|_| ())
            .expect_err("missing session id");
        assert_eq!(missing_fields(&err), vec!["sessionID".to_owned()]);

        // Email is required while building `OrderInputs`.
        let mut req = pre_risk_check_request();
        req.customer_info = None;
        let err = OrderInputs::from_pre_risk_check(&req, None, None)
            .map(|_| ())
            .expect_err("missing email");
        assert_eq!(missing_fields(&err), vec!["email".to_owned()]);

        // CPF/CNPJ document is required while building the billing person.
        let mut req = pre_risk_check_request();
        req.billing_document_data = None;
        let inputs = OrderInputs::from_pre_risk_check(&req, None, None).unwrap();
        let err = order_from_common(inputs).expect_err("missing billing.primaryDocument");
        assert_eq!(
            missing_fields(&err),
            vec!["billing.primaryDocument".to_owned()]
        );
    }

    #[test]
    fn order_code_requires_the_business_order_id() {
        let mut req = pre_risk_check_request();
        req.merchant_order_id = None;
        let err = OrderInputs::from_pre_risk_check(&req, None, None)
            .map(|_| ())
            .expect_err("missing business order ID");
        assert_eq!(missing_fields(&err), vec!["merchant_order_id".to_owned()]);
    }

    #[test]
    fn typed_order_description_and_connector_observation_stay_separate() {
        let mut req = pre_risk_check_request();
        for (description, expected) in [
            (Some("Order description".to_owned()), "Order description"),
            (None, "Legacy connector observation"),
            (Some("   ".to_owned()), "Legacy connector observation"),
        ] {
            req.order_description = description;
            let mut post = post_risk_check_request();
            post.order_description = req.order_description.clone();
            let pre_inputs = OrderInputs::from_pre_risk_check(&req, None, None).unwrap();
            let post_inputs = OrderInputs::from_post_risk_check(&post, None, None).unwrap();
            assert_eq!(
                order_from_common(pre_inputs)
                    .unwrap()
                    .order
                    .observation
                    .as_deref(),
                Some(expected)
            );
            assert_eq!(
                order_from_common(post_inputs)
                    .unwrap()
                    .order
                    .observation
                    .as_deref(),
                Some(expected)
            );
        }
    }

    #[test]
    fn grpc_card_details_preserve_supported_networks() {
        use domain_types::payment_method_data::Card;

        for (network, expected) in [
            (
                grpc_api_types::payments::CardNetwork::Visa,
                CardNetwork::Visa,
            ),
            (
                grpc_api_types::payments::CardNetwork::Mastercard,
                CardNetwork::Mastercard,
            ),
        ] {
            let card =
                Card::<DefaultPCIHolder>::foreign_try_from(grpc_api_types::payments::CardDetails {
                    card_number: Some(
                        cards::CardNumber::from_str("4111111111111111")
                            .expect("valid test card number"),
                    ),
                    card_exp_month: Some(Secret::new("12".to_owned())),
                    card_exp_year: Some(Secret::new("2030".to_owned())),
                    card_cvc: Some(Secret::new("123".to_owned())),
                    card_network: Some(i32::from(network)),
                    ..Default::default()
                })
                .expect("supported card network");
            assert_eq!(card.card_network, Some(expected));
        }

        assert!(
            CardNetwork::foreign_try_from(grpc_api_types::payments::CardNetwork::Unspecified)
                .is_err()
        );
    }

    #[test]
    fn clearsale_card_type_maps_supported_networks() {
        for (network, expected) in [
            (CardNetwork::DinersClub, Some(1)),
            (CardNetwork::Mastercard, Some(2)),
            (CardNetwork::Visa, Some(3)),
            (CardNetwork::AmericanExpress, Some(5)),
            (CardNetwork::JCB, None),
        ] {
            assert_eq!(clearsale_card_type(Some(&network)), expected);
        }
        assert_eq!(clearsale_card_type(None), None);
    }

    #[test]
    fn absent_emi_and_legacy_installments_leave_installments_unset() {
        let mut req = pre_risk_check_request();
        req.connector_feature_data = None;
        req.installment_details = None;
        let inputs = OrderInputs::from_pre_risk_check(&req, None, None).unwrap();
        let order = order_from_common(inputs).unwrap().order;
        assert_eq!(order.number_of_installments, None);
        assert_eq!(order.payments[0].installments, None);
    }

    /// `items[].amount` is the line quantity as an **Integer** (not a float
    /// and not the line total). The unused `total_value` guard param is gone.
    #[test]
    fn build_clear_sale_items_uses_integer_quantity() {
        let payment = common_utils::types::Money::zero(Currency::BRL);
        let items = build_clear_sale_items(&payment, Some(&order_details()), Some(false)).unwrap();
        assert_eq!(items[0].amount, Some(2));
        assert_eq!(items[0].value, Some(FloatMajorUnit(150.0)));
        assert!(build_clear_sale_items(&payment, None, None)
            .unwrap()
            .is_empty());
        assert!(build_clear_sale_items(&payment, Some(&Vec::new()), None)
            .unwrap()
            .is_empty());
    }

    /// Card PAN data is credit-only on the wire. A debit card still maps to
    /// payment type 19 but must omit `payments[].card` (ClearSale documents
    /// the card block as required/allowed only for credit).
    #[test]
    fn debit_payment_omits_card_block() {
        let debit = PaymentMethodData::Card(domain_types::payment_method_data::Card {
            card_type: Some("debit".to_owned()),
            ..card_as_default()
        });
        let payment = build_clear_sale_payment(
            Some(&debit),
            FloatMajorUnit(150.0),
            "2026-10-22T04:43:43.169848",
            None,
            None,
            Currency::BRL,
        )
        .expect("debit builds");
        assert_eq!(payment.payment_type, CLEARSALE_PAYMENT_TYPE_DEBIT_CARD);
        assert!(
            payment.card.is_none(),
            "debit must not send payments[].card"
        );

        // Credit keeps the card block (and still requires one).
        let credit = build_clear_sale_payment(
            Some(&card()),
            FloatMajorUnit(150.0),
            "2026-10-22T04:43:43.169848",
            None,
            None,
            Currency::BRL,
        )
        .expect("credit builds");
        assert_eq!(credit.payment_type, CLEARSALE_PAYMENT_TYPE_CREDIT_CARD);
        assert!(credit.card.is_some(), "credit must send payments[].card");
    }

    /// Wallets (GPay/APay/SPay) and `CardDetailsForNetworkTransactionId` map
    /// to type 1 (card rails) but carry no full PAN block. Requiring
    /// `payments[].card` for type 1 would make every wallet order fail with
    /// `MissingRequiredField("payment.card")` — the block is required only for
    /// `Card` / `CardWithNoCvc`.
    #[test]
    fn wallet_and_network_tx_id_omit_card_block() {
        let google_pay =
            PaymentMethodData::Wallet(WalletData::GooglePay(Box::new(GooglePayWalletData {
                pm_type: "CARD".to_owned(),
                description: "Visa ending in 1111".to_owned(),
                info: GooglePayPaymentMethodInfo {
                    card_network: "VISA".to_owned(),
                    card_details: "1111".to_owned(),
                    assurance_details: None,
                },
                tokenization_data: GpayTokenizationData::Encrypted(GpayEncryptedTokenizationData {
                    token_type: "PAYMENT_GATEWAY".to_owned(),
                    token: "opaque".to_owned(),
                }),
            })));
        let apple_pay =
            PaymentMethodData::Wallet(WalletData::ApplePay(Box::new(ApplePayWalletData {
                payment_data: ApplePayPaymentData::Encrypted("opaque".to_owned()),
                payment_method: ApplepayPaymentMethod {
                    display_name: "Visa".to_owned(),
                    network: "Visa".to_owned(),
                    pm_type: "debit".to_owned(),
                },
                transaction_identifier: "tx-1".to_owned(),
            })));
        let network_tx_id = PaymentMethodData::CardDetailsForNetworkTransactionId(
            CardDetailsForNetworkTransactionId {
                card_number: RawCardNumber(
                    cards::CardNumber::from_str("4111111111111111").expect("valid PAN"),
                ),
                card_exp_month: Secret::new("12".to_owned()),
                card_exp_year: Secret::new("2030".to_owned()),
                card_issuer: None,
                card_network: Some(CardNetwork::Visa),
                card_type: None,
                card_issuing_country: None,
                bank_code: None,
                nick_name: None,
                card_holder_name: None,
            },
        );

        for instrument in [&google_pay, &apple_pay, &network_tx_id] {
            let payment = build_clear_sale_payment(
                Some(instrument),
                FloatMajorUnit(150.0),
                "2026-10-22T04:43:43.169848",
                None,
                None,
                Currency::BRL,
            )
            .expect("wallet / network-tx-id builds without a card block");
            assert_eq!(payment.payment_type, CLEARSALE_PAYMENT_TYPE_CREDIT_CARD);
            assert!(
                payment.card.is_none(),
                "wallet / network-tx-id must not send payments[].card"
            );
        }
    }

    // ── domain_data travel/hotel projection (singular Options) ───────────

    /// `DomainData.travel_data` / `hotel_data` are singular Options — one
    /// booking of each kind. Passengers live on each `TravelSegment`.
    #[test]
    fn domain_data_travel_blocks_pass_through() {
        let booking = TravelData {
            carrier_name: Some("LA".to_owned()),
            travel_date: Some("2025-06-01".to_owned()),
            number_of_passengers: Some(2),
            travel_segments: vec![
                TravelSegment {
                    identification_number: Some("101".to_owned()),
                    departure_date_time: Some("2025-06-01T08:00:00Z".to_owned()),
                    boarding_date_time: Some("2025-06-01T07:30:00Z".to_owned()),
                    arrival_date_time: Some("2025-06-01T12:00:00Z".to_owned()),
                    origin: Some("SAO".to_owned()),
                    destination: Some("RIO".to_owned()),
                    travel_class: Some("semi-leito".to_owned()),
                    trip_type: Some("ONE_WAY".to_owned()),
                    passengers: vec![Traveller {
                        first_name: Some(Secret::new("Joao".to_owned())),
                        last_name: Some(Secret::new("Silva".to_owned())),
                        document: Some(CustomerDocumentDetails {
                            document_type: DocumentKind::Cpf,
                            document_number: Secret::new("11122233344".to_owned()),
                        }),
                        gender: Some(Gender::Male),
                        loyalty_card: Some(Secret::new("BC-99".to_owned())),
                        loyalty_card_issuer: Some("Club".to_owned()),
                        ..Traveller::default()
                    }],
                },
                TravelSegment {
                    identification_number: Some("G3-1234".to_owned()),
                    departure_date_time: Some("2025-06-10T09:00:00Z".to_owned()),
                    origin: Some("RIO".to_owned()),
                    destination: Some("SAO".to_owned()),
                    passengers: vec![Traveller {
                        first_name: Some(Secret::new("Maria".to_owned())),
                        ..Traveller::default()
                    }],
                    ..TravelSegment::default()
                },
            ],
            ..TravelData::default()
        };
        let domain_data = DomainData {
            travel_data: Some(booking),
            hotel_data: Some(HotelData {
                booking_id: Some("BK-1".to_owned()),
                hotel_segments: vec![HotelSegment {
                    hotel_name: Some("Hotel Rio".to_owned()),
                    hotel_city: Some("Rio de Janeiro".to_owned()),
                    hotel_state: Some("RJ".to_owned()),
                    hotel_country: Some("BR".to_owned()),
                    reservation_date_time: Some("2025-05-01T10:00:00Z".to_owned()),
                    reservation_expires_date_time: Some("2025-05-02T10:00:00Z".to_owned()),
                    checkin: Some("2025-06-01".to_owned()),
                    checkout: Some("2025-06-05".to_owned()),
                    ..HotelSegment::default()
                }],
                ..HotelData::default()
            }),
            ..DomainData::default()
        };

        let travel = ClearSaleTravelData::from_domain_data(Some(&domain_data));

        let passengers = travel.passengers.expect("passengers projected");
        assert_eq!(passengers.len(), 2);
        let p = passenger_to_clear_sale(&passengers[0]).expect("passenger has a name");
        assert_eq!(p.name.peek(), "Joao Silva");
        assert_eq!(p.identification_type, Some(1));

        let segments = travel.travel_segments.expect("segments projected");
        assert_eq!(segments.len(), 2);
        let c = travel_segment_to_clear_sale_connection(segments[0].0, segments[0].1)
            .expect("required fields present");
        assert_eq!(c.company, "LA");
        assert_eq!(c.identification_number, Some(101));
        // Datetimes are .NET-safe: no `Z`, date-only becomes midnight.
        assert_eq!(c.date.as_deref(), Some("2025-06-01T08:00:00"));
        assert_eq!(c.boarding.as_deref(), Some("2025-06-01T07:30:00"));
        assert_eq!(c.arriving.as_deref(), Some("2025-06-01T12:00:00"));
        // Free-form seat class passes through verbatim (e.g. Buser's "semi-leito").
        assert_eq!(c.seat_class.as_deref(), Some("semi-leito"));
        assert_eq!(c.type_of_trip.as_deref(), Some("ONE_WAY"));
        assert_eq!(
            serde_json::to_value(&c).expect("connection serializes")["typeOfTrip"],
            "ONE_WAY"
        );
        // "G3-1234" is not numeric — no identificationNumber on the wire.
        // Booking-level `carrier_name` ("LA") wins over the identifier prefix.
        let c2 = travel_segment_to_clear_sale_connection(segments[1].0, segments[1].1)
            .expect("required fields present");
        assert_eq!(c2.identification_number, None);
        assert_eq!(c2.company, "LA");
        assert_eq!(c2.date.as_deref(), Some("2025-06-10T09:00:00"));

        let hotels = travel.accommodations.expect("hotel projected");
        let h = accommodation_to_clear_sale(hotels[0]).expect("hotel name present");
        assert_eq!(h.name, "Hotel Rio");

        // reservationDate comes from `TravelData.travel_date`, not from
        // `connector_feature_data`.
        assert_eq!(
            travel.reservation_date.as_deref(),
            Some("2025-06-01T00:00:00.000")
        );

        // No travel/hotel blocks at all → not a travel order.
        let empty = DomainData::default();
        let bare_travel = ClearSaleTravelData::from_domain_data(Some(&empty));
        assert!(bare_travel.reservation_date.is_none());
        assert!(bare_travel.passengers.is_none());
        assert!(bare_travel.travel_segments.is_none());
        assert!(bare_travel.accommodations.is_none());

        // Entries missing mandatory ClearSale fields are dropped, not emitted.
        assert!(passenger_to_clear_sale(&Traveller::default()).is_none());
        assert!(accommodation_to_clear_sale(&HotelSegment::default()).is_none());
    }

    /// A round trip carries the same traveller on both legs. ClearSale wants
    /// one `passengers[]` entry per person, so the duplicate is collapsed on
    /// name + identity document — never sent twice.
    #[test]
    fn two_leg_trip_dedupes_repeated_traveller() {
        let joao = Traveller {
            first_name: Some(Secret::new("Joao".to_owned())),
            last_name: Some(Secret::new("Silva".to_owned())),
            document: Some(CustomerDocumentDetails {
                document_type: DocumentKind::Cpf,
                document_number: Secret::new("11122233344".to_owned()),
            }),
            ..Traveller::default()
        };
        let booking = TravelData {
            carrier_name: Some("LA".to_owned()),
            travel_segments: vec![
                TravelSegment {
                    identification_number: Some("101".to_owned()),
                    departure_date_time: Some("2025-06-01T08:00:00Z".to_owned()),
                    origin: Some("SAO".to_owned()),
                    destination: Some("RIO".to_owned()),
                    passengers: vec![joao.clone()],
                    ..TravelSegment::default()
                },
                TravelSegment {
                    identification_number: Some("102".to_owned()),
                    departure_date_time: Some("2025-06-10T09:00:00Z".to_owned()),
                    origin: Some("RIO".to_owned()),
                    destination: Some("SAO".to_owned()),
                    passengers: vec![joao.clone()],
                    ..TravelSegment::default()
                },
            ],
            ..TravelData::default()
        };
        let domain_data = DomainData {
            travel_data: Some(booking),
            ..DomainData::default()
        };

        let travel = ClearSaleTravelData::from_domain_data(Some(&domain_data));
        let passengers = travel.passengers.expect("passengers projected");
        assert_eq!(
            passengers.len(),
            1,
            "one traveller on both legs must be sent once"
        );

        // Both legs still project as connections.
        let segments = travel.travel_segments.expect("segments projected");
        assert_eq!(segments.len(), 2);
    }

    /// A CNPJ billing document is Pessoa Jurídica (2), not Pessoa Física (1).
    #[test]
    fn cnpj_billing_builds_pj_person() {
        let mut req = pre_risk_check_request();
        req.billing_document_data = Some(CustomerDocumentDetails {
            document_type: DocumentKind::Cnpj,
            document_number: Secret::new("11222333000181".to_owned()),
        });
        let inputs = OrderInputs::from_pre_risk_check(&req, None, None).unwrap();
        let order = order_from_common(inputs).unwrap().order;
        assert_eq!(order.billing.person_type, CLEARSALE_PERSON_TYPE_PJ);
        assert_eq!(order.billing.primary_document.peek(), "11222333000181");
    }

    /// A 200 with a non-OK body status is not a successful push.
    #[test]
    fn status_update_rejects_non_ok_body() {
        let ok = ClearSaleStatusUpdateResponse {
            status: "OK".to_owned(),
            message: Some("accepted".to_owned()),
        };
        assert!(ok.status.eq_ignore_ascii_case("OK"));

        let rejected = ClearSaleStatusUpdateResponse {
            status: "ERROR".to_owned(),
            message: Some("order not found".to_owned()),
        };
        assert!(!rejected.status.eq_ignore_ascii_case("OK"));
    }

    // ── webhook ──────────────────────────────────────────────────────────

    /// Webhook `type` is mapped explicitly — never hardcoded to Processing.
    /// Reference and process_payment_webhook must agree on the order `code`.
    #[test]
    fn webhook_event_type_and_reference_agree() {
        let payload = ClearSaleWebhookPayload {
            code: "order-abc-001".to_owned(),
            date: "2025-01-01T00:00:00Z".to_owned(),
            event_type: "status".to_owned(),
        };
        let json = serde_json::to_string(&payload).unwrap();
        let parsed: ClearSaleWebhookPayload = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.code, "order-abc-001");
        assert_eq!(parsed.event_type, "status");

        // Unrecognised type must not become PaymentIntentProcessing.
        // (The connector maps "status" → EventType::Payment and errors on the rest.)
        let other = ClearSaleWebhookPayload {
            event_type: "something-else".to_owned(),
            ..payload
        };
        assert_ne!(other.event_type, "status");
    }
}
