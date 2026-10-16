// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py paypal
//
// Paypal — all scenarios and flows in one file.
// Run a scenario:  cargo run --example paypal -- process_checkout_card
use cards::CardNumber;
use grpc_api_types::payments::connector_specific_config;
use grpc_api_types::payments::payment_method;
use grpc_api_types::payments::*;
use hyperswitch_masking::Secret;
use hyperswitch_payments_client::ConnectorClient;
use std::collections::HashMap;
use std::str::FromStr;

#[allow(dead_code)]
pub const SUPPORTED_FLOWS: &[&str] = &[
    "authenticate",
    "authorize",
    "capture",
    "create_client_authentication_token",
    "create_order",
    "create_server_authentication_token",
    "get",
    "parse_event",
    "post_authenticate",
    "proxy_authorize",
    "recurring_charge",
    "refund",
    "refund_get",
    "setup_recurring",
    "void",
];

#[allow(dead_code)]
fn build_client() -> ConnectorClient {
    // Configure the connector with authentication
    let config = ConnectorConfig {
        connector_config: Some(ConnectorSpecificConfig {
            config: Some(connector_specific_config::Config::Paypal(PaypalConfig {
                client_id: Some(hyperswitch_masking::Secret::new(
                    "YOUR_CLIENT_ID".to_string(),
                )), // Authentication credential
                client_secret: Some(hyperswitch_masking::Secret::new(
                    "YOUR_CLIENT_SECRET".to_string(),
                )), // Authentication credential
                payer_id: Some(hyperswitch_masking::Secret::new(
                    "YOUR_PAYER_ID".to_string(),
                )), // Authentication credential
                base_url: Some("https://sandbox.example.com".to_string()), // Base URL for API calls
                ..Default::default()
            })),
        }),
        options: Some(SdkOptions {
            environment: Environment::Sandbox.into(),
        }),
    };
    ConnectorClient::new(config, None).unwrap()
}

pub fn build_authenticate_request() -> PaymentMethodAuthenticationServiceAuthenticateRequest {
    PaymentMethodAuthenticationServiceAuthenticateRequest {
        amount: Some(Money {  // Amount Information.
            minor_amount: 1000,  // Amount in minor units (e.g., 1000 = $10.00).
            currency: Currency::Usd.into(),  // ISO 4217 currency code (e.g., "USD", "EUR").
        }),
        payment_method: Some(PaymentMethod {  // Payment Method.
            payment_method: Some(payment_method::PaymentMethod::Card(CardDetails {
                card_number: Some(CardNumber::from_str("4111111111111111").unwrap()),  // Card Identification.
                card_exp_month: Some(Secret::new("03".to_string())),
                card_exp_year: Some(Secret::new("2030".to_string())),
                card_cvc: Some(Secret::new("737".to_string())),
                card_holder_name: Some(Secret::new("John Doe".to_string())),  // Cardholder Information.
                ..Default::default()
            })),
        }),
        address: Some(PaymentAddress {  // Address Information.
            shipping_address: Some(Address {
                first_name: Some(Secret::new("Jane".to_string())),  // Personal Information.
                last_name: Some(Secret::new("Doe".to_string())),
                line1: Some(Secret::new("12 Main Street".to_string())),  // Address Details.
                city: Some(Secret::new("Seattle".to_string())),
                state: Some(Secret::new("WA".to_string())),
                zip_code: Some(Secret::new("98101".to_string())),
                country_alpha2_code: Some(CountryAlpha2::Us.into()),
                ..Default::default()
            }),
            billing_address: Some(Address {
                ..Default::default()
            }),
        }),
        connector_feature_data: Some(Secret::new("{\"authentication_order_context\":{\"version\":1,\"request\":{\"merchant_transaction_id\":\"probe_paypal_native\",\"merchant_order_id\":\"probe_native_order\",\"order_tax_amount\":100,\"shipping_cost\":200,\"setup_future_usage\":\"OFF_SESSION\",\"billing_descriptor\":{\"statement_descriptor\":\"PROBE NATIVE\"},\"l2_l3_data\":{\"order_info\":{\"merchant_order_reference_id\":\"probe_native_l23\",\"order_details\":[]}},\"description\":\"Native safe context probe\"}}}".to_string())),  // Connector-specific metadata for the transaction.
        return_url: Some("https://example.com/3ds-return".to_string()),  // URLs for Redirection.
        state: Some(ConnectorState {  // State Information.
            access_token: Some(AccessToken {  // Access token obtained from connector.
                token: Some(Secret::new("probe_access_token".to_string())),  // The token string.
                expires_in_seconds: Some(3600),  // Expiration timestamp (seconds since epoch).
                token_type: Some("Bearer".to_string()),  // Token type (e.g., "Bearer", "Basic").
            }),
            ..Default::default()
        }),
        ..Default::default()
    }
}

pub fn build_authorize_request(capture_method: &str) -> PaymentServiceAuthorizeRequest {
    PaymentServiceAuthorizeRequest {
        merchant_transaction_id: Some("probe_txn_001".to_string()),  // Identification.
        amount: Some(Money {  // The amount for the payment.
            minor_amount: 1000,  // Amount in minor units (e.g., 1000 = $10.00).
            currency: Currency::Usd.into(),  // ISO 4217 currency code (e.g., "USD", "EUR").
        }),
        order_tax_amount: Some(100),  // Tax amount for the order.
        shipping_cost: Some(200),  // Cost of shipping for the order.
        payment_method: Some(PaymentMethod {  // Payment method to be used.
            payment_method: Some(payment_method::PaymentMethod::Card(CardDetails {
                card_number: Some(CardNumber::from_str("4111111111111111").unwrap()),  // Card Identification.
                card_exp_month: Some(Secret::new("03".to_string())),
                card_exp_year: Some(Secret::new("2030".to_string())),
                card_cvc: Some(Secret::new("737".to_string())),
                card_holder_name: Some(Secret::new("John Doe".to_string())),  // Cardholder Information.
                ..Default::default()
            })),
        }),
        capture_method: Some(CaptureMethod::from_str_name(capture_method).unwrap_or_default().into()),  // Method for capturing the payment.
        address: Some(PaymentAddress {  // Address Information.
            shipping_address: Some(Address {
                first_name: Some(Secret::new("Jane".to_string())),  // Personal Information.
                last_name: Some(Secret::new("Doe".to_string())),
                line1: Some(Secret::new("12 Main Street".to_string())),  // Address Details.
                city: Some(Secret::new("Seattle".to_string())),
                state: Some(Secret::new("WA".to_string())),
                zip_code: Some(Secret::new("98101".to_string())),
                country_alpha2_code: Some(CountryAlpha2::Us.into()),
                ..Default::default()
            }),
            billing_address: Some(Address {
                line2: Some(Secret::new("Suite 10".to_string())),
                state: Some(Secret::new("WA".to_string())),
                ..Default::default()
            }),
        }),
        auth_type: AuthenticationType::NoThreeDs.into(),  // Authentication Details.
        connector_feature_data: Some(Secret::new("{\"authorize_id\":\"probe_auth_001\",\"capture_id\":\"probe_cap_001\",\"psync_flow\":\"CAPTURE\",\"order_id\":\"probe_order_001\"}".to_string())),  // Connector-specific feature data for the transaction.
        return_url: Some("https://example.com/return".to_string()),  // URLs for Redirection and Webhooks.
        billing_descriptor: Some(BillingDescriptor {  // Statement Descriptor.
            statement_descriptor: Some("PROBE DESCRIPTOR".to_string()),  // Complete description that appears on customers' statements.
            ..Default::default()
        }),
        state: Some(ConnectorState {  // State Information.
            access_token: Some(AccessToken {  // Access token obtained from connector.
                token: Some(Secret::new("probe_access_token".to_string())),  // The token string.
                expires_in_seconds: Some(3600),  // Expiration timestamp (seconds since epoch).
                token_type: Some("Bearer".to_string()),  // Token type (e.g., "Bearer", "Basic").
            }),
            ..Default::default()
        }),
        l2_l3_data: Some(L2l3Data {  // Level 2 / Level 3 data for enhanced payment processing.
            order_info: Some(OrderInfo {  // Order-level information.
                order_details: vec![  // Line items for the order.
                    OrderDetailsWithAmount {
                        product_name: "Probe item".to_string(),  // Name of the product that is being purchased.
                        quantity: 1,  // The quantity of the product to be purchased.
                        amount: 700,  // Amount per quantity (in minor currency units).
                        total_tax_amount: Some(100),  // Total tax amount (in minor currency units).
                        product_id: Some("sku_probe_001".to_string()),  // ID of the product that is being purchased.
                        description: Some("Probe line item".to_string()),  // Description for the item.
                        unit_of_measure: Some("EACH".to_string()),  // Unit of measure used for the item quantity.
                        commodity_code: Some("4900".to_string()),  // Commodity code describing the product classification.
                        ..Default::default()
                    },
                ],
                merchant_order_reference_id: Some("probe_invoice_001".to_string()),  // Merchant's internal reference ID for the order.
                shipping_cost: Some(200),  // Shipping cost for the order (in minor currency units).
                ..Default::default()
            }),
            tax_info: Some(TaxInfo {  // Tax information.
                order_tax_amount: Some(100),  // Total tax amount for the order (in minor currency units).
                ..Default::default()
            }),
        }),
        ..Default::default()
    }
}

pub fn build_capture_request(connector_transaction_id: &str) -> PaymentServiceCaptureRequest {
    PaymentServiceCaptureRequest {
        merchant_capture_id: Some("probe_capture_001".to_string()),  // Identification.
        connector_transaction_id: connector_transaction_id.to_string(),
        amount_to_capture: Some(Money {  // Capture Details.
            minor_amount: 1000,  // Amount in minor units (e.g., 1000 = $10.00).
            currency: Currency::Usd.into(),  // ISO 4217 currency code (e.g., "USD", "EUR").
        }),
        connector_feature_data: Some(Secret::new("{\"authorize_id\":\"probe_auth_001\",\"capture_id\":\"probe_cap_001\",\"psync_flow\":\"CAPTURE\",\"order_id\":\"probe_order_001\"}".to_string())),  // Connector-specific metadata for the transaction.
        capture_method: Some(CaptureMethod::Manual.into()),  // Capture Settings.
        state: Some(ConnectorState {  // State Information.
            access_token: Some(AccessToken {  // Access token obtained from connector.
                token: Some(Secret::new("probe_access_token".to_string())),  // The token string.
                expires_in_seconds: Some(3600),  // Expiration timestamp (seconds since epoch).
                token_type: Some("Bearer".to_string()),  // Token type (e.g., "Bearer", "Basic").
            }),
            ..Default::default()
        }),
        ..Default::default()
    }
}

pub fn build_create_client_authentication_token_request(
) -> MerchantAuthenticationServiceCreateClientAuthenticationTokenRequest {
    MerchantAuthenticationServiceCreateClientAuthenticationTokenRequest {
        merchant_client_session_id: "probe_sdk_session_001".to_string(),  // Infrastructure.
        connector_feature_data: Some(Secret::new("{\"authorize_id\":\"probe_auth_001\",\"capture_id\":\"probe_cap_001\",\"psync_flow\":\"CAPTURE\",\"order_id\":\"probe_order_001\"}".to_string())),
        // domain_context: {"payment": {"amount": {"minor_amount": 1000, "currency": "USD"}}}
        ..Default::default()
    }
}

pub fn build_create_order_request() -> PaymentServiceCreateOrderRequest {
    PaymentServiceCreateOrderRequest {
        merchant_order_id: Some("probe_order_001".to_string()),  // Identification.
        amount: Some(Money {  // Amount Information.
            minor_amount: 1000,  // Amount in minor units (e.g., 1000 = $10.00).
            currency: Currency::Usd.into(),  // ISO 4217 currency code (e.g., "USD", "EUR").
        }),
        connector_feature_data: Some(Secret::new("{\"authorize_id\":\"probe_auth_001\",\"capture_id\":\"probe_cap_001\",\"psync_flow\":\"CAPTURE\",\"order_id\":\"probe_order_001\"}".to_string())),  // Connector-specific metadata for the transaction.
        state: Some(ConnectorState {  // State Information.
            access_token: Some(AccessToken {  // Access token obtained from connector.
                token: Some(Secret::new("probe_access_token".to_string())),  // The token string.
                expires_in_seconds: Some(3600),  // Expiration timestamp (seconds since epoch).
                token_type: Some("Bearer".to_string()),  // Token type (e.g., "Bearer", "Basic").
            }),
            ..Default::default()
        }),
        ..Default::default()
    }
}

pub fn build_create_server_authentication_token_request(
) -> MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest {
    MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest {
        connector_feature_data: Some(Secret::new("{\"authorize_id\":\"probe_auth_001\",\"capture_id\":\"probe_cap_001\",\"psync_flow\":\"CAPTURE\",\"order_id\":\"probe_order_001\"}".to_string())),  // Connector-specific metadata for the transaction.
        ..Default::default()
    }
}

pub fn build_get_request(connector_transaction_id: &str) -> PaymentServiceGetRequest {
    PaymentServiceGetRequest {
        merchant_transaction_id: Some("probe_merchant_txn_001".to_string()),  // Identification.
        connector_transaction_id: connector_transaction_id.to_string(),
        amount: Some(Money {  // Amount Information.
            minor_amount: 1000,  // Amount in minor units (e.g., 1000 = $10.00).
            currency: Currency::Usd.into(),  // ISO 4217 currency code (e.g., "USD", "EUR").
        }),
        state: Some(ConnectorState {  // State Information.
            access_token: Some(AccessToken {  // Access token obtained from connector.
                token: Some(Secret::new("probe_access_token".to_string())),  // The token string.
                expires_in_seconds: Some(3600),  // Expiration timestamp (seconds since epoch).
                token_type: Some("Bearer".to_string()),  // Token type (e.g., "Bearer", "Basic").
            }),
            ..Default::default()
        }),
        connector_feature_data: Some(Secret::new("{\"authorize_id\":\"probe_auth_001\",\"capture_id\":\"probe_cap_001\",\"psync_flow\":\"CAPTURE\",\"order_id\":\"probe_order_001\"}".to_string())),  // Connector-specific metadata for the transaction.
        ..Default::default()
    }
}

#[allow(dead_code)]
pub fn build_handle_event_request() -> EventServiceHandleRequest {
    EventServiceHandleRequest {
        merchant_event_id: Some("probe_event_001".to_string()),
        request_details: Some(RequestDetails {
            method: HttpMethod::Post.into(),  // HTTP method of the request (e.g., GET, POST).
            uri: Some("https://example.com/webhook".to_string()),  // URI of the request.
            headers: [].into_iter().collect::<HashMap<_, _>>(),  // Headers of the HTTP request.
            body: "{\"event_type\":\"PAYMENT.CAPTURE.COMPLETED\",\"resource\":{\"id\":\"probe_capture_001\",\"status\":\"COMPLETED\",\"amount\":{\"value\":\"10.00\",\"currency_code\":\"USD\"}}}".as_bytes().to_vec(),  // Body of the HTTP request.
            ..Default::default()
        }),
        ..Default::default()
    }
}

pub fn build_parse_event_request() -> EventServiceParseRequest {
    EventServiceParseRequest {
        request_details: Some(RequestDetails {
            method: HttpMethod::Post.into(),  // HTTP method of the request (e.g., GET, POST).
            uri: Some("https://example.com/webhook".to_string()),  // URI of the request.
            headers: [].into_iter().collect::<HashMap<_, _>>(),  // Headers of the HTTP request.
            body: "{\"event_type\":\"PAYMENT.CAPTURE.COMPLETED\",\"resource\":{\"id\":\"probe_capture_001\",\"status\":\"COMPLETED\",\"amount\":{\"value\":\"10.00\",\"currency_code\":\"USD\"}}}".as_bytes().to_vec(),  // Body of the HTTP request.
            ..Default::default()
        }),
    }
}

pub fn build_post_authenticate_request() -> PaymentMethodAuthenticationServicePostAuthenticateRequest
{
    PaymentMethodAuthenticationServicePostAuthenticateRequest {
        amount: Some(Money {  // Amount Information.
            minor_amount: 1000,  // Amount in minor units (e.g., 1000 = $10.00).
            currency: Currency::Usd.into(),  // ISO 4217 currency code (e.g., "USD", "EUR").
        }),
        payment_method: Some(PaymentMethod {  // Payment Method.
            payment_method: Some(payment_method::PaymentMethod::Card(CardDetails {
                card_number: Some(CardNumber::from_str("4111111111111111").unwrap()),  // Card Identification.
                card_exp_month: Some(Secret::new("03".to_string())),
                card_exp_year: Some(Secret::new("2030".to_string())),
                card_cvc: Some(Secret::new("737".to_string())),
                card_holder_name: Some(Secret::new("John Doe".to_string())),  // Cardholder Information.
                ..Default::default()
            })),
        }),
        address: Some(PaymentAddress {  // Address Information.
            billing_address: Some(Address {
                ..Default::default()
            }),
            ..Default::default()
        }),
        connector_feature_data: Some(Secret::new("{\"authorize_id\":\"probe_auth_001\",\"capture_id\":\"probe_cap_001\",\"psync_flow\":\"CAPTURE\",\"order_id\":\"probe_order_001\"}".to_string())),  // Connector-specific metadata for the transaction.
        state: Some(ConnectorState {  // State Information.
            access_token: Some(AccessToken {  // Access token obtained from connector.
                token: Some(Secret::new("probe_access_token".to_string())),  // The token string.
                expires_in_seconds: Some(3600),  // Expiration timestamp (seconds since epoch).
                token_type: Some("Bearer".to_string()),  // Token type (e.g., "Bearer", "Basic").
            }),
            ..Default::default()
        }),
        ..Default::default()
    }
}

pub fn build_proxy_authorize_request() -> PaymentServiceProxyAuthorizeRequest {
    PaymentServiceProxyAuthorizeRequest {
        merchant_transaction_id: Some("probe_proxy_txn_001".to_string()),
        amount: Some(Money {
            minor_amount: 1000,  // Amount in minor units (e.g., 1000 = $10.00).
            currency: Currency::Usd.into(),  // ISO 4217 currency code (e.g., "USD", "EUR").
        }),
        card_proxy: Some(ProxyCardDetails {  // Card proxy for vault-aliased payments (VGS, Basis Theory, Spreedly). Real card values are substituted by the proxy before reaching the connector.
            card_number: Some(Secret::new("4111111111111111".to_string())),  // Card Identification.
            card_exp_month: Some(Secret::new("03".to_string())),
            card_exp_year: Some(Secret::new("2030".to_string())),
            card_cvc: Some(Secret::new("123".to_string())),
            card_holder_name: Some(Secret::new("John Doe".to_string())),  // Cardholder Information.
            card_network: Some(CardNetwork::Visa.into()),
            ..Default::default()
        }),
        address: Some(PaymentAddress {
            billing_address: Some(Address {
                ..Default::default()
            }),
            ..Default::default()
        }),
        capture_method: Some(CaptureMethod::Automatic.into()),
        auth_type: AuthenticationType::NoThreeDs.into(),
        return_url: Some("https://example.com/return".to_string()),
        connector_feature_data: Some(Secret::new("{\"authorize_id\":\"probe_auth_001\",\"capture_id\":\"probe_cap_001\",\"psync_flow\":\"CAPTURE\",\"order_id\":\"probe_order_001\"}".to_string())),
        state: Some(ConnectorState {
            access_token: Some(AccessToken {  // Access token obtained from connector.
                token: Some(Secret::new("probe_access_token".to_string())),  // The token string.
                expires_in_seconds: Some(3600),  // Expiration timestamp (seconds since epoch).
                token_type: Some("Bearer".to_string()),  // Token type (e.g., "Bearer", "Basic").
            }),
            ..Default::default()
        }),
        ..Default::default()
    }
}

pub fn build_recurring_charge_request() -> RecurringPaymentServiceChargeRequest {
    RecurringPaymentServiceChargeRequest {
        merchant_charge_id: Some("probe_paypal_mit_charge".to_string()),  // Reference ID for tracking.
        connector_recurring_payment_id: Some(MandateReference {  // Reference to existing mandate.
            mandate_id_type: Some(grpc_api_types::payments::mandate_reference::MandateIdType::NetworkMandateId(NetworkMandateId {
                network_transaction_id: "123456789012345".to_string(),
                ..Default::default()
            })),
        }),
        amount: Some(Money {  // Amount Information.
            minor_amount: 1000,  // Amount in minor units (e.g., 1000 = $10.00).
            currency: Currency::Usd.into(),  // ISO 4217 currency code (e.g., "USD", "EUR").
        }),
        payment_method: Some(PaymentMethod {  // Optional payment Method Information (for network transaction flows).
            payment_method: Some(payment_method::PaymentMethod::CardDetailsForNetworkTransactionId(CardDetailsForNetworkTransactionId {
                card_number: Some(CardNumber::from_str("4111111111111111").unwrap()),
                card_exp_month: Some(Secret::new("03".to_string())),
                card_exp_year: Some(Secret::new("2030".to_string())),
                card_network: Some(CardNetwork::Visa.into()),
                card_holder_name: Some(Secret::new("John Doe".to_string())),
                ..Default::default()
            })),
        }),
        metadata: Some(Secret::new("{\"paypal_mit\":{\"version\":1,\"previous_network_transaction_reference\":{\"id\":\"123456789012345\",\"network\":\"VISA\",\"date\":\"1014\"}}}".to_string())),  // Additional metadata for the connector.
        connector_feature_data: Some(Secret::new("{\"authorize_id\":\"probe_auth_001\",\"capture_id\":\"probe_cap_001\",\"psync_flow\":\"CAPTURE\",\"order_id\":\"probe_order_001\"}".to_string())),  // Connector-specific metadata for the transaction.
        return_url: Some("https://example.com/recurring-return".to_string()),
        address: Some(PaymentAddress {  // Address Information.
            shipping_address: Some(Address {
                first_name: Some(Secret::new("Jane".to_string())),  // Personal Information.
                last_name: Some(Secret::new("Doe".to_string())),
                line1: Some(Secret::new("12 Main Street".to_string())),  // Address Details.
                city: Some(Secret::new("Seattle".to_string())),
                state: Some(Secret::new("WA".to_string())),
                zip_code: Some(Secret::new("98101".to_string())),
                country_alpha2_code: Some(CountryAlpha2::Us.into()),
                ..Default::default()
            }),
            ..Default::default()
        }),
        connector_customer_id: Some("cust_probe_123".to_string()),
        payment_method_type: Some(PaymentMethodType::PayPal.into()),
        off_session: Some(true),  // Behavioral Flags and Preferences.
        state: Some(ConnectorState {  // State Information.
            access_token: Some(AccessToken {  // Access token obtained from connector.
                token: Some(Secret::new("probe_access_token".to_string())),  // The token string.
                expires_in_seconds: Some(3600),  // Expiration timestamp (seconds since epoch).
                token_type: Some("Bearer".to_string()),  // Token type (e.g., "Bearer", "Basic").
            }),
            ..Default::default()
        }),
        billing_descriptor: Some(BillingDescriptor {  // Billing Descriptor.
            statement_descriptor: Some("PROBE PAYPAL".to_string()),  // Complete description that appears on customers' statements.
            ..Default::default()
        }),
        l2_l3_data: Some(L2l3Data {  // Level 2 / Level 3 data for enhanced payment processing.
            order_info: Some(OrderInfo {  // Order-level information.
                merchant_order_reference_id: Some("probe_paypal_mit".to_string()),  // Merchant's internal reference ID for the order.
                ..Default::default()
            }),
            ..Default::default()
        }),
        ..Default::default()
    }
}

pub fn build_refund_request(connector_transaction_id: &str) -> PaymentServiceRefundRequest {
    PaymentServiceRefundRequest {
        merchant_refund_id: Some("probe_refund_001".to_string()),  // Identification.
        connector_transaction_id: connector_transaction_id.to_string(),
        payment_amount: 1000,  // Amount Information.
        refund_amount: Some(Money {
            minor_amount: 1000,  // Amount in minor units (e.g., 1000 = $10.00).
            currency: Currency::Usd.into(),  // ISO 4217 currency code (e.g., "USD", "EUR").
        }),
        reason: Some("customer_request".to_string()),  // Reason for the refund.
        connector_feature_data: Some(Secret::new("{\"authorize_id\":\"probe_auth_001\",\"capture_id\":\"probe_cap_001\",\"psync_flow\":\"CAPTURE\",\"order_id\":\"probe_order_001\"}".to_string())),  // Connector-specific metadata for the transaction.
        state: Some(ConnectorState {  // State data for access token storage and.
            access_token: Some(AccessToken {  // Access token obtained from connector.
                token: Some(Secret::new("probe_access_token".to_string())),  // The token string.
                expires_in_seconds: Some(3600),  // Expiration timestamp (seconds since epoch).
                token_type: Some("Bearer".to_string()),  // Token type (e.g., "Bearer", "Basic").
            }),
            ..Default::default()
        }),
        ..Default::default()
    }
}

pub fn build_refund_get_request() -> RefundServiceGetRequest {
    RefundServiceGetRequest {
        merchant_refund_id: Some("probe_refund_001".to_string()), // Identification.
        connector_transaction_id: "probe_connector_txn_001".to_string(),
        refund_id: "probe_refund_id_001".to_string(), // Deprecated.
        state: Some(ConnectorState {
            // State Information.
            access_token: Some(AccessToken {
                // Access token obtained from connector.
                token: Some(Secret::new("probe_access_token".to_string())), // The token string.
                expires_in_seconds: Some(3600), // Expiration timestamp (seconds since epoch).
                token_type: Some("Bearer".to_string()), // Token type (e.g., "Bearer", "Basic").
            }),
            ..Default::default()
        }),
        ..Default::default()
    }
}

pub fn build_setup_recurring_request() -> PaymentServiceSetupRecurringRequest {
    PaymentServiceSetupRecurringRequest {
        merchant_recurring_payment_id: "probe_mandate_001".to_string(),  // Identification.
        amount: Some(Money {  // Mandate Details.
            minor_amount: 0,  // Amount in minor units (e.g., 1000 = $10.00).
            currency: Currency::Usd.into(),  // ISO 4217 currency code (e.g., "USD", "EUR").
        }),
        payment_method: Some(PaymentMethod {
            payment_method: Some(payment_method::PaymentMethod::Card(CardDetails {
                card_number: Some(CardNumber::from_str("4111111111111111").unwrap()),  // Card Identification.
                card_exp_month: Some(Secret::new("03".to_string())),
                card_exp_year: Some(Secret::new("2030".to_string())),
                card_cvc: Some(Secret::new("737".to_string())),
                card_holder_name: Some(Secret::new("John Doe".to_string())),  // Cardholder Information.
                ..Default::default()
            })),
        }),
        address: Some(PaymentAddress {  // Address Information.
            billing_address: Some(Address {
                ..Default::default()
            }),
            ..Default::default()
        }),
        auth_type: AuthenticationType::NoThreeDs.into(),  // Type of authentication to be used.
        enrolled_for_3ds: false,  // Indicates if the customer is enrolled for 3D Secure.
        connector_feature_data: Some(Secret::new("{\"authorize_id\":\"probe_auth_001\",\"capture_id\":\"probe_cap_001\",\"psync_flow\":\"CAPTURE\",\"order_id\":\"probe_order_001\"}".to_string())),  // Connector-specific metadata for the transaction.
        return_url: Some("https://example.com/mandate-return".to_string()),  // URL to redirect after setup.
        setup_future_usage: Some(FutureUsage::OffSession.into()),  // Indicates future usage intention.
        request_incremental_authorization: false,  // Indicates if incremental authorization is requested.
        customer_acceptance: Some(CustomerAcceptance {  // Details of customer acceptance.
            acceptance_type: AcceptanceType::Offline.into(),  // Type of acceptance (e.g., online, offline).
            accepted_at: 0,  // Timestamp when the acceptance was made (Unix timestamp, seconds since epoch).
            ..Default::default()
        }),
        state: Some(ConnectorState {  // State data for access token storage and.
            access_token: Some(AccessToken {  // Access token obtained from connector.
                token: Some(Secret::new("probe_access_token".to_string())),  // The token string.
                expires_in_seconds: Some(3600),  // Expiration timestamp (seconds since epoch).
                token_type: Some("Bearer".to_string()),  // Token type (e.g., "Bearer", "Basic").
            }),
            ..Default::default()
        }),
        ..Default::default()
    }
}

pub fn build_void_request(connector_transaction_id: &str) -> PaymentServiceVoidRequest {
    PaymentServiceVoidRequest {
        merchant_void_id: Some("probe_void_001".to_string()),  // Identification.
        connector_transaction_id: connector_transaction_id.to_string(),
        connector_feature_data: Some(Secret::new("{\"authorize_id\":\"probe_auth_001\",\"capture_id\":\"probe_cap_001\",\"psync_flow\":\"CAPTURE\",\"order_id\":\"probe_order_001\"}".to_string())),  // Connector-specific metadata for the transaction.
        state: Some(ConnectorState {  // State Information.
            access_token: Some(AccessToken {  // Access token obtained from connector.
                token: Some(Secret::new("probe_access_token".to_string())),  // The token string.
                expires_in_seconds: Some(3600),  // Expiration timestamp (seconds since epoch).
                token_type: Some("Bearer".to_string()),  // Token type (e.g., "Bearer", "Basic").
            }),
            ..Default::default()
        }),
        ..Default::default()
    }
}

// Scenario: One-step Payment (Authorize + Capture)
// Simple payment that authorizes and captures in one call. Use for immediate charges.
#[allow(dead_code)]
pub async fn process_checkout_autocapture(
    client: &ConnectorClient,
    _merchant_transaction_id: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    // Step 1: Authorize — reserve funds on the payment method
    let authorize_response = client
        .authorize(build_authorize_request("AUTOMATIC"), &HashMap::new(), None)
        .await?;

    match authorize_response.status() {
        PaymentStatus::Failure | PaymentStatus::AuthorizationFailed => {
            return Err(format!("Payment failed: {:?}", authorize_response.error).into())
        }
        PaymentStatus::Pending => return Ok("pending — awaiting webhook".to_string()),
        _ => {}
    }

    Ok(format!(
        "Payment: {:?} — {}",
        authorize_response.status(),
        authorize_response
            .connector_transaction_id
            .as_deref()
            .unwrap_or("")
    ))
}

// Scenario: Card Payment (Authorize + Capture)
// Two-step card payment. First authorize, then capture. Use when you need to verify funds before finalizing.
#[allow(dead_code)]
pub async fn process_checkout_card(
    client: &ConnectorClient,
    _merchant_transaction_id: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    // Step 1: Authorize — reserve funds on the payment method
    let authorize_response = client
        .authorize(build_authorize_request("MANUAL"), &HashMap::new(), None)
        .await?;

    match authorize_response.status() {
        PaymentStatus::Failure | PaymentStatus::AuthorizationFailed => {
            return Err(format!("Payment failed: {:?}", authorize_response.error).into())
        }
        PaymentStatus::Pending => return Ok("pending — awaiting webhook".to_string()),
        _ => {}
    }

    // Step 2: Capture — settle the reserved funds
    let capture_response = client
        .capture(
            build_capture_request(
                authorize_response
                    .connector_transaction_id
                    .as_deref()
                    .unwrap_or(""),
            ),
            &HashMap::new(),
            None,
        )
        .await?;

    if capture_response.status() == PaymentStatus::Failure {
        return Err(format!("Capture failed: {:?}", capture_response.error).into());
    }

    Ok(format!(
        "Payment completed: {}",
        authorize_response
            .connector_transaction_id
            .as_deref()
            .unwrap_or("")
    ))
}

// Scenario: Refund
// Return funds to the customer for a completed payment.
#[allow(dead_code)]
pub async fn process_refund(
    client: &ConnectorClient,
    _merchant_transaction_id: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    // Step 1: Authorize — reserve funds on the payment method
    let authorize_response = client
        .authorize(build_authorize_request("AUTOMATIC"), &HashMap::new(), None)
        .await?;

    match authorize_response.status() {
        PaymentStatus::Failure | PaymentStatus::AuthorizationFailed => {
            return Err(format!("Payment failed: {:?}", authorize_response.error).into())
        }
        PaymentStatus::Pending => return Ok("pending — awaiting webhook".to_string()),
        _ => {}
    }

    // Step 2: Refund — return funds to the customer
    let refund_response = client
        .refund(
            build_refund_request(
                authorize_response
                    .connector_transaction_id
                    .as_deref()
                    .unwrap_or(""),
            ),
            &HashMap::new(),
            None,
        )
        .await?;

    if refund_response.status() == RefundStatus::RefundFailure {
        return Err(format!("Refund failed: {:?}", refund_response.error).into());
    }

    Ok(format!("Refunded: {:?}", refund_response.status()))
}

// Scenario: Void Payment
// Cancel an authorized but not-yet-captured payment.
#[allow(dead_code)]
pub async fn process_void_payment(
    client: &ConnectorClient,
    _merchant_transaction_id: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    // Step 1: Authorize — reserve funds on the payment method
    let authorize_response = client
        .authorize(build_authorize_request("MANUAL"), &HashMap::new(), None)
        .await?;

    match authorize_response.status() {
        PaymentStatus::Failure | PaymentStatus::AuthorizationFailed => {
            return Err(format!("Payment failed: {:?}", authorize_response.error).into())
        }
        PaymentStatus::Pending => return Ok("pending — awaiting webhook".to_string()),
        _ => {}
    }

    // Step 2: Void — release reserved funds (cancel authorization)
    let void_response = client
        .void(
            build_void_request(
                authorize_response
                    .connector_transaction_id
                    .as_deref()
                    .unwrap_or(""),
            ),
            &HashMap::new(),
            None,
        )
        .await?;

    Ok(format!("Voided: {:?}", void_response.status()))
}

// Scenario: Get Payment Status
// Retrieve current payment status from the connector.
#[allow(dead_code)]
pub async fn process_get_payment(
    client: &ConnectorClient,
    _merchant_transaction_id: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    // Step 1: Authorize — reserve funds on the payment method
    let authorize_response = client
        .authorize(build_authorize_request("MANUAL"), &HashMap::new(), None)
        .await?;

    match authorize_response.status() {
        PaymentStatus::Failure | PaymentStatus::AuthorizationFailed => {
            return Err(format!("Payment failed: {:?}", authorize_response.error).into())
        }
        PaymentStatus::Pending => return Ok("pending — awaiting webhook".to_string()),
        _ => {}
    }

    // Step 2: Get — retrieve current payment status from the connector
    let get_response = client
        .get(
            build_get_request(
                authorize_response
                    .connector_transaction_id
                    .as_deref()
                    .unwrap_or(""),
            ),
            &HashMap::new(),
            None,
        )
        .await?;

    Ok(format!("Status: {:?}", get_response.status()))
}

// Flow: PaymentMethodAuthenticationService.Authenticate
#[allow(dead_code)]
pub async fn process_authenticate(
    client: &ConnectorClient,
    _merchant_transaction_id: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let response = client
        .authenticate(build_authenticate_request(), &HashMap::new(), None)
        .await?;
    Ok(format!("status: {:?}", response.status()))
}

// Flow: PaymentService.Authorize (Card)
#[allow(dead_code)]
pub async fn process_authorize(
    client: &ConnectorClient,
    _merchant_transaction_id: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let response = client
        .authorize(build_authorize_request("AUTOMATIC"), &HashMap::new(), None)
        .await?;
    match response.status() {
        PaymentStatus::Failure | PaymentStatus::AuthorizationFailed => {
            Err(format!("Authorize failed: {:?}", response.error).into())
        }
        PaymentStatus::Pending => Ok("pending — await webhook".to_string()),
        _ => Ok(format!(
            "Authorized: {}",
            response.connector_transaction_id.as_deref().unwrap_or("")
        )),
    }
}

// Flow: PaymentService.Capture
#[allow(dead_code)]
pub async fn process_capture(
    client: &ConnectorClient,
    _merchant_transaction_id: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let response = client
        .capture(
            build_capture_request("probe_auth_001"),
            &HashMap::new(),
            None,
        )
        .await?;
    Ok(format!("status: {:?}", response.status()))
}

// Flow: MerchantAuthenticationService.CreateClientAuthenticationToken
#[allow(dead_code)]
pub async fn process_create_client_authentication_token(
    client: &ConnectorClient,
    _merchant_transaction_id: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let response = client
        .create_client_authentication_token(
            build_create_client_authentication_token_request(),
            &HashMap::new(),
            None,
        )
        .await?;
    Ok(format!("status: {:?}", response.status_code))
}

// Flow: PaymentService.CreateOrder
#[allow(dead_code)]
pub async fn process_create_order(
    client: &ConnectorClient,
    _merchant_transaction_id: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let response = client
        .create_order(build_create_order_request(), &HashMap::new(), None)
        .await?;
    Ok(format!("status: {:?}", response.status()))
}

// Flow: MerchantAuthenticationService.CreateServerAuthenticationToken
#[allow(dead_code)]
pub async fn process_create_server_authentication_token(
    client: &ConnectorClient,
    _merchant_transaction_id: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let response = client
        .create_server_authentication_token(
            build_create_server_authentication_token_request(),
            &HashMap::new(),
            None,
        )
        .await?;
    Ok(format!("status: {:?}", response.status()))
}

// Flow: PaymentService.Get
#[allow(dead_code)]
pub async fn process_get(
    client: &ConnectorClient,
    _merchant_transaction_id: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let response = client
        .get(build_get_request("probe_cap_001"), &HashMap::new(), None)
        .await?;
    Ok(format!("status: {:?}", response.status()))
}

// Flow: EventService.ParseEvent
#[allow(dead_code)]
pub async fn process_parse_event(
    client: &ConnectorClient,
    _merchant_transaction_id: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let response = client.parse_event(build_parse_event_request())?;
    Ok(format!("{response:?}"))
}

// Flow: PaymentMethodAuthenticationService.PostAuthenticate
#[allow(dead_code)]
pub async fn process_post_authenticate(
    client: &ConnectorClient,
    _merchant_transaction_id: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let response = client
        .post_authenticate(build_post_authenticate_request(), &HashMap::new(), None)
        .await?;
    Ok(format!("status: {:?}", response.status()))
}

// Flow: PaymentService.ProxyAuthorize
#[allow(dead_code)]
pub async fn process_proxy_authorize(
    client: &ConnectorClient,
    _merchant_transaction_id: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let response = client
        .proxy_authorize(build_proxy_authorize_request(), &HashMap::new(), None)
        .await?;
    Ok(format!("status: {:?}", response.status()))
}

// Flow: RecurringPaymentService.Charge
#[allow(dead_code)]
pub async fn process_recurring_charge(
    client: &ConnectorClient,
    _merchant_transaction_id: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let response = client
        .recurring_charge(build_recurring_charge_request(), &HashMap::new(), None)
        .await?;
    Ok(format!("status: {:?}", response.status()))
}

// Flow: RefundService.Get
#[allow(dead_code)]
pub async fn process_refund_get(
    client: &ConnectorClient,
    _merchant_transaction_id: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let response = client
        .refund_get(build_refund_get_request(), &HashMap::new(), None)
        .await?;
    Ok(format!("status: {:?}", response.status()))
}

// Flow: PaymentService.SetupRecurring
#[allow(dead_code)]
pub async fn process_setup_recurring(
    client: &ConnectorClient,
    _merchant_transaction_id: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let response = client
        .setup_recurring(build_setup_recurring_request(), &HashMap::new(), None)
        .await?;
    if response.status() == PaymentStatus::Failure {
        return Err(format!("Setup failed: {:?}", response.error).into());
    }
    Ok(format!(
        "Mandate: {}",
        response
            .connector_recurring_payment_id
            .as_deref()
            .unwrap_or("")
    ))
}

// Flow: PaymentService.Void
#[allow(dead_code)]
pub async fn process_void(
    client: &ConnectorClient,
    _merchant_transaction_id: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let response = client
        .void(build_void_request("probe_auth_001"), &HashMap::new(), None)
        .await?;
    Ok(format!("status: {:?}", response.status()))
}

#[allow(dead_code)]
#[tokio::main]
async fn main() {
    let client = build_client();
    let flow = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "process_checkout_autocapture".to_string());
    let result: Result<String, Box<dyn std::error::Error>> = match flow.as_str() {
        "process_checkout_autocapture" => process_checkout_autocapture(&client, "order_001").await,
        "process_checkout_card" => process_checkout_card(&client, "order_001").await,
        "process_refund" => process_refund(&client, "order_001").await,
        "process_void_payment" => process_void_payment(&client, "order_001").await,
        "process_get_payment" => process_get_payment(&client, "order_001").await,
        "process_authenticate" => process_authenticate(&client, "txn_001").await,
        "process_authorize" => process_authorize(&client, "txn_001").await,
        "process_capture" => process_capture(&client, "txn_001").await,
        "process_create_client_authentication_token" => {
            process_create_client_authentication_token(&client, "txn_001").await
        }
        "process_create_order" => process_create_order(&client, "txn_001").await,
        "process_create_server_authentication_token" => {
            process_create_server_authentication_token(&client, "txn_001").await
        }
        "process_get" => process_get(&client, "txn_001").await,
        "process_parse_event" => process_parse_event(&client, "txn_001").await,
        "process_post_authenticate" => process_post_authenticate(&client, "txn_001").await,
        "process_proxy_authorize" => process_proxy_authorize(&client, "txn_001").await,
        "process_recurring_charge" => process_recurring_charge(&client, "txn_001").await,
        "process_refund_get" => process_refund_get(&client, "txn_001").await,
        "process_setup_recurring" => process_setup_recurring(&client, "txn_001").await,
        "process_void" => process_void(&client, "txn_001").await,
        _ => {
            eprintln!("Unknown flow: {}. Available: process_checkout_autocapture, process_checkout_card, process_refund, process_void_payment, process_get_payment, process_authenticate, process_authorize, process_capture, process_create_client_authentication_token, process_create_order, process_create_server_authentication_token, process_get, process_parse_event, process_post_authenticate, process_proxy_authorize, process_recurring_charge, process_refund_get, process_setup_recurring, process_void", flow);
            return;
        }
    };
    match result {
        Ok(msg) => println!("✓ {msg}"),
        Err(e) => eprintln!("✗ {e}"),
    }
}
