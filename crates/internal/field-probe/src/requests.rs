// ── Probe request builders ────────────────────────────────────────────────────
//
// RULE: Base requests must contain ONLY fields that are universal to ALL
// connectors for that flow — i.e. fields that every connector implementation
// requires to even attempt building a request (e.g. connector_transaction_id,
// amount). Do NOT add fields that only some connectors need.
//
// Fields that are required by a subset of connectors belong in patching.rs /
// patch-config.toml, where they are injected lazily only when the connector's
// transformer reports them as missing.  Adding connector-specific fields to the
// base request silently hides which fields connectors actually require and
// pollutes the required_fields list for connectors that don't need them.
//
// Good example: `connector_transaction_id` — every capture/void/refund/get
//   connector needs this.
// Bad example:  `encoded_data` — only Adyen's PSync transformer reads this;
//   it belongs in patch_get_request or patch-config.toml [get] section.
// ─────────────────────────────────────────────────────────────────────────────

use cards::CardNumber;
use grpc_api_types::payments::{
    self as proto, mandate_reference::MandateIdType, payment_method::PaymentMethod as PmVariant,
    AcceptanceType, Address, AuthenticationType, CaptureMethod, CardDetails,
    ConnectorMandateReferenceId, CustomerAcceptance, CustomerServiceCreateRequest,
    DisputeServiceAcceptRequest, DisputeServiceDefendRequest, DisputeServiceSubmitEvidenceRequest,
    EvidenceDocument, EvidenceType, MandateReference,
    MerchantAuthenticationServiceCreateClientAuthenticationTokenRequest,
    MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest,
    MerchantAuthenticationServiceCreateServerSessionAuthenticationTokenRequest, PaymentAddress,
    PaymentClientAuthenticationContext, PaymentMethod,
    PaymentMethodAuthenticationServiceAuthenticateRequest,
    PaymentMethodAuthenticationServicePostAuthenticateRequest,
    PaymentMethodAuthenticationServicePreAuthenticateRequest, PaymentMethodServiceTokenizeRequest,
    PaymentServiceAuthorizeRequest, PaymentServiceCaptureRequest, PaymentServiceCreateOrderRequest,
    PaymentServiceGetRequest, PaymentServiceProxyAuthorizeRequest,
    PaymentServiceProxySetupRecurringRequest, PaymentServiceRefundRequest,
    PaymentServiceReverseRequest, PaymentServiceSetupRecurringRequest,
    PaymentServiceTokenAuthorizeRequest, PaymentServiceTokenSetupRecurringRequest,
    PaymentServiceVoidRequest, RecurringPaymentServiceChargeRequest,
};
use hyperswitch_masking::Secret;
use std::str::FromStr;

use crate::sample_data::{card_payment_method, usd_money};

pub(crate) fn base_authorize_request_with_meta(
    pm: PaymentMethod,
    connector_meta: Option<String>,
) -> PaymentServiceAuthorizeRequest {
    // Base request with fields that are commonly required by most connectors.
    // `address` is included as an empty wrapper because the domain-layer checks
    // req.address.is_some() before transformers run.
    // Leaf sub-fields are NOT pre-populated so the probe can discover exactly
    // which ones each connector truly needs.
    PaymentServiceAuthorizeRequest {
        amount: Some(usd_money(1000)),
        payment_method: Some(pm),
        capture_method: Some(CaptureMethod::Automatic as i32),
        auth_type: AuthenticationType::NoThreeDs as i32,
        merchant_transaction_id: Some("probe_txn_001".to_string()),
        connector_feature_data: connector_meta.map(Secret::new),
        return_url: Some("https://example.com/return".to_string()),
        address: Some(PaymentAddress {
            billing_address: Some(Address::default()),
            shipping_address: None,
        }),
        ..Default::default()
    }
}

/// Build an authorize request with OAuth state (access token) for OAuth connectors.
pub(crate) fn base_authorize_request_with_state(
    pm: PaymentMethod,
    connector_meta: Option<String>,
    state: proto::ConnectorState,
) -> PaymentServiceAuthorizeRequest {
    PaymentServiceAuthorizeRequest {
        amount: Some(usd_money(1000)),
        payment_method: Some(pm),
        capture_method: Some(CaptureMethod::Automatic as i32),
        auth_type: AuthenticationType::NoThreeDs as i32,
        merchant_transaction_id: Some("probe_txn_001".to_string()),
        connector_feature_data: connector_meta.map(Secret::new),
        return_url: Some("https://example.com/return".to_string()),
        address: Some(PaymentAddress {
            billing_address: Some(Address::default()),
            shipping_address: None,
        }),
        state: Some(state),
        ..Default::default()
    }
}

pub(crate) fn base_capture_request() -> PaymentServiceCaptureRequest {
    PaymentServiceCaptureRequest {
        connector_transaction_id: "probe_connector_txn_001".to_string(),
        amount_to_capture: Some(usd_money(1000)),
        merchant_capture_id: Some("probe_capture_001".to_string()),
        ..Default::default()
    }
}

pub(crate) fn base_refund_request() -> PaymentServiceRefundRequest {
    PaymentServiceRefundRequest {
        connector_transaction_id: "probe_connector_txn_001".to_string(),
        payment_amount: 1000,
        refund_amount: Some(usd_money(1000)),
        merchant_refund_id: Some("probe_refund_001".to_string()),
        reason: Some("customer_request".to_string()),
        ..Default::default()
    }
}

pub(crate) fn base_void_request() -> PaymentServiceVoidRequest {
    PaymentServiceVoidRequest {
        connector_transaction_id: "probe_connector_txn_001".to_string(),
        merchant_void_id: Some("probe_void_001".to_string()),
        ..Default::default()
    }
}

pub(crate) fn base_get_request() -> PaymentServiceGetRequest {
    PaymentServiceGetRequest {
        connector_transaction_id: "probe_connector_txn_001".to_string(),
        merchant_transaction_id: Some("probe_merchant_txn_001".to_string()),
        amount: Some(usd_money(1000)),
        ..Default::default()
    }
}

pub(crate) fn base_reverse_request() -> PaymentServiceReverseRequest {
    PaymentServiceReverseRequest {
        connector_transaction_id: "probe_connector_txn_001".to_string(),
        merchant_reverse_id: Some("probe_reverse_001".to_string()),
        ..Default::default()
    }
}

pub(crate) fn base_create_order_request() -> PaymentServiceCreateOrderRequest {
    PaymentServiceCreateOrderRequest {
        amount: Some(usd_money(1000)),
        merchant_order_id: Some("probe_order_001".to_string()),
        ..Default::default()
    }
}

pub(crate) fn base_setup_recurring_request() -> PaymentServiceSetupRecurringRequest {
    PaymentServiceSetupRecurringRequest {
        amount: Some(usd_money(0)),
        payment_method: Some(card_payment_method()),
        address: Some(PaymentAddress {
            billing_address: Some(Address::default()),
            shipping_address: None,
        }),
        auth_type: AuthenticationType::NoThreeDs as i32,
        return_url: Some("https://example.com/mandate-return".to_string()),
        merchant_recurring_payment_id: "probe_mandate_001".to_string(),
        setup_future_usage: Some(proto::FutureUsage::OffSession as i32),
        customer_acceptance: Some(CustomerAcceptance {
            acceptance_type: AcceptanceType::Offline as i32,
            accepted_at: 0,
            online_mandate_details: None,
        }),
        // browser_info intentionally omitted - let patching discover required fields
        ..Default::default()
    }
}

pub(crate) fn base_recurring_charge_request() -> RecurringPaymentServiceChargeRequest {
    RecurringPaymentServiceChargeRequest {
        amount: Some(usd_money(1000)),
        payment_method: Some(PaymentMethod {
            payment_method: Some(PmVariant::Token(proto::TokenPaymentMethodType {
                token: Some(Secret::new("probe_pm_token".to_string())),
            })),
        }),
        off_session: Some(true),
        // Must match customer.id used in setup_recurring so connectors that key
        // recurring charges by shopper/customer reference (e.g. Adyen shopperReference)
        // see the same identifier in both steps.
        connector_customer_id: Some("cust_probe_123".to_string()),
        return_url: Some("https://example.com/recurring-return".to_string()),
        payment_method_type: Some(proto::PaymentMethodType::PayPal as i32),
        connector_recurring_payment_id: Some(MandateReference {
            mandate_id_type: Some(MandateIdType::ConnectorMandateId(
                ConnectorMandateReferenceId {
                    connector_mandate_id: Some("probe-mandate-123".to_string()),
                    payment_method_id: None,
                    connector_mandate_request_reference_id: None,
                },
            )),
        }),
        ..Default::default()
    }
}

pub(crate) fn base_create_customer_request() -> CustomerServiceCreateRequest {
    // create_customer is explicitly about registering a customer — pre-populate
    // all standard customer fields so connectors get a complete customer record.
    CustomerServiceCreateRequest {
        merchant_customer_id: Some("cust_probe_123".to_string()),
        customer_name: Some("John Doe".to_string()),
        email: Some(Secret::new("test@example.com".to_string())),
        phone_number: Some("4155552671".to_string()),
        ..Default::default()
    }
}

pub(crate) fn base_tokenize_request() -> PaymentMethodServiceTokenizeRequest {
    PaymentMethodServiceTokenizeRequest {
        amount: Some(usd_money(1000)),
        payment_method: Some(card_payment_method()),
        address: Some(PaymentAddress {
            billing_address: Some(Address::default()),
            shipping_address: None,
        }),
        ..Default::default()
    }
}

pub(crate) fn base_create_server_authentication_token_request(
) -> MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest {
    MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest {
        ..Default::default()
    }
}

pub(crate) fn base_create_server_session_authentication_token_request(
) -> MerchantAuthenticationServiceCreateServerSessionAuthenticationTokenRequest {
    MerchantAuthenticationServiceCreateServerSessionAuthenticationTokenRequest {
        domain_context: Some(
            grpc_api_types::payments::merchant_authentication_service_create_server_session_authentication_token_request::DomainContext::Payment(
                grpc_api_types::payments::PaymentSessionContext {
                    amount: Some(usd_money(1000)),
                    ..Default::default()
                },
            ),
        ),
        ..Default::default()
    }
}

pub(crate) fn base_create_client_authentication_token_request(
) -> MerchantAuthenticationServiceCreateClientAuthenticationTokenRequest {
    MerchantAuthenticationServiceCreateClientAuthenticationTokenRequest {
        domain_context: Some(
            grpc_api_types::payments::merchant_authentication_service_create_client_authentication_token_request::DomainContext::Payment(
                PaymentClientAuthenticationContext {
                    amount: Some(usd_money(1000)),
                    ..Default::default()
                },
            ),
        ),
        ..Default::default()
    }
}

pub(crate) fn base_pre_authenticate_request(
) -> PaymentMethodAuthenticationServicePreAuthenticateRequest {
    PaymentMethodAuthenticationServicePreAuthenticateRequest {
        payment_method: Some(card_payment_method()),
        amount: Some(usd_money(1000)),
        // Minimal empty address — domain layer checks address.is_some() before transformers run.
        address: Some(PaymentAddress {
            billing_address: Some(Address::default()),
            shipping_address: None,
        }),
        return_url: Some("https://example.com/3ds-return".to_string()),
        ..Default::default()
    }
}

pub(crate) fn base_authenticate_request() -> PaymentMethodAuthenticationServiceAuthenticateRequest {
    PaymentMethodAuthenticationServiceAuthenticateRequest {
        payment_method: Some(card_payment_method()),
        amount: Some(usd_money(1000)),
        // Minimal empty address — same rationale as pre_authenticate.
        address: Some(PaymentAddress {
            billing_address: Some(Address::default()),
            shipping_address: None,
        }),
        return_url: Some("https://example.com/3ds-return".to_string()),
        ..Default::default()
    }
}

pub(crate) fn base_post_authenticate_request(
) -> PaymentMethodAuthenticationServicePostAuthenticateRequest {
    PaymentMethodAuthenticationServicePostAuthenticateRequest {
        payment_method: Some(card_payment_method()),
        amount: Some(usd_money(1000)),
        // Minimal empty address — same rationale as pre_authenticate.
        address: Some(PaymentAddress {
            billing_address: Some(Address::default()),
            shipping_address: None,
        }),
        ..Default::default()
    }
}

pub(crate) fn base_accept_dispute_request() -> DisputeServiceAcceptRequest {
    DisputeServiceAcceptRequest {
        merchant_dispute_id: Some("probe_dispute_001".to_string()),
        connector_transaction_id: "probe_txn_001".to_string(),
        dispute_id: "probe_dispute_id_001".to_string(),
    }
}

pub(crate) fn base_submit_evidence_request() -> DisputeServiceSubmitEvidenceRequest {
    DisputeServiceSubmitEvidenceRequest {
        merchant_dispute_id: Some("probe_dispute_001".to_string()),
        connector_transaction_id: Some("probe_txn_001".to_string()),
        dispute_id: "probe_dispute_id_001".to_string(),
        evidence_documents: vec![EvidenceDocument {
            evidence_type: EvidenceType::ServiceDocumentation as i32,
            file_content: Some(b"probe evidence content".to_vec()),
            file_mime_type: Some("application/pdf".to_string()),
            provider_file_id: None,
            text_content: None,
        }],
        ..Default::default()
    }
}

pub(crate) fn base_defend_dispute_request() -> DisputeServiceDefendRequest {
    DisputeServiceDefendRequest {
        merchant_dispute_id: Some("probe_dispute_001".to_string()),
        connector_transaction_id: "probe_txn_001".to_string(),
        dispute_id: "probe_dispute_id_001".to_string(),
        reason_code: Some("probe_reason".to_string()),
    }
}

// ── Non-PCI (Tokenized / Proxy) request builders ──────────────────────────────

fn base_card_proxy() -> CardDetails {
    CardDetails {
        card_number: Some(CardNumber::from_str("4111111111111111").unwrap()),
        card_exp_month: Some(Secret::new("03".to_string())),
        card_exp_year: Some(Secret::new("2030".to_string())),
        card_cvc: Some(Secret::new("123".to_string())),
        card_holder_name: Some(Secret::new("John Doe".to_string())),
        ..Default::default()
    }
}

pub(crate) fn base_tokenized_authorize_request() -> PaymentServiceTokenAuthorizeRequest {
    PaymentServiceTokenAuthorizeRequest {
        merchant_transaction_id: Some("probe_tokenized_txn_001".to_string()),
        amount: Some(usd_money(1000)),
        connector_token: Some(Secret::new("pm_1AbcXyzStripeTestToken".to_string())),
        capture_method: Some(CaptureMethod::Automatic as i32),
        address: Some(PaymentAddress {
            billing_address: Some(Address::default()),
            ..Default::default()
        }),
        return_url: Some("https://example.com/return".to_string()),
        ..Default::default()
    }
}

pub(crate) fn base_tokenized_setup_recurring_request() -> PaymentServiceTokenSetupRecurringRequest {
    PaymentServiceTokenSetupRecurringRequest {
        merchant_recurring_payment_id: "probe_tokenized_mandate_001".to_string(),
        amount: Some(usd_money(0)),
        connector_token: Some(Secret::new("pm_1AbcXyzStripeTestToken".to_string())),
        address: Some(PaymentAddress {
            billing_address: Some(Address::default()),
            ..Default::default()
        }),
        customer_acceptance: Some(CustomerAcceptance {
            accepted_at: 0,
            acceptance_type: AcceptanceType::Online as i32,
            online_mandate_details: Some(proto::OnlineMandate {
                ip_address: Some("127.0.0.1".to_string()),
                user_agent: "Mozilla/5.0".to_string(),
            }),
        }),
        setup_mandate_details: Some(proto::SetupMandateDetails {
            mandate_type: Some(proto::MandateType {
                mandate_type: Some(proto::mandate_type::MandateType::MultiUse(
                    proto::MandateAmountData {
                        amount: 0,
                        currency: proto::Currency::Usd as i32,
                        ..Default::default()
                    },
                )),
            }),
            ..Default::default()
        }),
        setup_future_usage: Some(proto::FutureUsage::OffSession as i32),
        connector_feature_data: Some(Secret::new("{}".to_string())),
        ..Default::default()
    }
}

pub(crate) fn base_proxied_authorize_request() -> PaymentServiceProxyAuthorizeRequest {
    PaymentServiceProxyAuthorizeRequest {
        merchant_transaction_id: Some("probe_proxy_txn_001".to_string()),
        amount: Some(usd_money(1000)),
        card_proxy: Some(base_card_proxy()),
        capture_method: Some(CaptureMethod::Automatic as i32),
        auth_type: AuthenticationType::NoThreeDs as i32,
        address: Some(PaymentAddress {
            billing_address: Some(Address::default()),
            ..Default::default()
        }),
        return_url: Some("https://example.com/return".to_string()),
        ..Default::default()
    }
}

pub(crate) fn base_proxied_setup_recurring_request() -> PaymentServiceProxySetupRecurringRequest {
    PaymentServiceProxySetupRecurringRequest {
        merchant_recurring_payment_id: "probe_proxy_mandate_001".to_string(),
        amount: Some(usd_money(0)),
        card_proxy: Some(base_card_proxy()),
        auth_type: AuthenticationType::NoThreeDs as i32,
        address: Some(PaymentAddress {
            billing_address: Some(Address::default()),
            ..Default::default()
        }),
        customer_acceptance: Some(CustomerAcceptance {
            acceptance_type: AcceptanceType::Offline as i32,
            accepted_at: 0,
            online_mandate_details: None,
        }),
        setup_future_usage: Some(proto::FutureUsage::OffSession as i32),
        ..Default::default()
    }
}
