/*
Singleton (Creational Patterns)
    |
    |
 Request
    |
    |
Factory Method (Creational Patterns)
    |
    |
Strategy Pattern (Behavioral Patterns)
    |
    |
Adapter Pattern - Request (Structural Patterns)
    |
    |
Template Method (Behavioral Patterns)
    |
    |
Builder Method (Creational Patterns)
    |
    |
 HTTP Call
    |
    |
Adapter Pattern - Response (Structural Patterns)
*/

use std::sync::{Arc, OnceLock};

#[derive(Debug)]
pub struct Config {
    pub server: Server,
}

#[derive(Debug)]
pub struct Server {
    pub host: String,
    pub port: u16,
}

static CONFIG: OnceLock<Arc<Config>> = OnceLock::new();

impl Config {
    pub fn init(host: String, port: u16) {
        let config = Config {
            server: Server { host, port },
        };

        CONFIG
            .set(Arc::new(config))
            .expect("Config already initialized");
    }

    pub fn instance() -> Arc<Config> {
        CONFIG
            .get()
            .expect("Config has not been initialized")
            .clone()
    }
}

#[derive(Debug)]
pub enum Method {
    POST,
    GET,
}

#[derive(Debug)]
pub struct Request {
    pub method: Method,
    pub url: String,
}

#[derive(Debug)]
pub struct RequestBuilder {
    pub method: Method,
    pub url: String,
}

impl RequestBuilder {
    fn new() -> RequestBuilder {
        Self {
            method: Method::POST,
            url: String::new(),
        }
    }
    pub fn method(mut self, method: Method) -> Self {
        self.method = method;
        self
    }
    pub fn url(mut self, url: String) -> Self {
        self.url = url;
        self
    }
    fn build(self) -> Request {
        Request {
            method: self.method,
            url: self.url,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PaymentRequest {
    pub amount: i64,
    pub currency: String,
    pub card_number: String,
}

#[derive(Debug)]
pub enum PaymentStatus {
    Authorized,
    Failed,
}

#[derive(Debug)]
pub struct PaymentResponse {
    pub status: PaymentStatus,
    pub connector_transaction_id: String,
}

type BoxedConnectorIntegration<'a, Req> = Box<&'a dyn ConnectorIntegrationV2<Req>>;

// Strategy (Behavioral)
// Strategy interface
/*
                 ConnectorIntegrationV2
                       Strategy
                          |
              +-----------+-----------+
              |                       |
           Adyen                    Stripe
      Concrete Strategy        Concrete Strategy
*/
pub trait ConnectorIntegrationV2<Req>: ConnectorIntegrationAnyV2<Req> {
    fn get_http_method(&self) -> Method;

    fn get_url(&self) -> String;

    // Template Method (Behavioral)
    // Fixed skeleton; each connector fills in get_http_method and get_url
    fn build_requestv2(&self) -> Option<Request> {
        Some(
            // Builder (Creational)
            // Build a complex object step-by-step
            RequestBuilder::new()
                .method(self.get_http_method())
                .url(self.get_url())
                .build(),
        )
    }
}

pub trait ConnectorIntegrationAnyV2<Req> {
    fn get_connector_integrationv2(&self) -> BoxedConnectorIntegration<'_, Req>;
}

impl<S, Req> ConnectorIntegrationAnyV2<Req> for S
where
    S: ConnectorIntegrationV2<Req>,
{
    fn get_connector_integrationv2(&self) -> BoxedConnectorIntegration<'_, Req> {
        Box::new(self)
    }
}

pub trait ConnectorServiceTrait: PaymentAuthorizeV2 {}

pub trait PaymentAuthorizeV2: ConnectorIntegrationV2<PaymentRequest> {}

pub type BoxedConnector = Box<&'static dyn ConnectorServiceTrait>;
pub struct ConnectorData {
    pub connector: BoxedConnector,
    pub connector_name: ConnectorEnum,
}

impl ConnectorData {
    // Factory (Creational)
    // Here the factory is deciding which object to create based on the runtime value of connector
    fn get_connector_by_name(connector: ConnectorEnum) -> ConnectorData {
        match connector {
            ConnectorEnum::Adyen => Self {
                connector: Box::new(Adyen::new()),
                connector_name: connector,
            },
        }
    }
}

pub struct Adyen;

pub enum ConnectorEnum {
    Adyen,
}

impl Adyen {
    fn new() -> &'static Self {
        &Self
    }
}

// Adyen-specific request (the shape Adyen's API wants)
#[derive(Debug)]
pub struct AdyenPaymentRequest {
    pub amount: AdyenAmount,
    pub merchant_account: String,
    pub payment_method: AdyenPaymentMethod,
}

#[derive(Debug)]
pub struct AdyenAmount {
    pub value: i64,
    pub currency: String,
}

#[derive(Debug)]
pub struct AdyenPaymentMethod {
    pub method_type: String, // "scheme" for cards
    pub number: String,
}


// Adapter (Structural) - Request
// Convert the unified request into the shape Adyen's API expects
impl TryFrom<&PaymentRequest> for AdyenPaymentRequest {
    type Error = String;
    fn try_from(req: &PaymentRequest) -> Result<Self, Self::Error> {
        if req.amount <= 0 {
            return Err("Amount must be positive".into());
        }
        Ok(Self {
            amount: AdyenAmount {
                value: req.amount,
                currency: req.currency.clone(),
            },
            merchant_account: "MyMerchantAccount".into(),
            payment_method: AdyenPaymentMethod {
                method_type: "scheme".into(),
                number: req.card_number.clone(),
            },
        })
    }
}

// Adyen-specific response (the shape Adyen's API returns)
#[derive(Debug)]
pub struct AdyenPaymentResponse {
    pub psp_reference: String,
    pub result_code: String,
}

// Adapter (Structural) - Response
// Convert Adyen's response back into the unified shape
impl TryFrom<AdyenPaymentResponse> for PaymentResponse {
    type Error = String;
    fn try_from(res: AdyenPaymentResponse) -> Result<Self, Self::Error> {
        let status = match res.result_code.as_str() {
            "Authorised" => PaymentStatus::Authorized,
            "Refused" => PaymentStatus::Failed,
            other => return Err(format!("Unknown resultCode: {other}")),
        };
        Ok(Self {
            status,
            connector_transaction_id: res.psp_reference,
        })
    }
}

impl ConnectorServiceTrait for Adyen {}

impl PaymentAuthorizeV2 for Adyen {}

// concrete strategy
impl ConnectorIntegrationV2<PaymentRequest> for Adyen {
    fn get_http_method(&self) -> Method {
        Method::POST
    }

    fn get_url(&self) -> String {
        "https://api.adyen.io".to_string()
    }
}

pub struct Stripe;

// concrete strategy
impl ConnectorIntegrationV2<PaymentRequest> for Stripe {
    fn get_http_method(&self) -> Method {
        Method::POST
    }

    fn get_url(&self) -> String {
        "https://api.stripe.com".to_string()
    }
}

pub fn process_internal_authorization(
    connector: ConnectorEnum,
    payload: PaymentRequest,
    config: Arc<Config>,
) -> Result<PaymentResponse, String> {
    println!(
        "Server running on {}:{}",
        config.server.host, config.server.port
    );

    // Selecting the strategy

    let connector_data = ConnectorData::get_connector_by_name(connector);

    // Strategy (Behavioral)

    let connector_integration_v2: BoxedConnectorIntegration<'_, PaymentRequest> =
        connector_data.connector.get_connector_integrationv2();

    // Adapter (Structural) - Request
    let connector_request = AdyenPaymentRequest::try_from(&payload)?;
    println!("{:?}", connector_request);

    // Template Method (Behavioral) + Builder (Creational)
    let request = connector_integration_v2.build_requestv2();
    println!("{:?}", request);

    // HTTP Call (mocked): pretend Adyen replied
    let connector_response = AdyenPaymentResponse {
        psp_reference: "8515131751004933".to_string(),
        result_code: "Authorised".to_string(),
    };

    // Adapter (Structural) - Response
    PaymentResponse::try_from(connector_response)
}

fn main() {
    // Singleton (Creational)
    Config::init("http://localhost".to_string(), 8080);
    let config = Config::instance();

    let payload = PaymentRequest {
        amount: 1000,
        currency: "EUR".to_string(),
        card_number: "4111111111111111".to_string(),
    };

    let response = process_internal_authorization(ConnectorEnum::Adyen, payload, config);
    println!("{:?}", response);
}
