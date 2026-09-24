# How We Used Low-Level Design While Building Hyperswitch Prism

Adding your first payment connector is fun. Adding your second is where you find out whether you designed anything at all.

Every processor wants something different: a different URL, a different HTTP method, a different JSON shape, a different idea of what a "card" looks like. If each connector is a pile of one-off code, connector #2 is a copy-paste of connector #1, and connector #50 is a maintenance nightmare.

Hyperswitch Prism talks to 100+ connectors today. That's not because we're fast typists. It's because a handful of boring, well-known design patterns do the heavy lifting. This post walks through them.

---

## Meet Hyperswitch Prism

Hyperswitch Prism is a stateless Rust library that takes one unified payment request and turns it into the right API call for 100+ processors. The codebase is large, so the snippets in this post come from a condensed reference version of it ([`prism-lld-reference.rs`](https://github.com/juspay/hyperswitch-prism/blob/main/docs/blogs/code/prism-lld-reference.rs)). It has the same structure, with the details removed.

Here's the whole journey of one payment, and the map for the rest of this post:

```
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
```

Let's walk it top to bottom.

---

## 1. Singleton: set the config once, read it anywhere

Every part of the service needs config, but it should be loaded exactly once. We don't want to pass it through twenty function signatures, and we don't want a mutable global either.

```rust
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
```

`OnceLock` gives us "write once, read forever" with thread safety for free, and `Arc` makes handing it out cheap. If someone calls `init` twice, or reads the config before it's set, the program fails loudly instead of quietly running with the wrong settings.

## 2. Request: one shape for every payment

The merchant never speaks Adyen or Stripe. They send one unified request:

```rust
#[derive(Debug, Clone)]
pub struct PaymentRequest {
    pub amount: i64,
    pub currency: String,
    pub card_number: String,
}
```

```rust
    let payload = PaymentRequest {
        amount: 1000,
        currency: "EUR".to_string(),
        card_number: "4111111111111111".to_string(),
    };
```

Everything after this point is about turning that one shape into whatever the chosen connector wants.

## 3. Factory Method: from a name to a connector

Which connector to use is decided at runtime: it arrives as data, not as code. The factory turns that value into a real object:

```rust
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
```

*(Purists will point out that this is technically a "simple factory". The GoF Factory Method relies on subclasses overriding a creation method. The idea is the same: callers ask for a connector, and one place decides how to make it.)*

The nice part is that Rust's `match` is exhaustive. Add a new variant to `ConnectorEnum` and forget the match arm, and the compiler won't let you ship it.

## 4. Strategy: one contract, many connectors

This is the heart of Prism. Every connector implements the same trait, so the core never has to care which one it's talking to:

```rust
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
```

And here's Adyen, one concrete strategy:

```rust
// concrete strategy
impl ConnectorIntegrationV2<PaymentRequest> for Adyen {
    fn get_http_method(&self) -> Method {
        Method::POST
    }

    fn get_url(&self) -> String {
        "https://api.adyen.io".to_string()
    }
}
```

That's all Adyen has to say. It answers two questions, *which method?* and *which URL?*, and doesn't touch anything else.

## 5. Adapter (request): speaking each connector's language

We know *who* we're talking to. Now we need to speak their language. Our unified `PaymentRequest` has to become the exact shape Adyen expects:

```rust
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
```

Adyen's quirks, like `"scheme"` for cards and a `merchant_account` field, live here and nowhere else. Validation lives here too, so bad input is rejected before we ever build an HTTP request. In the real codebase, these are the `transformers.rs` files that sit next to every connector.

## 6. Template Method: we own the steps, connectors fill the blanks

Who actually puts the request together? The trait does, through a default method:

```rust
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
```

This is the pattern we lean on the most. The *order* of steps is written once and shared by everyone. Connectors only plug in the parts that are genuinely theirs. When we want to change how every request is built, we change one method, not 100 connectors.

## 7. Builder: assembling the request step by step

Rather than one constructor with a long list of arguments, the request is built one step at a time:

```rust
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
```

It reads like a sentence, gives us sensible defaults (`POST`), and in the real Prism it grows to headers, bodies and certificates without the call site turning into a wall of arguments.

## 8. Adapter (response): bringing the answer home

The request goes out, and Adyen replies in its own format. Before that reaches the merchant, we translate it back. It's the same Adapter, just running the other way.

Every connector's reply lands in one unified shape:

```rust
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
```

And Adyen's version of "it worked" gets mapped into it:

```rust
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
```

This is where the real value is. Adyen says `"Authorised"`, another processor says `"succeeded"`, a third says `"APPROVED"`, and the merchant sees `Authorized` every time. Every connector has two adapters, one on the way in and one on the way out, and that's what lets the core treat 100+ processors as one.

---

## The small bit of Rust glue

One trick makes the Strategy easy to use. A blanket impl gives *every* connector a way to hand itself out as a strategy object, without writing a line:

```rust
impl<S, Req> ConnectorIntegrationAnyV2<Req> for S
where
    S: ConnectorIntegrationV2<Req>,
{
    fn get_connector_integrationv2(&self) -> BoxedConnectorIntegration<'_, Req> {
        Box::new(self)
    }
}
```

And supertraits let us describe a connector's capabilities as a stack:

```rust
pub trait ConnectorServiceTrait: PaymentAuthorizeV2 {}

pub trait PaymentAuthorizeV2: ConnectorIntegrationV2<PaymentRequest> {}
```

Want capture, refund or void? Add another trait to the stack. The compiler then checks that every connector implements all of them.

---

## Putting it together

Here's the whole diagram as code:

```rust
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
```

Run it, and each output line comes from a different step:

```
Server running on http://localhost:8080
AdyenPaymentRequest { amount: AdyenAmount { value: 1000, currency: "EUR" }, merchant_account: "MyMerchantAccount", payment_method: AdyenPaymentMethod { method_type: "scheme", number: "4111111111111111" } }
Some(Request { method: POST, url: "https://api.adyen.io" })
Ok(PaymentResponse { status: Authorized, connector_transaction_id: "8515131751004933" })
```

Singleton, then the request Adapter, then Template Method + Builder, then the response Adapter. The diagram, running top to bottom.

*(Two honest simplifications: to keep the snippet short, this calls Adyen's adapters directly, and the HTTP call is mocked. In Prism, each connector owns its own transformations, so the core never names a specific connector, and the request really goes over the wire.)*

---

## The real test: connector #2

This is the reason for all of it. Here's Stripe's strategy:

```rust
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
```

That's the whole strategy: two answers, same as Adyen. To finish the job, we'd add Stripe's two adapters (request and response), plus one variant and one match arm in the factory.

What we wouldn't touch: the Singleton, the unified `Request`, the Template Method, the Builder, or the orchestration function. On the diagram, only the Strategy and the two Adapter boxes get a new entry. Everything else stays exactly the same.

That's the real job of low-level design here. It's not about textbook purity. It's about making sure the 100th connector costs about as much as the 2nd.

---

Prism is open source, built in Rust, with SDKs across languages.

{% github juspay/hyperswitch-prism %}

- Node.js: `npm install hyperswitch-prism`
- Python: `pip install hyperswitch-prism`
- Java: `io.hyperswitch:prism` on Maven Central
- Docs: [docs.hyperswitch.io/integrations/prism/prism/installation](https://docs.hyperswitch.io/integrations/prism/prism/installation)

---

*If this was useful, a ⭐ on GitHub goes a long way. And if you've used a different pattern to tame connector sprawl, we'd love to hear about it in the comments.*
