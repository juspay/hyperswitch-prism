




# Card Payment (Authorize + Capture)

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/manifest.json
Scenario: checkout_card
Regenerate: python3 scripts/generators/docs/generate.py --scenarios
-->

## Overview

Reserve funds with Authorize, then settle with a separate Capture call. Use for physical goods or delayed fulfillment where capture happens later.

**Use this for:** Physical goods or delayed fulfillment where you need to reserve funds before shipping

**Flows used:** authorize, capture


**Payment Method:** Card




## Quick Start

Choose your SDK language to see a complete working example:

<table>
<tr><td><b>Python</b></td><td><b>JavaScript</b></td><td><b>Kotlin</b></td><td><b>Rust</b></td></tr>
<tr>
<td valign="top">

<details><summary>Python Quick Start</summary>

```python
async def process_checkout_card(
    connector_name: str,
    credentials: dict
) -> str:
    """
    Process a card payment with authorize + capture flow.
    
    Args:
        connector_name: Name of the connector (stripe, adyen, etc.)
        credentials: Connector-specific credentials
    
    Returns:
        "success" | "pending" | "failed"
    """
    print(f"\n{'='*60}")
    print(f"Processing Card Payment via {connector_name}")
    print(f"{'='*60}")
    
    # Load probe data for this connector
    probe_data = load_probe_data(connector_name)
    
    # Get configuration
    config = get_connector_config(connector_name, credentials)
    client = PaymentClient(config)
    
    # Build authorize request
    auth_request_dict = build_authorize_request(probe_data, capture_method="MANUAL")
    auth_request = ParseDict(auth_request_dict, payment_pb2.PaymentServiceAuthorizeRequest())
    
    print(f"\nRequest: {json.dumps(auth_request_dict, indent=2)}")
    
    # Step 1: Authorize
    print("\n[1/2] Authorizing...")
    auth_response = await client.authorize(auth_request)
    
    print(f"Status: {auth_response.status}")
    print(f"Connector Transaction ID: {auth_response.connector_transaction_id}")
    
    # Handle status
    if auth_response.status == "FAILED":
        print(f"❌ Payment declined: {auth_response.error_message}")
        return "failed"
    
    if auth_response.status == "PENDING":
        print("⏳ Payment pending - awaiting async confirmation")
        print("   (In production: wait for webhook, then poll get())")
        return "pending"
    
    if auth_response.status != "AUTHORIZED":
        print(f"⚠️  Unexpected status: {auth_response.status}")
        return "error"
    
    print("✅ Funds reserved successfully")
    
    # Step 2: Capture
    print("\n[2/2] Capturing...")
    capture_request_dict = build_capture_request(
        connector_transaction_id=auth_response.connector_transaction_id,
        amount=auth_request_dict["amount"],
    )
    capture_request = ParseDict(capture_request_dict, payment_pb2.PaymentServiceCaptureRequest())
    
    capture_response = await client.capture(capture_request)
    print(f"Capture Status: {capture_response.status}")
    
    if capture_response.status in ["CAPTURED", "AUTHORIZED"]:
        print("✅ Payment captured successfully")
        return "success"
    else:
        print(f"⚠️  Capture returned: {capture_response.status}")
        return "partial"
```

</details>

</td>
<td valign="top">

<details><summary>JavaScript Quick Start</summary>

```javascript
async function processCheckoutCard(connectorName, credentials) {
    console.log('\n' + '='.repeat(60));
    console.log(`Processing Card Payment via ${connectorName}`);
    console.log('='.repeat(60));
    
    // Load probe data for this connector
    const probeData = loadProbeData(connectorName);
    
    // Get configuration
    const config = getConnectorConfig(connectorName, credentials);
    const client = new PaymentClient(config);
    
    // Build authorize request
    const authRequest = buildAuthorizeRequest(probeData, 'MANUAL');
    
    console.log('\nRequest:', JSON.stringify(authRequest, null, 2));
    
    // Step 1: Authorize
    console.log('\n[1/2] Authorizing...');
    const authResponse = await client.authorize(authRequest);
    
    console.log(`Status: ${authResponse.status}`);
    console.log(`Connector Transaction ID: ${authResponse.connectorTransactionId}`);
    
    // Handle status
    if (authResponse.status === 'FAILED') {
        console.log(`❌ Payment declined: ${authResponse.errorMessage || 'Unknown error'}`);
        return 'failed';
    }
    
    if (authResponse.status === 'PENDING') {
        console.log('⏳ Payment pending - awaiting async confirmation');
        console.log('   (In production: wait for webhook, then poll get())');
        return 'pending';
    }
    
    if (authResponse.status !== 'AUTHORIZED') {
        console.log(`⚠️  Unexpected status: ${authResponse.status}`);
        return 'error';
    }
    
    console.log('✅ Funds reserved successfully');
    
    // Step 2: Capture
    console.log('\n[2/2] Capturing...');
    const captureRequest = buildCaptureRequest(
        authResponse.connectorTransactionId,
        authRequest.amount
    );
    
    const captureResponse = await client.capture(captureRequest);
    console.log(`Capture Status: ${captureResponse.status}`);
    
    if (captureResponse.status === 'CAPTURED' || captureResponse.status === 'AUTHORIZED') {
        console.log('✅ Payment captured successfully');
        return 'success';
    } else {
        console.log(`⚠️  Capture returned: ${captureResponse.status}`);
        return 'partial';
    }
}
```

</details>

</td>
<td valign="top">

<details><summary>Kotlin Quick Start</summary>

```kotlin
suspend fun processCheckoutCard(
    connectorName: String,
    credentials: Map<String, String>
): String {
    println("\n${"=".repeat(60)}")
    println("Processing Card Payment via $connectorName")
    println("${"=".repeat(60)}")
    
    // Load probe data for this connector
    val probeData = loadProbeData(connectorName)
    
    // Build config
    val config = getConnectorConfig(connectorName, credentials)
    val client = PaymentClient(config)
    
    // Build request from probe data
    val authRequest = buildAuthorizeRequest(probeData, CaptureMethod.MANUAL)
    
    println("\nRequest: ${Gson().toJson(authRequest)}")
    
    // Step 1: Authorize
    println("\n[1/2] Authorizing...")
    val authResponse = client.authorize(authRequest)
    
    println("Status: ${authResponse.status}")
    println("Connector Transaction ID: ${authResponse.connectorTransactionId}")
    
    // Handle status
    return when (authResponse.status) {
        payments.PaymentStatus.FAILURE -> {
            println("❌ Payment declined: ${authResponse.error?.message ?: "Unknown error"}")
            "failed"
        }
        payments.PaymentStatus.PENDING -> {
            println("⏳ Payment pending - awaiting async confirmation")
            println("   (In production: wait for webhook, then poll get())")
            "pending"
        }
        payments.PaymentStatus.AUTHORIZED -> {
            println("✅ Funds reserved successfully")
            
            // Step 2: Capture
            println("\n[2/2] Capturing...")
            val captureRequest = buildCaptureRequest(
                authResponse.connectorTransactionId,
                authResponse.amount
            )
            
            val captureResponse = client.capture(captureRequest)
            println("Capture Status: ${captureResponse.status}")
            
            when (captureResponse.status) {
                payments.PaymentStatus.CAPTURED, payments.PaymentStatus.AUTHORIZED -> {
                    println("✅ Payment captured successfully")
                    "success"
                }
                else -> {
                    println("⚠️  Capture returned: ${captureResponse.status}")
                    "partial"
                }
            }
        }
        else -> {
            println("⚠️  Unexpected status: ${authResponse.status}")
            "error"
        }
    }
}
```

</details>

</td>
<td valign="top">

<details><summary>Rust Quick Start</summary>

```rust
pub async fn process_checkout_card(
    connector_name: &str,
    credentials: &HashMap<String, String>,
) -> Result<String, Box<dyn Error>> {
    println!("\n{}", "=".repeat(60));
    println!("Processing Card Payment via {}", connector_name);
    println!("{}", "=".repeat(60));
    
    // Load connector probe data
    let probe_data = load_probe_data(connector_name)?;
    
    // Build config
    let config = get_connector_config(connector_name, credentials)?;
    let client = ConnectorClient::new(config, None)?;
    
    // Build request from probe data
    let auth_request = build_authorize_request(&probe_data, "MANUAL")?;
    
    // Step 1: Authorize
    println!("\n[1/2] Authorizing...");
    let auth_response = client.authorize(auth_request, &HashMap::new(), None).await?;
    
    let status = auth_response.status();
    println!("Status: {:?}", status);
    println!("Connector Transaction ID: {:?}", auth_response.connector_transaction_id);
    
    // Handle status
    match status {
        PaymentStatus::Failure | PaymentStatus::AuthorizationFailed => {
            println!("❌ Payment declined");
            return Ok("failed".to_string());
        }
        PaymentStatus::Pending => {
            println!("⏳ Payment pending - awaiting async confirmation");
            println!("   (In production: wait for webhook, then poll get())");
            return Ok("pending".to_string());
        }
        PaymentStatus::Authorized => {
            println!("✅ Funds reserved successfully");
        }
        _ => {
            println!("⚠️  Unexpected status: {:?}", status);
            return Ok("error".to_string());
        }
    }
    
    // Step 2: Capture
    println!("\n[2/2] Capturing...");
    let capture_request = build_capture_request(
        auth_response.connector_transaction_id.as_deref().unwrap_or(""),
        auth_response.amount.as_ref().unwrap()
    );
    
    let capture_response = client.capture(capture_request, &HashMap::new(), None).await?;
    let capture_status = capture_response.status();
    println!("Capture Status: {:?}", capture_status);
    
    match capture_status {
        PaymentStatus::Captured | PaymentStatus::Authorized => {
            println!("✅ Payment captured successfully");
            Ok("success".to_string())
        }
        _ => {
            println!("⚠️  Capture returned: {:?}", capture_status);
            Ok("partial".to_string())
        }
    }
}
```

</details>

</td>
</tr>
</table>

## Supported Connectors


This scenario is supported by the following connectors:

| Connector | Status |
|-----------|--------|
| **ACI** | ✅ Supported |
| **Adyen** | ✅ Supported |
| **Airwallex** | ✅ Supported |
| **authipay** | ✅ Supported |
| **Authorize.net** | ✅ Supported |
| **Bambora** | ✅ Supported |
| **Bambora APAC** | ✅ Supported |
| **Bank of America** | ✅ Supported |
| **Barclaycard** | ✅ Supported |
| **Billwerk** | ✅ Supported |
| **BlueSnap** | ✅ Supported |
| **Braintree** | ✅ Supported |
| **Celero** | ✅ Supported |
| **Checkout.com** | ✅ Supported |
| **CyberSource** | ✅ Supported |
| **Datatrans** | ✅ Supported |
| **dLocal** | ✅ Supported |
| **Elavon** | ✅ Supported |
| **Finix** | ✅ Supported |
| **Fiserv** | ✅ Supported |
| **Fiserv EMEA** | ✅ Supported |
| **Fiuu** | ✅ Supported |
| **Getnet** | ✅ Supported |
| **Global Payments** | ✅ Supported |
| **HiPay** | ✅ Supported |
| **J.P. Morgan** | ✅ Supported |
| **NMI** | ✅ Supported |
| **Noon** | ✅ Supported |
| **Novalnet** | ✅ Supported |
| **Nuvei** | ✅ Supported |
| **Paybox** | ✅ Supported |
| **PayPal** | ✅ Supported |
| **Paysafe** | ✅ Supported |
| **PlacetoPay** | ✅ Supported |
| **PowerTranz** | ✅ Supported |
| **Rapyd** | ✅ Supported |
| **Razorpay** | ✅ Supported |
| **Revolut** | ✅ Supported |
| **Revolv3** | ✅ Supported |
| **Shift4** | ✅ Supported |
| **Silverflow** | ✅ Supported |
| **Stax** | ✅ Supported |
| **Stripe** | ✅ Supported |
| **Trust Payments** | ✅ Supported |
| **TSYS** | ✅ Supported |
| **Wells Fargo** | ✅ Supported |
| **Worldpay** | ✅ Supported |
| **Worldpay Vantiv** | ✅ Supported |
| **Worldpay XML** | ✅ Supported |
| **Xendit** | ✅ Supported |
| **Zift** | ✅ Supported |





## Status Handling


### Authorize Flow

| Status | Recommended Action |
|--------|-------------------|
| `AUTHORIZED` | Funds reserved — proceed to Capture to settle |
| `PENDING` | Awaiting async confirmation — wait for webhook before capturing |
| `FAILED` | Payment declined — surface error to customer, do not retry without new details |



### Capture Flow

| Status | Recommended Action |
|--------|-------------------|
| `CAPTURED` | Funds settled successfully — payment complete |
| `PENDING` | Settlement processing — await webhook confirmation |
| `FAILED` | Capture failed — check error details |





## Connector Implementations

Complete, runnable examples for each connector:

### Python


<details>
<summary><b>ACI</b></summary>

**Configuration:**
```python
# Configuration not available
```

**Full Example:**
```python
async def process_checkout_card(
    connector_name: str,
    credentials: dict
) -> str:
    """
    Process a card payment with authorize + capture flow.
    
    Args:
        connector_name: Name of the connector (stripe, adyen, etc.)
        credentials: Connector-specific credentials
    
    Returns:
        "success" | "pending" | "failed"
    """
    print(f"\n{'='*60}")
    print(f"Processing Card Payment via {connector_name}")
    print(f"{'='*60}")
    
    # Load probe data for this connector
    probe_data = load_probe_data(connector_name)
    
    # Get configuration
    config = get_connector_config(connector_name, credentials)
    client = PaymentClient(config)
    
    # Build authorize request
    auth_request_dict = build_authorize_request(probe_data, capture_method="MANUAL")
    auth_request = ParseDict(auth_request_dict, payment_pb2.PaymentServiceAuthorizeRequest())
    
    print(f"\nRequest: {json.dumps(auth_request_dict, indent=2)}")
    
    # Step 1: Authorize
    print("\n[1/2] Authorizing...")
    auth_response = await client.authorize(auth_request)
    
    print(f"Status: {auth_response.status}")
    print(f"Connector Transaction ID: {auth_response.connector_transaction_id}")
    
    # Handle status
    if auth_response.status == "FAILED":
        print(f"❌ Payment declined: {auth_response.error_message}")
        return "failed"
    
    if auth_response.status == "PENDING":
        print("⏳ Payment pending - awaiting async confirmation")
        print("   (In production: wait for webhook, then poll get())")
        return "pending"
    
    if auth_response.status != "AUTHORIZED":
        print(f"⚠️  Unexpected status: {auth_response.status}")
        return "error"
    
    print("✅ Funds reserved successfully")
    
    # Step 2: Capture
    print("\n[2/2] Capturing...")
    capture_request_dict = build_capture_request(
        connector_transaction_id=auth_response.connector_transaction_id,
        amount=auth_request_dict["amount"],
    )
    capture_request = ParseDict(capture_request_dict, payment_pb2.PaymentServiceCaptureRequest())
    
    capture_response = await client.capture(capture_request)
    print(f"Capture Status: {capture_response.status}")
    
    if capture_response.status in ["CAPTURED", "AUTHORIZED"]:
        print("✅ Payment captured successfully")
        return "success"
    else:
        print(f"⚠️  Capture returned: {capture_response.status}")
        return "partial"
```

</details>


<details>
<summary><b>Adyen</b></summary>

**Configuration:**
```python
def get_adyen_config(api_key: str, merchant_account: str) -> sdk_config_pb2.ConnectorConfig:
    """Configuration for Adyen."""
    config = sdk_config_pb2.ConnectorConfig(
        options=sdk_config_pb2.SdkOptions(environment=sdk_config_pb2.Environment.SANDBOX),
    )
    config.connector_config.CopyFrom(payment_pb2.ConnectorSpecificConfig(
        adyen=payment_pb2.AdyenConfig(
            api_key=api_key,
            merchant_account=merchant_account,
        ),
    ))
    return config
```

**Full Example:**
```python
async def process_checkout_card(
    connector_name: str,
    credentials: dict
) -> str:
    """
    Process a card payment with authorize + capture flow.
    
    Args:
        connector_name: Name of the connector (stripe, adyen, etc.)
        credentials: Connector-specific credentials
    
    Returns:
        "success" | "pending" | "failed"
    """
    print(f"\n{'='*60}")
    print(f"Processing Card Payment via {connector_name}")
    print(f"{'='*60}")
    
    # Load probe data for this connector
    probe_data = load_probe_data(connector_name)
    
    # Get configuration
    config = get_connector_config(connector_name, credentials)
    client = PaymentClient(config)
    
    # Build authorize request
    auth_request_dict = build_authorize_request(probe_data, capture_method="MANUAL")
    auth_request = ParseDict(auth_request_dict, payment_pb2.PaymentServiceAuthorizeRequest())
    
    print(f"\nRequest: {json.dumps(auth_request_dict, indent=2)}")
    
    # Step 1: Authorize
    print("\n[1/2] Authorizing...")
    auth_response = await client.authorize(auth_request)
    
    print(f"Status: {auth_response.status}")
    print(f"Connector Transaction ID: {auth_response.connector_transaction_id}")
    
    # Handle status
    if auth_response.status == "FAILED":
        print(f"❌ Payment declined: {auth_response.error_message}")
        return "failed"
    
    if auth_response.status == "PENDING":
        print("⏳ Payment pending - awaiting async confirmation")
        print("   (In production: wait for webhook, then poll get())")
        return "pending"
    
    if auth_response.status != "AUTHORIZED":
        print(f"⚠️  Unexpected status: {auth_response.status}")
        return "error"
    
    print("✅ Funds reserved successfully")
    
    # Step 2: Capture
    print("\n[2/2] Capturing...")
    capture_request_dict = build_capture_request(
        connector_transaction_id=auth_response.connector_transaction_id,
        amount=auth_request_dict["amount"],
    )
    capture_request = ParseDict(capture_request_dict, payment_pb2.PaymentServiceCaptureRequest())
    
    capture_response = await client.capture(capture_request)
    print(f"Capture Status: {capture_response.status}")
    
    if capture_response.status in ["CAPTURED", "AUTHORIZED"]:
        print("✅ Payment captured successfully")
        return "success"
    else:
        print(f"⚠️  Capture returned: {capture_response.status}")
        return "partial"
```

</details>


<details>
<summary><b>Airwallex</b></summary>

**Configuration:**
```python
# Configuration not available
```

**Full Example:**
```python
async def process_checkout_card(
    connector_name: str,
    credentials: dict
) -> str:
    """
    Process a card payment with authorize + capture flow.
    
    Args:
        connector_name: Name of the connector (stripe, adyen, etc.)
        credentials: Connector-specific credentials
    
    Returns:
        "success" | "pending" | "failed"
    """
    print(f"\n{'='*60}")
    print(f"Processing Card Payment via {connector_name}")
    print(f"{'='*60}")
    
    # Load probe data for this connector
    probe_data = load_probe_data(connector_name)
    
    # Get configuration
    config = get_connector_config(connector_name, credentials)
    client = PaymentClient(config)
    
    # Build authorize request
    auth_request_dict = build_authorize_request(probe_data, capture_method="MANUAL")
    auth_request = ParseDict(auth_request_dict, payment_pb2.PaymentServiceAuthorizeRequest())
    
    print(f"\nRequest: {json.dumps(auth_request_dict, indent=2)}")
    
    # Step 1: Authorize
    print("\n[1/2] Authorizing...")
    auth_response = await client.authorize(auth_request)
    
    print(f"Status: {auth_response.status}")
    print(f"Connector Transaction ID: {auth_response.connector_transaction_id}")
    
    # Handle status
    if auth_response.status == "FAILED":
        print(f"❌ Payment declined: {auth_response.error_message}")
        return "failed"
    
    if auth_response.status == "PENDING":
        print("⏳ Payment pending - awaiting async confirmation")
        print("   (In production: wait for webhook, then poll get())")
        return "pending"
    
    if auth_response.status != "AUTHORIZED":
        print(f"⚠️  Unexpected status: {auth_response.status}")
        return "error"
    
    print("✅ Funds reserved successfully")
    
    # Step 2: Capture
    print("\n[2/2] Capturing...")
    capture_request_dict = build_capture_request(
        connector_transaction_id=auth_response.connector_transaction_id,
        amount=auth_request_dict["amount"],
    )
    capture_request = ParseDict(capture_request_dict, payment_pb2.PaymentServiceCaptureRequest())
    
    capture_response = await client.capture(capture_request)
    print(f"Capture Status: {capture_response.status}")
    
    if capture_response.status in ["CAPTURED", "AUTHORIZED"]:
        print("✅ Payment captured successfully")
        return "success"
    else:
        print(f"⚠️  Capture returned: {capture_response.status}")
        return "partial"
```

</details>


<details>
<summary><b>authipay</b></summary>

**Configuration:**
```python
# Configuration not available
```

**Full Example:**
```python
async def process_checkout_card(
    connector_name: str,
    credentials: dict
) -> str:
    """
    Process a card payment with authorize + capture flow.
    
    Args:
        connector_name: Name of the connector (stripe, adyen, etc.)
        credentials: Connector-specific credentials
    
    Returns:
        "success" | "pending" | "failed"
    """
    print(f"\n{'='*60}")
    print(f"Processing Card Payment via {connector_name}")
    print(f"{'='*60}")
    
    # Load probe data for this connector
    probe_data = load_probe_data(connector_name)
    
    # Get configuration
    config = get_connector_config(connector_name, credentials)
    client = PaymentClient(config)
    
    # Build authorize request
    auth_request_dict = build_authorize_request(probe_data, capture_method="MANUAL")
    auth_request = ParseDict(auth_request_dict, payment_pb2.PaymentServiceAuthorizeRequest())
    
    print(f"\nRequest: {json.dumps(auth_request_dict, indent=2)}")
    
    # Step 1: Authorize
    print("\n[1/2] Authorizing...")
    auth_response = await client.authorize(auth_request)
    
    print(f"Status: {auth_response.status}")
    print(f"Connector Transaction ID: {auth_response.connector_transaction_id}")
    
    # Handle status
    if auth_response.status == "FAILED":
        print(f"❌ Payment declined: {auth_response.error_message}")
        return "failed"
    
    if auth_response.status == "PENDING":
        print("⏳ Payment pending - awaiting async confirmation")
        print("   (In production: wait for webhook, then poll get())")
        return "pending"
    
    if auth_response.status != "AUTHORIZED":
        print(f"⚠️  Unexpected status: {auth_response.status}")
        return "error"
    
    print("✅ Funds reserved successfully")
    
    # Step 2: Capture
    print("\n[2/2] Capturing...")
    capture_request_dict = build_capture_request(
        connector_transaction_id=auth_response.connector_transaction_id,
        amount=auth_request_dict["amount"],
    )
    capture_request = ParseDict(capture_request_dict, payment_pb2.PaymentServiceCaptureRequest())
    
    capture_response = await client.capture(capture_request)
    print(f"Capture Status: {capture_response.status}")
    
    if capture_response.status in ["CAPTURED", "AUTHORIZED"]:
        print("✅ Payment captured successfully")
        return "success"
    else:
        print(f"⚠️  Capture returned: {capture_response.status}")
        return "partial"
```

</details>


<details>
<summary><b>Authorize.net</b></summary>

**Configuration:**
```python
# Configuration not available
```

**Full Example:**
```python
async def process_checkout_card(
    connector_name: str,
    credentials: dict
) -> str:
    """
    Process a card payment with authorize + capture flow.
    
    Args:
        connector_name: Name of the connector (stripe, adyen, etc.)
        credentials: Connector-specific credentials
    
    Returns:
        "success" | "pending" | "failed"
    """
    print(f"\n{'='*60}")
    print(f"Processing Card Payment via {connector_name}")
    print(f"{'='*60}")
    
    # Load probe data for this connector
    probe_data = load_probe_data(connector_name)
    
    # Get configuration
    config = get_connector_config(connector_name, credentials)
    client = PaymentClient(config)
    
    # Build authorize request
    auth_request_dict = build_authorize_request(probe_data, capture_method="MANUAL")
    auth_request = ParseDict(auth_request_dict, payment_pb2.PaymentServiceAuthorizeRequest())
    
    print(f"\nRequest: {json.dumps(auth_request_dict, indent=2)}")
    
    # Step 1: Authorize
    print("\n[1/2] Authorizing...")
    auth_response = await client.authorize(auth_request)
    
    print(f"Status: {auth_response.status}")
    print(f"Connector Transaction ID: {auth_response.connector_transaction_id}")
    
    # Handle status
    if auth_response.status == "FAILED":
        print(f"❌ Payment declined: {auth_response.error_message}")
        return "failed"
    
    if auth_response.status == "PENDING":
        print("⏳ Payment pending - awaiting async confirmation")
        print("   (In production: wait for webhook, then poll get())")
        return "pending"
    
    if auth_response.status != "AUTHORIZED":
        print(f"⚠️  Unexpected status: {auth_response.status}")
        return "error"
    
    print("✅ Funds reserved successfully")
    
    # Step 2: Capture
    print("\n[2/2] Capturing...")
    capture_request_dict = build_capture_request(
        connector_transaction_id=auth_response.connector_transaction_id,
        amount=auth_request_dict["amount"],
    )
    capture_request = ParseDict(capture_request_dict, payment_pb2.PaymentServiceCaptureRequest())
    
    capture_response = await client.capture(capture_request)
    print(f"Capture Status: {capture_response.status}")
    
    if capture_response.status in ["CAPTURED", "AUTHORIZED"]:
        print("✅ Payment captured successfully")
        return "success"
    else:
        print(f"⚠️  Capture returned: {capture_response.status}")
        return "partial"
```

</details>


<details>
<summary><b>Bambora</b></summary>

**Configuration:**
```python
# Configuration not available
```

**Full Example:**
```python
async def process_checkout_card(
    connector_name: str,
    credentials: dict
) -> str:
    """
    Process a card payment with authorize + capture flow.
    
    Args:
        connector_name: Name of the connector (stripe, adyen, etc.)
        credentials: Connector-specific credentials
    
    Returns:
        "success" | "pending" | "failed"
    """
    print(f"\n{'='*60}")
    print(f"Processing Card Payment via {connector_name}")
    print(f"{'='*60}")
    
    # Load probe data for this connector
    probe_data = load_probe_data(connector_name)
    
    # Get configuration
    config = get_connector_config(connector_name, credentials)
    client = PaymentClient(config)
    
    # Build authorize request
    auth_request_dict = build_authorize_request(probe_data, capture_method="MANUAL")
    auth_request = ParseDict(auth_request_dict, payment_pb2.PaymentServiceAuthorizeRequest())
    
    print(f"\nRequest: {json.dumps(auth_request_dict, indent=2)}")
    
    # Step 1: Authorize
    print("\n[1/2] Authorizing...")
    auth_response = await client.authorize(auth_request)
    
    print(f"Status: {auth_response.status}")
    print(f"Connector Transaction ID: {auth_response.connector_transaction_id}")
    
    # Handle status
    if auth_response.status == "FAILED":
        print(f"❌ Payment declined: {auth_response.error_message}")
        return "failed"
    
    if auth_response.status == "PENDING":
        print("⏳ Payment pending - awaiting async confirmation")
        print("   (In production: wait for webhook, then poll get())")
        return "pending"
    
    if auth_response.status != "AUTHORIZED":
        print(f"⚠️  Unexpected status: {auth_response.status}")
        return "error"
    
    print("✅ Funds reserved successfully")
    
    # Step 2: Capture
    print("\n[2/2] Capturing...")
    capture_request_dict = build_capture_request(
        connector_transaction_id=auth_response.connector_transaction_id,
        amount=auth_request_dict["amount"],
    )
    capture_request = ParseDict(capture_request_dict, payment_pb2.PaymentServiceCaptureRequest())
    
    capture_response = await client.capture(capture_request)
    print(f"Capture Status: {capture_response.status}")
    
    if capture_response.status in ["CAPTURED", "AUTHORIZED"]:
        print("✅ Payment captured successfully")
        return "success"
    else:
        print(f"⚠️  Capture returned: {capture_response.status}")
        return "partial"
```

</details>


<details>
<summary><b>Bambora APAC</b></summary>

**Configuration:**
```python
# Configuration not available
```

**Full Example:**
```python
async def process_checkout_card(
    connector_name: str,
    credentials: dict
) -> str:
    """
    Process a card payment with authorize + capture flow.
    
    Args:
        connector_name: Name of the connector (stripe, adyen, etc.)
        credentials: Connector-specific credentials
    
    Returns:
        "success" | "pending" | "failed"
    """
    print(f"\n{'='*60}")
    print(f"Processing Card Payment via {connector_name}")
    print(f"{'='*60}")
    
    # Load probe data for this connector
    probe_data = load_probe_data(connector_name)
    
    # Get configuration
    config = get_connector_config(connector_name, credentials)
    client = PaymentClient(config)
    
    # Build authorize request
    auth_request_dict = build_authorize_request(probe_data, capture_method="MANUAL")
    auth_request = ParseDict(auth_request_dict, payment_pb2.PaymentServiceAuthorizeRequest())
    
    print(f"\nRequest: {json.dumps(auth_request_dict, indent=2)}")
    
    # Step 1: Authorize
    print("\n[1/2] Authorizing...")
    auth_response = await client.authorize(auth_request)
    
    print(f"Status: {auth_response.status}")
    print(f"Connector Transaction ID: {auth_response.connector_transaction_id}")
    
    # Handle status
    if auth_response.status == "FAILED":
        print(f"❌ Payment declined: {auth_response.error_message}")
        return "failed"
    
    if auth_response.status == "PENDING":
        print("⏳ Payment pending - awaiting async confirmation")
        print("   (In production: wait for webhook, then poll get())")
        return "pending"
    
    if auth_response.status != "AUTHORIZED":
        print(f"⚠️  Unexpected status: {auth_response.status}")
        return "error"
    
    print("✅ Funds reserved successfully")
    
    # Step 2: Capture
    print("\n[2/2] Capturing...")
    capture_request_dict = build_capture_request(
        connector_transaction_id=auth_response.connector_transaction_id,
        amount=auth_request_dict["amount"],
    )
    capture_request = ParseDict(capture_request_dict, payment_pb2.PaymentServiceCaptureRequest())
    
    capture_response = await client.capture(capture_request)
    print(f"Capture Status: {capture_response.status}")
    
    if capture_response.status in ["CAPTURED", "AUTHORIZED"]:
        print("✅ Payment captured successfully")
        return "success"
    else:
        print(f"⚠️  Capture returned: {capture_response.status}")
        return "partial"
```

</details>


<details>
<summary><b>Bank of America</b></summary>

**Configuration:**
```python
# Configuration not available
```

**Full Example:**
```python
async def process_checkout_card(
    connector_name: str,
    credentials: dict
) -> str:
    """
    Process a card payment with authorize + capture flow.
    
    Args:
        connector_name: Name of the connector (stripe, adyen, etc.)
        credentials: Connector-specific credentials
    
    Returns:
        "success" | "pending" | "failed"
    """
    print(f"\n{'='*60}")
    print(f"Processing Card Payment via {connector_name}")
    print(f"{'='*60}")
    
    # Load probe data for this connector
    probe_data = load_probe_data(connector_name)
    
    # Get configuration
    config = get_connector_config(connector_name, credentials)
    client = PaymentClient(config)
    
    # Build authorize request
    auth_request_dict = build_authorize_request(probe_data, capture_method="MANUAL")
    auth_request = ParseDict(auth_request_dict, payment_pb2.PaymentServiceAuthorizeRequest())
    
    print(f"\nRequest: {json.dumps(auth_request_dict, indent=2)}")
    
    # Step 1: Authorize
    print("\n[1/2] Authorizing...")
    auth_response = await client.authorize(auth_request)
    
    print(f"Status: {auth_response.status}")
    print(f"Connector Transaction ID: {auth_response.connector_transaction_id}")
    
    # Handle status
    if auth_response.status == "FAILED":
        print(f"❌ Payment declined: {auth_response.error_message}")
        return "failed"
    
    if auth_response.status == "PENDING":
        print("⏳ Payment pending - awaiting async confirmation")
        print("   (In production: wait for webhook, then poll get())")
        return "pending"
    
    if auth_response.status != "AUTHORIZED":
        print(f"⚠️  Unexpected status: {auth_response.status}")
        return "error"
    
    print("✅ Funds reserved successfully")
    
    # Step 2: Capture
    print("\n[2/2] Capturing...")
    capture_request_dict = build_capture_request(
        connector_transaction_id=auth_response.connector_transaction_id,
        amount=auth_request_dict["amount"],
    )
    capture_request = ParseDict(capture_request_dict, payment_pb2.PaymentServiceCaptureRequest())
    
    capture_response = await client.capture(capture_request)
    print(f"Capture Status: {capture_response.status}")
    
    if capture_response.status in ["CAPTURED", "AUTHORIZED"]:
        print("✅ Payment captured successfully")
        return "success"
    else:
        print(f"⚠️  Capture returned: {capture_response.status}")
        return "partial"
```

</details>


<details>
<summary><b>Barclaycard</b></summary>

**Configuration:**
```python
# Configuration not available
```

**Full Example:**
```python
async def process_checkout_card(
    connector_name: str,
    credentials: dict
) -> str:
    """
    Process a card payment with authorize + capture flow.
    
    Args:
        connector_name: Name of the connector (stripe, adyen, etc.)
        credentials: Connector-specific credentials
    
    Returns:
        "success" | "pending" | "failed"
    """
    print(f"\n{'='*60}")
    print(f"Processing Card Payment via {connector_name}")
    print(f"{'='*60}")
    
    # Load probe data for this connector
    probe_data = load_probe_data(connector_name)
    
    # Get configuration
    config = get_connector_config(connector_name, credentials)
    client = PaymentClient(config)
    
    # Build authorize request
    auth_request_dict = build_authorize_request(probe_data, capture_method="MANUAL")
    auth_request = ParseDict(auth_request_dict, payment_pb2.PaymentServiceAuthorizeRequest())
    
    print(f"\nRequest: {json.dumps(auth_request_dict, indent=2)}")
    
    # Step 1: Authorize
    print("\n[1/2] Authorizing...")
    auth_response = await client.authorize(auth_request)
    
    print(f"Status: {auth_response.status}")
    print(f"Connector Transaction ID: {auth_response.connector_transaction_id}")
    
    # Handle status
    if auth_response.status == "FAILED":
        print(f"❌ Payment declined: {auth_response.error_message}")
        return "failed"
    
    if auth_response.status == "PENDING":
        print("⏳ Payment pending - awaiting async confirmation")
        print("   (In production: wait for webhook, then poll get())")
        return "pending"
    
    if auth_response.status != "AUTHORIZED":
        print(f"⚠️  Unexpected status: {auth_response.status}")
        return "error"
    
    print("✅ Funds reserved successfully")
    
    # Step 2: Capture
    print("\n[2/2] Capturing...")
    capture_request_dict = build_capture_request(
        connector_transaction_id=auth_response.connector_transaction_id,
        amount=auth_request_dict["amount"],
    )
    capture_request = ParseDict(capture_request_dict, payment_pb2.PaymentServiceCaptureRequest())
    
    capture_response = await client.capture(capture_request)
    print(f"Capture Status: {capture_response.status}")
    
    if capture_response.status in ["CAPTURED", "AUTHORIZED"]:
        print("✅ Payment captured successfully")
        return "success"
    else:
        print(f"⚠️  Capture returned: {capture_response.status}")
        return "partial"
```

</details>


<details>
<summary><b>Billwerk</b></summary>

**Configuration:**
```python
# Configuration not available
```

**Full Example:**
```python
async def process_checkout_card(
    connector_name: str,
    credentials: dict
) -> str:
    """
    Process a card payment with authorize + capture flow.
    
    Args:
        connector_name: Name of the connector (stripe, adyen, etc.)
        credentials: Connector-specific credentials
    
    Returns:
        "success" | "pending" | "failed"
    """
    print(f"\n{'='*60}")
    print(f"Processing Card Payment via {connector_name}")
    print(f"{'='*60}")
    
    # Load probe data for this connector
    probe_data = load_probe_data(connector_name)
    
    # Get configuration
    config = get_connector_config(connector_name, credentials)
    client = PaymentClient(config)
    
    # Build authorize request
    auth_request_dict = build_authorize_request(probe_data, capture_method="MANUAL")
    auth_request = ParseDict(auth_request_dict, payment_pb2.PaymentServiceAuthorizeRequest())
    
    print(f"\nRequest: {json.dumps(auth_request_dict, indent=2)}")
    
    # Step 1: Authorize
    print("\n[1/2] Authorizing...")
    auth_response = await client.authorize(auth_request)
    
    print(f"Status: {auth_response.status}")
    print(f"Connector Transaction ID: {auth_response.connector_transaction_id}")
    
    # Handle status
    if auth_response.status == "FAILED":
        print(f"❌ Payment declined: {auth_response.error_message}")
        return "failed"
    
    if auth_response.status == "PENDING":
        print("⏳ Payment pending - awaiting async confirmation")
        print("   (In production: wait for webhook, then poll get())")
        return "pending"
    
    if auth_response.status != "AUTHORIZED":
        print(f"⚠️  Unexpected status: {auth_response.status}")
        return "error"
    
    print("✅ Funds reserved successfully")
    
    # Step 2: Capture
    print("\n[2/2] Capturing...")
    capture_request_dict = build_capture_request(
        connector_transaction_id=auth_response.connector_transaction_id,
        amount=auth_request_dict["amount"],
    )
    capture_request = ParseDict(capture_request_dict, payment_pb2.PaymentServiceCaptureRequest())
    
    capture_response = await client.capture(capture_request)
    print(f"Capture Status: {capture_response.status}")
    
    if capture_response.status in ["CAPTURED", "AUTHORIZED"]:
        print("✅ Payment captured successfully")
        return "success"
    else:
        print(f"⚠️  Capture returned: {capture_response.status}")
        return "partial"
```

</details>


<details>
<summary><b>BlueSnap</b></summary>

**Configuration:**
```python
# Configuration not available
```

**Full Example:**
```python
async def process_checkout_card(
    connector_name: str,
    credentials: dict
) -> str:
    """
    Process a card payment with authorize + capture flow.
    
    Args:
        connector_name: Name of the connector (stripe, adyen, etc.)
        credentials: Connector-specific credentials
    
    Returns:
        "success" | "pending" | "failed"
    """
    print(f"\n{'='*60}")
    print(f"Processing Card Payment via {connector_name}")
    print(f"{'='*60}")
    
    # Load probe data for this connector
    probe_data = load_probe_data(connector_name)
    
    # Get configuration
    config = get_connector_config(connector_name, credentials)
    client = PaymentClient(config)
    
    # Build authorize request
    auth_request_dict = build_authorize_request(probe_data, capture_method="MANUAL")
    auth_request = ParseDict(auth_request_dict, payment_pb2.PaymentServiceAuthorizeRequest())
    
    print(f"\nRequest: {json.dumps(auth_request_dict, indent=2)}")
    
    # Step 1: Authorize
    print("\n[1/2] Authorizing...")
    auth_response = await client.authorize(auth_request)
    
    print(f"Status: {auth_response.status}")
    print(f"Connector Transaction ID: {auth_response.connector_transaction_id}")
    
    # Handle status
    if auth_response.status == "FAILED":
        print(f"❌ Payment declined: {auth_response.error_message}")
        return "failed"
    
    if auth_response.status == "PENDING":
        print("⏳ Payment pending - awaiting async confirmation")
        print("   (In production: wait for webhook, then poll get())")
        return "pending"
    
    if auth_response.status != "AUTHORIZED":
        print(f"⚠️  Unexpected status: {auth_response.status}")
        return "error"
    
    print("✅ Funds reserved successfully")
    
    # Step 2: Capture
    print("\n[2/2] Capturing...")
    capture_request_dict = build_capture_request(
        connector_transaction_id=auth_response.connector_transaction_id,
        amount=auth_request_dict["amount"],
    )
    capture_request = ParseDict(capture_request_dict, payment_pb2.PaymentServiceCaptureRequest())
    
    capture_response = await client.capture(capture_request)
    print(f"Capture Status: {capture_response.status}")
    
    if capture_response.status in ["CAPTURED", "AUTHORIZED"]:
        print("✅ Payment captured successfully")
        return "success"
    else:
        print(f"⚠️  Capture returned: {capture_response.status}")
        return "partial"
```

</details>


<details>
<summary><b>Braintree</b></summary>

**Configuration:**
```python
# Configuration not available
```

**Full Example:**
```python
async def process_checkout_card(
    connector_name: str,
    credentials: dict
) -> str:
    """
    Process a card payment with authorize + capture flow.
    
    Args:
        connector_name: Name of the connector (stripe, adyen, etc.)
        credentials: Connector-specific credentials
    
    Returns:
        "success" | "pending" | "failed"
    """
    print(f"\n{'='*60}")
    print(f"Processing Card Payment via {connector_name}")
    print(f"{'='*60}")
    
    # Load probe data for this connector
    probe_data = load_probe_data(connector_name)
    
    # Get configuration
    config = get_connector_config(connector_name, credentials)
    client = PaymentClient(config)
    
    # Build authorize request
    auth_request_dict = build_authorize_request(probe_data, capture_method="MANUAL")
    auth_request = ParseDict(auth_request_dict, payment_pb2.PaymentServiceAuthorizeRequest())
    
    print(f"\nRequest: {json.dumps(auth_request_dict, indent=2)}")
    
    # Step 1: Authorize
    print("\n[1/2] Authorizing...")
    auth_response = await client.authorize(auth_request)
    
    print(f"Status: {auth_response.status}")
    print(f"Connector Transaction ID: {auth_response.connector_transaction_id}")
    
    # Handle status
    if auth_response.status == "FAILED":
        print(f"❌ Payment declined: {auth_response.error_message}")
        return "failed"
    
    if auth_response.status == "PENDING":
        print("⏳ Payment pending - awaiting async confirmation")
        print("   (In production: wait for webhook, then poll get())")
        return "pending"
    
    if auth_response.status != "AUTHORIZED":
        print(f"⚠️  Unexpected status: {auth_response.status}")
        return "error"
    
    print("✅ Funds reserved successfully")
    
    # Step 2: Capture
    print("\n[2/2] Capturing...")
    capture_request_dict = build_capture_request(
        connector_transaction_id=auth_response.connector_transaction_id,
        amount=auth_request_dict["amount"],
    )
    capture_request = ParseDict(capture_request_dict, payment_pb2.PaymentServiceCaptureRequest())
    
    capture_response = await client.capture(capture_request)
    print(f"Capture Status: {capture_response.status}")
    
    if capture_response.status in ["CAPTURED", "AUTHORIZED"]:
        print("✅ Payment captured successfully")
        return "success"
    else:
        print(f"⚠️  Capture returned: {capture_response.status}")
        return "partial"
```

</details>


<details>
<summary><b>Celero</b></summary>

**Configuration:**
```python
# Configuration not available
```

**Full Example:**
```python
async def process_checkout_card(
    connector_name: str,
    credentials: dict
) -> str:
    """
    Process a card payment with authorize + capture flow.
    
    Args:
        connector_name: Name of the connector (stripe, adyen, etc.)
        credentials: Connector-specific credentials
    
    Returns:
        "success" | "pending" | "failed"
    """
    print(f"\n{'='*60}")
    print(f"Processing Card Payment via {connector_name}")
    print(f"{'='*60}")
    
    # Load probe data for this connector
    probe_data = load_probe_data(connector_name)
    
    # Get configuration
    config = get_connector_config(connector_name, credentials)
    client = PaymentClient(config)
    
    # Build authorize request
    auth_request_dict = build_authorize_request(probe_data, capture_method="MANUAL")
    auth_request = ParseDict(auth_request_dict, payment_pb2.PaymentServiceAuthorizeRequest())
    
    print(f"\nRequest: {json.dumps(auth_request_dict, indent=2)}")
    
    # Step 1: Authorize
    print("\n[1/2] Authorizing...")
    auth_response = await client.authorize(auth_request)
    
    print(f"Status: {auth_response.status}")
    print(f"Connector Transaction ID: {auth_response.connector_transaction_id}")
    
    # Handle status
    if auth_response.status == "FAILED":
        print(f"❌ Payment declined: {auth_response.error_message}")
        return "failed"
    
    if auth_response.status == "PENDING":
        print("⏳ Payment pending - awaiting async confirmation")
        print("   (In production: wait for webhook, then poll get())")
        return "pending"
    
    if auth_response.status != "AUTHORIZED":
        print(f"⚠️  Unexpected status: {auth_response.status}")
        return "error"
    
    print("✅ Funds reserved successfully")
    
    # Step 2: Capture
    print("\n[2/2] Capturing...")
    capture_request_dict = build_capture_request(
        connector_transaction_id=auth_response.connector_transaction_id,
        amount=auth_request_dict["amount"],
    )
    capture_request = ParseDict(capture_request_dict, payment_pb2.PaymentServiceCaptureRequest())
    
    capture_response = await client.capture(capture_request)
    print(f"Capture Status: {capture_response.status}")
    
    if capture_response.status in ["CAPTURED", "AUTHORIZED"]:
        print("✅ Payment captured successfully")
        return "success"
    else:
        print(f"⚠️  Capture returned: {capture_response.status}")
        return "partial"
```

</details>


<details>
<summary><b>Checkout.com</b></summary>

**Configuration:**
```python
def get_checkout_config(api_key: str) -> sdk_config_pb2.ConnectorConfig:
    """Configuration for Checkout.com."""
    config = sdk_config_pb2.ConnectorConfig(
        options=sdk_config_pb2.SdkOptions(environment=sdk_config_pb2.Environment.SANDBOX),
    )
    config.connector_config.CopyFrom(payment_pb2.ConnectorSpecificConfig(
        checkout=payment_pb2.CheckoutConfig(api_key=api_key),
    ))
    return config
```

**Full Example:**
```python
async def process_checkout_card(
    connector_name: str,
    credentials: dict
) -> str:
    """
    Process a card payment with authorize + capture flow.
    
    Args:
        connector_name: Name of the connector (stripe, adyen, etc.)
        credentials: Connector-specific credentials
    
    Returns:
        "success" | "pending" | "failed"
    """
    print(f"\n{'='*60}")
    print(f"Processing Card Payment via {connector_name}")
    print(f"{'='*60}")
    
    # Load probe data for this connector
    probe_data = load_probe_data(connector_name)
    
    # Get configuration
    config = get_connector_config(connector_name, credentials)
    client = PaymentClient(config)
    
    # Build authorize request
    auth_request_dict = build_authorize_request(probe_data, capture_method="MANUAL")
    auth_request = ParseDict(auth_request_dict, payment_pb2.PaymentServiceAuthorizeRequest())
    
    print(f"\nRequest: {json.dumps(auth_request_dict, indent=2)}")
    
    # Step 1: Authorize
    print("\n[1/2] Authorizing...")
    auth_response = await client.authorize(auth_request)
    
    print(f"Status: {auth_response.status}")
    print(f"Connector Transaction ID: {auth_response.connector_transaction_id}")
    
    # Handle status
    if auth_response.status == "FAILED":
        print(f"❌ Payment declined: {auth_response.error_message}")
        return "failed"
    
    if auth_response.status == "PENDING":
        print("⏳ Payment pending - awaiting async confirmation")
        print("   (In production: wait for webhook, then poll get())")
        return "pending"
    
    if auth_response.status != "AUTHORIZED":
        print(f"⚠️  Unexpected status: {auth_response.status}")
        return "error"
    
    print("✅ Funds reserved successfully")
    
    # Step 2: Capture
    print("\n[2/2] Capturing...")
    capture_request_dict = build_capture_request(
        connector_transaction_id=auth_response.connector_transaction_id,
        amount=auth_request_dict["amount"],
    )
    capture_request = ParseDict(capture_request_dict, payment_pb2.PaymentServiceCaptureRequest())
    
    capture_response = await client.capture(capture_request)
    print(f"Capture Status: {capture_response.status}")
    
    if capture_response.status in ["CAPTURED", "AUTHORIZED"]:
        print("✅ Payment captured successfully")
        return "success"
    else:
        print(f"⚠️  Capture returned: {capture_response.status}")
        return "partial"
```

</details>


<details>
<summary><b>CyberSource</b></summary>

**Configuration:**
```python
# Configuration not available
```

**Full Example:**
```python
async def process_checkout_card(
    connector_name: str,
    credentials: dict
) -> str:
    """
    Process a card payment with authorize + capture flow.
    
    Args:
        connector_name: Name of the connector (stripe, adyen, etc.)
        credentials: Connector-specific credentials
    
    Returns:
        "success" | "pending" | "failed"
    """
    print(f"\n{'='*60}")
    print(f"Processing Card Payment via {connector_name}")
    print(f"{'='*60}")
    
    # Load probe data for this connector
    probe_data = load_probe_data(connector_name)
    
    # Get configuration
    config = get_connector_config(connector_name, credentials)
    client = PaymentClient(config)
    
    # Build authorize request
    auth_request_dict = build_authorize_request(probe_data, capture_method="MANUAL")
    auth_request = ParseDict(auth_request_dict, payment_pb2.PaymentServiceAuthorizeRequest())
    
    print(f"\nRequest: {json.dumps(auth_request_dict, indent=2)}")
    
    # Step 1: Authorize
    print("\n[1/2] Authorizing...")
    auth_response = await client.authorize(auth_request)
    
    print(f"Status: {auth_response.status}")
    print(f"Connector Transaction ID: {auth_response.connector_transaction_id}")
    
    # Handle status
    if auth_response.status == "FAILED":
        print(f"❌ Payment declined: {auth_response.error_message}")
        return "failed"
    
    if auth_response.status == "PENDING":
        print("⏳ Payment pending - awaiting async confirmation")
        print("   (In production: wait for webhook, then poll get())")
        return "pending"
    
    if auth_response.status != "AUTHORIZED":
        print(f"⚠️  Unexpected status: {auth_response.status}")
        return "error"
    
    print("✅ Funds reserved successfully")
    
    # Step 2: Capture
    print("\n[2/2] Capturing...")
    capture_request_dict = build_capture_request(
        connector_transaction_id=auth_response.connector_transaction_id,
        amount=auth_request_dict["amount"],
    )
    capture_request = ParseDict(capture_request_dict, payment_pb2.PaymentServiceCaptureRequest())
    
    capture_response = await client.capture(capture_request)
    print(f"Capture Status: {capture_response.status}")
    
    if capture_response.status in ["CAPTURED", "AUTHORIZED"]:
        print("✅ Payment captured successfully")
        return "success"
    else:
        print(f"⚠️  Capture returned: {capture_response.status}")
        return "partial"
```

</details>


<details>
<summary><b>Datatrans</b></summary>

**Configuration:**
```python
# Configuration not available
```

**Full Example:**
```python
async def process_checkout_card(
    connector_name: str,
    credentials: dict
) -> str:
    """
    Process a card payment with authorize + capture flow.
    
    Args:
        connector_name: Name of the connector (stripe, adyen, etc.)
        credentials: Connector-specific credentials
    
    Returns:
        "success" | "pending" | "failed"
    """
    print(f"\n{'='*60}")
    print(f"Processing Card Payment via {connector_name}")
    print(f"{'='*60}")
    
    # Load probe data for this connector
    probe_data = load_probe_data(connector_name)
    
    # Get configuration
    config = get_connector_config(connector_name, credentials)
    client = PaymentClient(config)
    
    # Build authorize request
    auth_request_dict = build_authorize_request(probe_data, capture_method="MANUAL")
    auth_request = ParseDict(auth_request_dict, payment_pb2.PaymentServiceAuthorizeRequest())
    
    print(f"\nRequest: {json.dumps(auth_request_dict, indent=2)}")
    
    # Step 1: Authorize
    print("\n[1/2] Authorizing...")
    auth_response = await client.authorize(auth_request)
    
    print(f"Status: {auth_response.status}")
    print(f"Connector Transaction ID: {auth_response.connector_transaction_id}")
    
    # Handle status
    if auth_response.status == "FAILED":
        print(f"❌ Payment declined: {auth_response.error_message}")
        return "failed"
    
    if auth_response.status == "PENDING":
        print("⏳ Payment pending - awaiting async confirmation")
        print("   (In production: wait for webhook, then poll get())")
        return "pending"
    
    if auth_response.status != "AUTHORIZED":
        print(f"⚠️  Unexpected status: {auth_response.status}")
        return "error"
    
    print("✅ Funds reserved successfully")
    
    # Step 2: Capture
    print("\n[2/2] Capturing...")
    capture_request_dict = build_capture_request(
        connector_transaction_id=auth_response.connector_transaction_id,
        amount=auth_request_dict["amount"],
    )
    capture_request = ParseDict(capture_request_dict, payment_pb2.PaymentServiceCaptureRequest())
    
    capture_response = await client.capture(capture_request)
    print(f"Capture Status: {capture_response.status}")
    
    if capture_response.status in ["CAPTURED", "AUTHORIZED"]:
        print("✅ Payment captured successfully")
        return "success"
    else:
        print(f"⚠️  Capture returned: {capture_response.status}")
        return "partial"
```

</details>


<details>
<summary><b>dLocal</b></summary>

**Configuration:**
```python
# Configuration not available
```

**Full Example:**
```python
async def process_checkout_card(
    connector_name: str,
    credentials: dict
) -> str:
    """
    Process a card payment with authorize + capture flow.
    
    Args:
        connector_name: Name of the connector (stripe, adyen, etc.)
        credentials: Connector-specific credentials
    
    Returns:
        "success" | "pending" | "failed"
    """
    print(f"\n{'='*60}")
    print(f"Processing Card Payment via {connector_name}")
    print(f"{'='*60}")
    
    # Load probe data for this connector
    probe_data = load_probe_data(connector_name)
    
    # Get configuration
    config = get_connector_config(connector_name, credentials)
    client = PaymentClient(config)
    
    # Build authorize request
    auth_request_dict = build_authorize_request(probe_data, capture_method="MANUAL")
    auth_request = ParseDict(auth_request_dict, payment_pb2.PaymentServiceAuthorizeRequest())
    
    print(f"\nRequest: {json.dumps(auth_request_dict, indent=2)}")
    
    # Step 1: Authorize
    print("\n[1/2] Authorizing...")
    auth_response = await client.authorize(auth_request)
    
    print(f"Status: {auth_response.status}")
    print(f"Connector Transaction ID: {auth_response.connector_transaction_id}")
    
    # Handle status
    if auth_response.status == "FAILED":
        print(f"❌ Payment declined: {auth_response.error_message}")
        return "failed"
    
    if auth_response.status == "PENDING":
        print("⏳ Payment pending - awaiting async confirmation")
        print("   (In production: wait for webhook, then poll get())")
        return "pending"
    
    if auth_response.status != "AUTHORIZED":
        print(f"⚠️  Unexpected status: {auth_response.status}")
        return "error"
    
    print("✅ Funds reserved successfully")
    
    # Step 2: Capture
    print("\n[2/2] Capturing...")
    capture_request_dict = build_capture_request(
        connector_transaction_id=auth_response.connector_transaction_id,
        amount=auth_request_dict["amount"],
    )
    capture_request = ParseDict(capture_request_dict, payment_pb2.PaymentServiceCaptureRequest())
    
    capture_response = await client.capture(capture_request)
    print(f"Capture Status: {capture_response.status}")
    
    if capture_response.status in ["CAPTURED", "AUTHORIZED"]:
        print("✅ Payment captured successfully")
        return "success"
    else:
        print(f"⚠️  Capture returned: {capture_response.status}")
        return "partial"
```

</details>


<details>
<summary><b>Elavon</b></summary>

**Configuration:**
```python
# Configuration not available
```

**Full Example:**
```python
async def process_checkout_card(
    connector_name: str,
    credentials: dict
) -> str:
    """
    Process a card payment with authorize + capture flow.
    
    Args:
        connector_name: Name of the connector (stripe, adyen, etc.)
        credentials: Connector-specific credentials
    
    Returns:
        "success" | "pending" | "failed"
    """
    print(f"\n{'='*60}")
    print(f"Processing Card Payment via {connector_name}")
    print(f"{'='*60}")
    
    # Load probe data for this connector
    probe_data = load_probe_data(connector_name)
    
    # Get configuration
    config = get_connector_config(connector_name, credentials)
    client = PaymentClient(config)
    
    # Build authorize request
    auth_request_dict = build_authorize_request(probe_data, capture_method="MANUAL")
    auth_request = ParseDict(auth_request_dict, payment_pb2.PaymentServiceAuthorizeRequest())
    
    print(f"\nRequest: {json.dumps(auth_request_dict, indent=2)}")
    
    # Step 1: Authorize
    print("\n[1/2] Authorizing...")
    auth_response = await client.authorize(auth_request)
    
    print(f"Status: {auth_response.status}")
    print(f"Connector Transaction ID: {auth_response.connector_transaction_id}")
    
    # Handle status
    if auth_response.status == "FAILED":
        print(f"❌ Payment declined: {auth_response.error_message}")
        return "failed"
    
    if auth_response.status == "PENDING":
        print("⏳ Payment pending - awaiting async confirmation")
        print("   (In production: wait for webhook, then poll get())")
        return "pending"
    
    if auth_response.status != "AUTHORIZED":
        print(f"⚠️  Unexpected status: {auth_response.status}")
        return "error"
    
    print("✅ Funds reserved successfully")
    
    # Step 2: Capture
    print("\n[2/2] Capturing...")
    capture_request_dict = build_capture_request(
        connector_transaction_id=auth_response.connector_transaction_id,
        amount=auth_request_dict["amount"],
    )
    capture_request = ParseDict(capture_request_dict, payment_pb2.PaymentServiceCaptureRequest())
    
    capture_response = await client.capture(capture_request)
    print(f"Capture Status: {capture_response.status}")
    
    if capture_response.status in ["CAPTURED", "AUTHORIZED"]:
        print("✅ Payment captured successfully")
        return "success"
    else:
        print(f"⚠️  Capture returned: {capture_response.status}")
        return "partial"
```

</details>


<details>
<summary><b>Finix</b></summary>

**Configuration:**
```python
# Configuration not available
```

**Full Example:**
```python
async def process_checkout_card(
    connector_name: str,
    credentials: dict
) -> str:
    """
    Process a card payment with authorize + capture flow.
    
    Args:
        connector_name: Name of the connector (stripe, adyen, etc.)
        credentials: Connector-specific credentials
    
    Returns:
        "success" | "pending" | "failed"
    """
    print(f"\n{'='*60}")
    print(f"Processing Card Payment via {connector_name}")
    print(f"{'='*60}")
    
    # Load probe data for this connector
    probe_data = load_probe_data(connector_name)
    
    # Get configuration
    config = get_connector_config(connector_name, credentials)
    client = PaymentClient(config)
    
    # Build authorize request
    auth_request_dict = build_authorize_request(probe_data, capture_method="MANUAL")
    auth_request = ParseDict(auth_request_dict, payment_pb2.PaymentServiceAuthorizeRequest())
    
    print(f"\nRequest: {json.dumps(auth_request_dict, indent=2)}")
    
    # Step 1: Authorize
    print("\n[1/2] Authorizing...")
    auth_response = await client.authorize(auth_request)
    
    print(f"Status: {auth_response.status}")
    print(f"Connector Transaction ID: {auth_response.connector_transaction_id}")
    
    # Handle status
    if auth_response.status == "FAILED":
        print(f"❌ Payment declined: {auth_response.error_message}")
        return "failed"
    
    if auth_response.status == "PENDING":
        print("⏳ Payment pending - awaiting async confirmation")
        print("   (In production: wait for webhook, then poll get())")
        return "pending"
    
    if auth_response.status != "AUTHORIZED":
        print(f"⚠️  Unexpected status: {auth_response.status}")
        return "error"
    
    print("✅ Funds reserved successfully")
    
    # Step 2: Capture
    print("\n[2/2] Capturing...")
    capture_request_dict = build_capture_request(
        connector_transaction_id=auth_response.connector_transaction_id,
        amount=auth_request_dict["amount"],
    )
    capture_request = ParseDict(capture_request_dict, payment_pb2.PaymentServiceCaptureRequest())
    
    capture_response = await client.capture(capture_request)
    print(f"Capture Status: {capture_response.status}")
    
    if capture_response.status in ["CAPTURED", "AUTHORIZED"]:
        print("✅ Payment captured successfully")
        return "success"
    else:
        print(f"⚠️  Capture returned: {capture_response.status}")
        return "partial"
```

</details>


<details>
<summary><b>Fiserv</b></summary>

**Configuration:**
```python
# Configuration not available
```

**Full Example:**
```python
async def process_checkout_card(
    connector_name: str,
    credentials: dict
) -> str:
    """
    Process a card payment with authorize + capture flow.
    
    Args:
        connector_name: Name of the connector (stripe, adyen, etc.)
        credentials: Connector-specific credentials
    
    Returns:
        "success" | "pending" | "failed"
    """
    print(f"\n{'='*60}")
    print(f"Processing Card Payment via {connector_name}")
    print(f"{'='*60}")
    
    # Load probe data for this connector
    probe_data = load_probe_data(connector_name)
    
    # Get configuration
    config = get_connector_config(connector_name, credentials)
    client = PaymentClient(config)
    
    # Build authorize request
    auth_request_dict = build_authorize_request(probe_data, capture_method="MANUAL")
    auth_request = ParseDict(auth_request_dict, payment_pb2.PaymentServiceAuthorizeRequest())
    
    print(f"\nRequest: {json.dumps(auth_request_dict, indent=2)}")
    
    # Step 1: Authorize
    print("\n[1/2] Authorizing...")
    auth_response = await client.authorize(auth_request)
    
    print(f"Status: {auth_response.status}")
    print(f"Connector Transaction ID: {auth_response.connector_transaction_id}")
    
    # Handle status
    if auth_response.status == "FAILED":
        print(f"❌ Payment declined: {auth_response.error_message}")
        return "failed"
    
    if auth_response.status == "PENDING":
        print("⏳ Payment pending - awaiting async confirmation")
        print("   (In production: wait for webhook, then poll get())")
        return "pending"
    
    if auth_response.status != "AUTHORIZED":
        print(f"⚠️  Unexpected status: {auth_response.status}")
        return "error"
    
    print("✅ Funds reserved successfully")
    
    # Step 2: Capture
    print("\n[2/2] Capturing...")
    capture_request_dict = build_capture_request(
        connector_transaction_id=auth_response.connector_transaction_id,
        amount=auth_request_dict["amount"],
    )
    capture_request = ParseDict(capture_request_dict, payment_pb2.PaymentServiceCaptureRequest())
    
    capture_response = await client.capture(capture_request)
    print(f"Capture Status: {capture_response.status}")
    
    if capture_response.status in ["CAPTURED", "AUTHORIZED"]:
        print("✅ Payment captured successfully")
        return "success"
    else:
        print(f"⚠️  Capture returned: {capture_response.status}")
        return "partial"
```

</details>


<details>
<summary><b>Fiserv EMEA</b></summary>

**Configuration:**
```python
# Configuration not available
```

**Full Example:**
```python
async def process_checkout_card(
    connector_name: str,
    credentials: dict
) -> str:
    """
    Process a card payment with authorize + capture flow.
    
    Args:
        connector_name: Name of the connector (stripe, adyen, etc.)
        credentials: Connector-specific credentials
    
    Returns:
        "success" | "pending" | "failed"
    """
    print(f"\n{'='*60}")
    print(f"Processing Card Payment via {connector_name}")
    print(f"{'='*60}")
    
    # Load probe data for this connector
    probe_data = load_probe_data(connector_name)
    
    # Get configuration
    config = get_connector_config(connector_name, credentials)
    client = PaymentClient(config)
    
    # Build authorize request
    auth_request_dict = build_authorize_request(probe_data, capture_method="MANUAL")
    auth_request = ParseDict(auth_request_dict, payment_pb2.PaymentServiceAuthorizeRequest())
    
    print(f"\nRequest: {json.dumps(auth_request_dict, indent=2)}")
    
    # Step 1: Authorize
    print("\n[1/2] Authorizing...")
    auth_response = await client.authorize(auth_request)
    
    print(f"Status: {auth_response.status}")
    print(f"Connector Transaction ID: {auth_response.connector_transaction_id}")
    
    # Handle status
    if auth_response.status == "FAILED":
        print(f"❌ Payment declined: {auth_response.error_message}")
        return "failed"
    
    if auth_response.status == "PENDING":
        print("⏳ Payment pending - awaiting async confirmation")
        print("   (In production: wait for webhook, then poll get())")
        return "pending"
    
    if auth_response.status != "AUTHORIZED":
        print(f"⚠️  Unexpected status: {auth_response.status}")
        return "error"
    
    print("✅ Funds reserved successfully")
    
    # Step 2: Capture
    print("\n[2/2] Capturing...")
    capture_request_dict = build_capture_request(
        connector_transaction_id=auth_response.connector_transaction_id,
        amount=auth_request_dict["amount"],
    )
    capture_request = ParseDict(capture_request_dict, payment_pb2.PaymentServiceCaptureRequest())
    
    capture_response = await client.capture(capture_request)
    print(f"Capture Status: {capture_response.status}")
    
    if capture_response.status in ["CAPTURED", "AUTHORIZED"]:
        print("✅ Payment captured successfully")
        return "success"
    else:
        print(f"⚠️  Capture returned: {capture_response.status}")
        return "partial"
```

</details>


<details>
<summary><b>Fiuu</b></summary>

**Configuration:**
```python
# Configuration not available
```

**Full Example:**
```python
async def process_checkout_card(
    connector_name: str,
    credentials: dict
) -> str:
    """
    Process a card payment with authorize + capture flow.
    
    Args:
        connector_name: Name of the connector (stripe, adyen, etc.)
        credentials: Connector-specific credentials
    
    Returns:
        "success" | "pending" | "failed"
    """
    print(f"\n{'='*60}")
    print(f"Processing Card Payment via {connector_name}")
    print(f"{'='*60}")
    
    # Load probe data for this connector
    probe_data = load_probe_data(connector_name)
    
    # Get configuration
    config = get_connector_config(connector_name, credentials)
    client = PaymentClient(config)
    
    # Build authorize request
    auth_request_dict = build_authorize_request(probe_data, capture_method="MANUAL")
    auth_request = ParseDict(auth_request_dict, payment_pb2.PaymentServiceAuthorizeRequest())
    
    print(f"\nRequest: {json.dumps(auth_request_dict, indent=2)}")
    
    # Step 1: Authorize
    print("\n[1/2] Authorizing...")
    auth_response = await client.authorize(auth_request)
    
    print(f"Status: {auth_response.status}")
    print(f"Connector Transaction ID: {auth_response.connector_transaction_id}")
    
    # Handle status
    if auth_response.status == "FAILED":
        print(f"❌ Payment declined: {auth_response.error_message}")
        return "failed"
    
    if auth_response.status == "PENDING":
        print("⏳ Payment pending - awaiting async confirmation")
        print("   (In production: wait for webhook, then poll get())")
        return "pending"
    
    if auth_response.status != "AUTHORIZED":
        print(f"⚠️  Unexpected status: {auth_response.status}")
        return "error"
    
    print("✅ Funds reserved successfully")
    
    # Step 2: Capture
    print("\n[2/2] Capturing...")
    capture_request_dict = build_capture_request(
        connector_transaction_id=auth_response.connector_transaction_id,
        amount=auth_request_dict["amount"],
    )
    capture_request = ParseDict(capture_request_dict, payment_pb2.PaymentServiceCaptureRequest())
    
    capture_response = await client.capture(capture_request)
    print(f"Capture Status: {capture_response.status}")
    
    if capture_response.status in ["CAPTURED", "AUTHORIZED"]:
        print("✅ Payment captured successfully")
        return "success"
    else:
        print(f"⚠️  Capture returned: {capture_response.status}")
        return "partial"
```

</details>


<details>
<summary><b>Getnet</b></summary>

**Configuration:**
```python
# Configuration not available
```

**Full Example:**
```python
async def process_checkout_card(
    connector_name: str,
    credentials: dict
) -> str:
    """
    Process a card payment with authorize + capture flow.
    
    Args:
        connector_name: Name of the connector (stripe, adyen, etc.)
        credentials: Connector-specific credentials
    
    Returns:
        "success" | "pending" | "failed"
    """
    print(f"\n{'='*60}")
    print(f"Processing Card Payment via {connector_name}")
    print(f"{'='*60}")
    
    # Load probe data for this connector
    probe_data = load_probe_data(connector_name)
    
    # Get configuration
    config = get_connector_config(connector_name, credentials)
    client = PaymentClient(config)
    
    # Build authorize request
    auth_request_dict = build_authorize_request(probe_data, capture_method="MANUAL")
    auth_request = ParseDict(auth_request_dict, payment_pb2.PaymentServiceAuthorizeRequest())
    
    print(f"\nRequest: {json.dumps(auth_request_dict, indent=2)}")
    
    # Step 1: Authorize
    print("\n[1/2] Authorizing...")
    auth_response = await client.authorize(auth_request)
    
    print(f"Status: {auth_response.status}")
    print(f"Connector Transaction ID: {auth_response.connector_transaction_id}")
    
    # Handle status
    if auth_response.status == "FAILED":
        print(f"❌ Payment declined: {auth_response.error_message}")
        return "failed"
    
    if auth_response.status == "PENDING":
        print("⏳ Payment pending - awaiting async confirmation")
        print("   (In production: wait for webhook, then poll get())")
        return "pending"
    
    if auth_response.status != "AUTHORIZED":
        print(f"⚠️  Unexpected status: {auth_response.status}")
        return "error"
    
    print("✅ Funds reserved successfully")
    
    # Step 2: Capture
    print("\n[2/2] Capturing...")
    capture_request_dict = build_capture_request(
        connector_transaction_id=auth_response.connector_transaction_id,
        amount=auth_request_dict["amount"],
    )
    capture_request = ParseDict(capture_request_dict, payment_pb2.PaymentServiceCaptureRequest())
    
    capture_response = await client.capture(capture_request)
    print(f"Capture Status: {capture_response.status}")
    
    if capture_response.status in ["CAPTURED", "AUTHORIZED"]:
        print("✅ Payment captured successfully")
        return "success"
    else:
        print(f"⚠️  Capture returned: {capture_response.status}")
        return "partial"
```

</details>


<details>
<summary><b>Global Payments</b></summary>

**Configuration:**
```python
# Configuration not available
```

**Full Example:**
```python
async def process_checkout_card(
    connector_name: str,
    credentials: dict
) -> str:
    """
    Process a card payment with authorize + capture flow.
    
    Args:
        connector_name: Name of the connector (stripe, adyen, etc.)
        credentials: Connector-specific credentials
    
    Returns:
        "success" | "pending" | "failed"
    """
    print(f"\n{'='*60}")
    print(f"Processing Card Payment via {connector_name}")
    print(f"{'='*60}")
    
    # Load probe data for this connector
    probe_data = load_probe_data(connector_name)
    
    # Get configuration
    config = get_connector_config(connector_name, credentials)
    client = PaymentClient(config)
    
    # Build authorize request
    auth_request_dict = build_authorize_request(probe_data, capture_method="MANUAL")
    auth_request = ParseDict(auth_request_dict, payment_pb2.PaymentServiceAuthorizeRequest())
    
    print(f"\nRequest: {json.dumps(auth_request_dict, indent=2)}")
    
    # Step 1: Authorize
    print("\n[1/2] Authorizing...")
    auth_response = await client.authorize(auth_request)
    
    print(f"Status: {auth_response.status}")
    print(f"Connector Transaction ID: {auth_response.connector_transaction_id}")
    
    # Handle status
    if auth_response.status == "FAILED":
        print(f"❌ Payment declined: {auth_response.error_message}")
        return "failed"
    
    if auth_response.status == "PENDING":
        print("⏳ Payment pending - awaiting async confirmation")
        print("   (In production: wait for webhook, then poll get())")
        return "pending"
    
    if auth_response.status != "AUTHORIZED":
        print(f"⚠️  Unexpected status: {auth_response.status}")
        return "error"
    
    print("✅ Funds reserved successfully")
    
    # Step 2: Capture
    print("\n[2/2] Capturing...")
    capture_request_dict = build_capture_request(
        connector_transaction_id=auth_response.connector_transaction_id,
        amount=auth_request_dict["amount"],
    )
    capture_request = ParseDict(capture_request_dict, payment_pb2.PaymentServiceCaptureRequest())
    
    capture_response = await client.capture(capture_request)
    print(f"Capture Status: {capture_response.status}")
    
    if capture_response.status in ["CAPTURED", "AUTHORIZED"]:
        print("✅ Payment captured successfully")
        return "success"
    else:
        print(f"⚠️  Capture returned: {capture_response.status}")
        return "partial"
```

</details>


<details>
<summary><b>HiPay</b></summary>

**Configuration:**
```python
# Configuration not available
```

**Full Example:**
```python
async def process_checkout_card(
    connector_name: str,
    credentials: dict
) -> str:
    """
    Process a card payment with authorize + capture flow.
    
    Args:
        connector_name: Name of the connector (stripe, adyen, etc.)
        credentials: Connector-specific credentials
    
    Returns:
        "success" | "pending" | "failed"
    """
    print(f"\n{'='*60}")
    print(f"Processing Card Payment via {connector_name}")
    print(f"{'='*60}")
    
    # Load probe data for this connector
    probe_data = load_probe_data(connector_name)
    
    # Get configuration
    config = get_connector_config(connector_name, credentials)
    client = PaymentClient(config)
    
    # Build authorize request
    auth_request_dict = build_authorize_request(probe_data, capture_method="MANUAL")
    auth_request = ParseDict(auth_request_dict, payment_pb2.PaymentServiceAuthorizeRequest())
    
    print(f"\nRequest: {json.dumps(auth_request_dict, indent=2)}")
    
    # Step 1: Authorize
    print("\n[1/2] Authorizing...")
    auth_response = await client.authorize(auth_request)
    
    print(f"Status: {auth_response.status}")
    print(f"Connector Transaction ID: {auth_response.connector_transaction_id}")
    
    # Handle status
    if auth_response.status == "FAILED":
        print(f"❌ Payment declined: {auth_response.error_message}")
        return "failed"
    
    if auth_response.status == "PENDING":
        print("⏳ Payment pending - awaiting async confirmation")
        print("   (In production: wait for webhook, then poll get())")
        return "pending"
    
    if auth_response.status != "AUTHORIZED":
        print(f"⚠️  Unexpected status: {auth_response.status}")
        return "error"
    
    print("✅ Funds reserved successfully")
    
    # Step 2: Capture
    print("\n[2/2] Capturing...")
    capture_request_dict = build_capture_request(
        connector_transaction_id=auth_response.connector_transaction_id,
        amount=auth_request_dict["amount"],
    )
    capture_request = ParseDict(capture_request_dict, payment_pb2.PaymentServiceCaptureRequest())
    
    capture_response = await client.capture(capture_request)
    print(f"Capture Status: {capture_response.status}")
    
    if capture_response.status in ["CAPTURED", "AUTHORIZED"]:
        print("✅ Payment captured successfully")
        return "success"
    else:
        print(f"⚠️  Capture returned: {capture_response.status}")
        return "partial"
```

</details>


<details>
<summary><b>J.P. Morgan</b></summary>

**Configuration:**
```python
# Configuration not available
```

**Full Example:**
```python
async def process_checkout_card(
    connector_name: str,
    credentials: dict
) -> str:
    """
    Process a card payment with authorize + capture flow.
    
    Args:
        connector_name: Name of the connector (stripe, adyen, etc.)
        credentials: Connector-specific credentials
    
    Returns:
        "success" | "pending" | "failed"
    """
    print(f"\n{'='*60}")
    print(f"Processing Card Payment via {connector_name}")
    print(f"{'='*60}")
    
    # Load probe data for this connector
    probe_data = load_probe_data(connector_name)
    
    # Get configuration
    config = get_connector_config(connector_name, credentials)
    client = PaymentClient(config)
    
    # Build authorize request
    auth_request_dict = build_authorize_request(probe_data, capture_method="MANUAL")
    auth_request = ParseDict(auth_request_dict, payment_pb2.PaymentServiceAuthorizeRequest())
    
    print(f"\nRequest: {json.dumps(auth_request_dict, indent=2)}")
    
    # Step 1: Authorize
    print("\n[1/2] Authorizing...")
    auth_response = await client.authorize(auth_request)
    
    print(f"Status: {auth_response.status}")
    print(f"Connector Transaction ID: {auth_response.connector_transaction_id}")
    
    # Handle status
    if auth_response.status == "FAILED":
        print(f"❌ Payment declined: {auth_response.error_message}")
        return "failed"
    
    if auth_response.status == "PENDING":
        print("⏳ Payment pending - awaiting async confirmation")
        print("   (In production: wait for webhook, then poll get())")
        return "pending"
    
    if auth_response.status != "AUTHORIZED":
        print(f"⚠️  Unexpected status: {auth_response.status}")
        return "error"
    
    print("✅ Funds reserved successfully")
    
    # Step 2: Capture
    print("\n[2/2] Capturing...")
    capture_request_dict = build_capture_request(
        connector_transaction_id=auth_response.connector_transaction_id,
        amount=auth_request_dict["amount"],
    )
    capture_request = ParseDict(capture_request_dict, payment_pb2.PaymentServiceCaptureRequest())
    
    capture_response = await client.capture(capture_request)
    print(f"Capture Status: {capture_response.status}")
    
    if capture_response.status in ["CAPTURED", "AUTHORIZED"]:
        print("✅ Payment captured successfully")
        return "success"
    else:
        print(f"⚠️  Capture returned: {capture_response.status}")
        return "partial"
```

</details>


<details>
<summary><b>NMI</b></summary>

**Configuration:**
```python
# Configuration not available
```

**Full Example:**
```python
async def process_checkout_card(
    connector_name: str,
    credentials: dict
) -> str:
    """
    Process a card payment with authorize + capture flow.
    
    Args:
        connector_name: Name of the connector (stripe, adyen, etc.)
        credentials: Connector-specific credentials
    
    Returns:
        "success" | "pending" | "failed"
    """
    print(f"\n{'='*60}")
    print(f"Processing Card Payment via {connector_name}")
    print(f"{'='*60}")
    
    # Load probe data for this connector
    probe_data = load_probe_data(connector_name)
    
    # Get configuration
    config = get_connector_config(connector_name, credentials)
    client = PaymentClient(config)
    
    # Build authorize request
    auth_request_dict = build_authorize_request(probe_data, capture_method="MANUAL")
    auth_request = ParseDict(auth_request_dict, payment_pb2.PaymentServiceAuthorizeRequest())
    
    print(f"\nRequest: {json.dumps(auth_request_dict, indent=2)}")
    
    # Step 1: Authorize
    print("\n[1/2] Authorizing...")
    auth_response = await client.authorize(auth_request)
    
    print(f"Status: {auth_response.status}")
    print(f"Connector Transaction ID: {auth_response.connector_transaction_id}")
    
    # Handle status
    if auth_response.status == "FAILED":
        print(f"❌ Payment declined: {auth_response.error_message}")
        return "failed"
    
    if auth_response.status == "PENDING":
        print("⏳ Payment pending - awaiting async confirmation")
        print("   (In production: wait for webhook, then poll get())")
        return "pending"
    
    if auth_response.status != "AUTHORIZED":
        print(f"⚠️  Unexpected status: {auth_response.status}")
        return "error"
    
    print("✅ Funds reserved successfully")
    
    # Step 2: Capture
    print("\n[2/2] Capturing...")
    capture_request_dict = build_capture_request(
        connector_transaction_id=auth_response.connector_transaction_id,
        amount=auth_request_dict["amount"],
    )
    capture_request = ParseDict(capture_request_dict, payment_pb2.PaymentServiceCaptureRequest())
    
    capture_response = await client.capture(capture_request)
    print(f"Capture Status: {capture_response.status}")
    
    if capture_response.status in ["CAPTURED", "AUTHORIZED"]:
        print("✅ Payment captured successfully")
        return "success"
    else:
        print(f"⚠️  Capture returned: {capture_response.status}")
        return "partial"
```

</details>


<details>
<summary><b>Noon</b></summary>

**Configuration:**
```python
# Configuration not available
```

**Full Example:**
```python
async def process_checkout_card(
    connector_name: str,
    credentials: dict
) -> str:
    """
    Process a card payment with authorize + capture flow.
    
    Args:
        connector_name: Name of the connector (stripe, adyen, etc.)
        credentials: Connector-specific credentials
    
    Returns:
        "success" | "pending" | "failed"
    """
    print(f"\n{'='*60}")
    print(f"Processing Card Payment via {connector_name}")
    print(f"{'='*60}")
    
    # Load probe data for this connector
    probe_data = load_probe_data(connector_name)
    
    # Get configuration
    config = get_connector_config(connector_name, credentials)
    client = PaymentClient(config)
    
    # Build authorize request
    auth_request_dict = build_authorize_request(probe_data, capture_method="MANUAL")
    auth_request = ParseDict(auth_request_dict, payment_pb2.PaymentServiceAuthorizeRequest())
    
    print(f"\nRequest: {json.dumps(auth_request_dict, indent=2)}")
    
    # Step 1: Authorize
    print("\n[1/2] Authorizing...")
    auth_response = await client.authorize(auth_request)
    
    print(f"Status: {auth_response.status}")
    print(f"Connector Transaction ID: {auth_response.connector_transaction_id}")
    
    # Handle status
    if auth_response.status == "FAILED":
        print(f"❌ Payment declined: {auth_response.error_message}")
        return "failed"
    
    if auth_response.status == "PENDING":
        print("⏳ Payment pending - awaiting async confirmation")
        print("   (In production: wait for webhook, then poll get())")
        return "pending"
    
    if auth_response.status != "AUTHORIZED":
        print(f"⚠️  Unexpected status: {auth_response.status}")
        return "error"
    
    print("✅ Funds reserved successfully")
    
    # Step 2: Capture
    print("\n[2/2] Capturing...")
    capture_request_dict = build_capture_request(
        connector_transaction_id=auth_response.connector_transaction_id,
        amount=auth_request_dict["amount"],
    )
    capture_request = ParseDict(capture_request_dict, payment_pb2.PaymentServiceCaptureRequest())
    
    capture_response = await client.capture(capture_request)
    print(f"Capture Status: {capture_response.status}")
    
    if capture_response.status in ["CAPTURED", "AUTHORIZED"]:
        print("✅ Payment captured successfully")
        return "success"
    else:
        print(f"⚠️  Capture returned: {capture_response.status}")
        return "partial"
```

</details>


<details>
<summary><b>Novalnet</b></summary>

**Configuration:**
```python
# Configuration not available
```

**Full Example:**
```python
async def process_checkout_card(
    connector_name: str,
    credentials: dict
) -> str:
    """
    Process a card payment with authorize + capture flow.
    
    Args:
        connector_name: Name of the connector (stripe, adyen, etc.)
        credentials: Connector-specific credentials
    
    Returns:
        "success" | "pending" | "failed"
    """
    print(f"\n{'='*60}")
    print(f"Processing Card Payment via {connector_name}")
    print(f"{'='*60}")
    
    # Load probe data for this connector
    probe_data = load_probe_data(connector_name)
    
    # Get configuration
    config = get_connector_config(connector_name, credentials)
    client = PaymentClient(config)
    
    # Build authorize request
    auth_request_dict = build_authorize_request(probe_data, capture_method="MANUAL")
    auth_request = ParseDict(auth_request_dict, payment_pb2.PaymentServiceAuthorizeRequest())
    
    print(f"\nRequest: {json.dumps(auth_request_dict, indent=2)}")
    
    # Step 1: Authorize
    print("\n[1/2] Authorizing...")
    auth_response = await client.authorize(auth_request)
    
    print(f"Status: {auth_response.status}")
    print(f"Connector Transaction ID: {auth_response.connector_transaction_id}")
    
    # Handle status
    if auth_response.status == "FAILED":
        print(f"❌ Payment declined: {auth_response.error_message}")
        return "failed"
    
    if auth_response.status == "PENDING":
        print("⏳ Payment pending - awaiting async confirmation")
        print("   (In production: wait for webhook, then poll get())")
        return "pending"
    
    if auth_response.status != "AUTHORIZED":
        print(f"⚠️  Unexpected status: {auth_response.status}")
        return "error"
    
    print("✅ Funds reserved successfully")
    
    # Step 2: Capture
    print("\n[2/2] Capturing...")
    capture_request_dict = build_capture_request(
        connector_transaction_id=auth_response.connector_transaction_id,
        amount=auth_request_dict["amount"],
    )
    capture_request = ParseDict(capture_request_dict, payment_pb2.PaymentServiceCaptureRequest())
    
    capture_response = await client.capture(capture_request)
    print(f"Capture Status: {capture_response.status}")
    
    if capture_response.status in ["CAPTURED", "AUTHORIZED"]:
        print("✅ Payment captured successfully")
        return "success"
    else:
        print(f"⚠️  Capture returned: {capture_response.status}")
        return "partial"
```

</details>


<details>
<summary><b>Nuvei</b></summary>

**Configuration:**
```python
# Configuration not available
```

**Full Example:**
```python
async def process_checkout_card(
    connector_name: str,
    credentials: dict
) -> str:
    """
    Process a card payment with authorize + capture flow.
    
    Args:
        connector_name: Name of the connector (stripe, adyen, etc.)
        credentials: Connector-specific credentials
    
    Returns:
        "success" | "pending" | "failed"
    """
    print(f"\n{'='*60}")
    print(f"Processing Card Payment via {connector_name}")
    print(f"{'='*60}")
    
    # Load probe data for this connector
    probe_data = load_probe_data(connector_name)
    
    # Get configuration
    config = get_connector_config(connector_name, credentials)
    client = PaymentClient(config)
    
    # Build authorize request
    auth_request_dict = build_authorize_request(probe_data, capture_method="MANUAL")
    auth_request = ParseDict(auth_request_dict, payment_pb2.PaymentServiceAuthorizeRequest())
    
    print(f"\nRequest: {json.dumps(auth_request_dict, indent=2)}")
    
    # Step 1: Authorize
    print("\n[1/2] Authorizing...")
    auth_response = await client.authorize(auth_request)
    
    print(f"Status: {auth_response.status}")
    print(f"Connector Transaction ID: {auth_response.connector_transaction_id}")
    
    # Handle status
    if auth_response.status == "FAILED":
        print(f"❌ Payment declined: {auth_response.error_message}")
        return "failed"
    
    if auth_response.status == "PENDING":
        print("⏳ Payment pending - awaiting async confirmation")
        print("   (In production: wait for webhook, then poll get())")
        return "pending"
    
    if auth_response.status != "AUTHORIZED":
        print(f"⚠️  Unexpected status: {auth_response.status}")
        return "error"
    
    print("✅ Funds reserved successfully")
    
    # Step 2: Capture
    print("\n[2/2] Capturing...")
    capture_request_dict = build_capture_request(
        connector_transaction_id=auth_response.connector_transaction_id,
        amount=auth_request_dict["amount"],
    )
    capture_request = ParseDict(capture_request_dict, payment_pb2.PaymentServiceCaptureRequest())
    
    capture_response = await client.capture(capture_request)
    print(f"Capture Status: {capture_response.status}")
    
    if capture_response.status in ["CAPTURED", "AUTHORIZED"]:
        print("✅ Payment captured successfully")
        return "success"
    else:
        print(f"⚠️  Capture returned: {capture_response.status}")
        return "partial"
```

</details>


<details>
<summary><b>Paybox</b></summary>

**Configuration:**
```python
# Configuration not available
```

**Full Example:**
```python
async def process_checkout_card(
    connector_name: str,
    credentials: dict
) -> str:
    """
    Process a card payment with authorize + capture flow.
    
    Args:
        connector_name: Name of the connector (stripe, adyen, etc.)
        credentials: Connector-specific credentials
    
    Returns:
        "success" | "pending" | "failed"
    """
    print(f"\n{'='*60}")
    print(f"Processing Card Payment via {connector_name}")
    print(f"{'='*60}")
    
    # Load probe data for this connector
    probe_data = load_probe_data(connector_name)
    
    # Get configuration
    config = get_connector_config(connector_name, credentials)
    client = PaymentClient(config)
    
    # Build authorize request
    auth_request_dict = build_authorize_request(probe_data, capture_method="MANUAL")
    auth_request = ParseDict(auth_request_dict, payment_pb2.PaymentServiceAuthorizeRequest())
    
    print(f"\nRequest: {json.dumps(auth_request_dict, indent=2)}")
    
    # Step 1: Authorize
    print("\n[1/2] Authorizing...")
    auth_response = await client.authorize(auth_request)
    
    print(f"Status: {auth_response.status}")
    print(f"Connector Transaction ID: {auth_response.connector_transaction_id}")
    
    # Handle status
    if auth_response.status == "FAILED":
        print(f"❌ Payment declined: {auth_response.error_message}")
        return "failed"
    
    if auth_response.status == "PENDING":
        print("⏳ Payment pending - awaiting async confirmation")
        print("   (In production: wait for webhook, then poll get())")
        return "pending"
    
    if auth_response.status != "AUTHORIZED":
        print(f"⚠️  Unexpected status: {auth_response.status}")
        return "error"
    
    print("✅ Funds reserved successfully")
    
    # Step 2: Capture
    print("\n[2/2] Capturing...")
    capture_request_dict = build_capture_request(
        connector_transaction_id=auth_response.connector_transaction_id,
        amount=auth_request_dict["amount"],
    )
    capture_request = ParseDict(capture_request_dict, payment_pb2.PaymentServiceCaptureRequest())
    
    capture_response = await client.capture(capture_request)
    print(f"Capture Status: {capture_response.status}")
    
    if capture_response.status in ["CAPTURED", "AUTHORIZED"]:
        print("✅ Payment captured successfully")
        return "success"
    else:
        print(f"⚠️  Capture returned: {capture_response.status}")
        return "partial"
```

</details>


<details>
<summary><b>PayPal</b></summary>

**Configuration:**
```python
# Configuration not available
```

**Full Example:**
```python
async def process_checkout_card(
    connector_name: str,
    credentials: dict
) -> str:
    """
    Process a card payment with authorize + capture flow.
    
    Args:
        connector_name: Name of the connector (stripe, adyen, etc.)
        credentials: Connector-specific credentials
    
    Returns:
        "success" | "pending" | "failed"
    """
    print(f"\n{'='*60}")
    print(f"Processing Card Payment via {connector_name}")
    print(f"{'='*60}")
    
    # Load probe data for this connector
    probe_data = load_probe_data(connector_name)
    
    # Get configuration
    config = get_connector_config(connector_name, credentials)
    client = PaymentClient(config)
    
    # Build authorize request
    auth_request_dict = build_authorize_request(probe_data, capture_method="MANUAL")
    auth_request = ParseDict(auth_request_dict, payment_pb2.PaymentServiceAuthorizeRequest())
    
    print(f"\nRequest: {json.dumps(auth_request_dict, indent=2)}")
    
    # Step 1: Authorize
    print("\n[1/2] Authorizing...")
    auth_response = await client.authorize(auth_request)
    
    print(f"Status: {auth_response.status}")
    print(f"Connector Transaction ID: {auth_response.connector_transaction_id}")
    
    # Handle status
    if auth_response.status == "FAILED":
        print(f"❌ Payment declined: {auth_response.error_message}")
        return "failed"
    
    if auth_response.status == "PENDING":
        print("⏳ Payment pending - awaiting async confirmation")
        print("   (In production: wait for webhook, then poll get())")
        return "pending"
    
    if auth_response.status != "AUTHORIZED":
        print(f"⚠️  Unexpected status: {auth_response.status}")
        return "error"
    
    print("✅ Funds reserved successfully")
    
    # Step 2: Capture
    print("\n[2/2] Capturing...")
    capture_request_dict = build_capture_request(
        connector_transaction_id=auth_response.connector_transaction_id,
        amount=auth_request_dict["amount"],
    )
    capture_request = ParseDict(capture_request_dict, payment_pb2.PaymentServiceCaptureRequest())
    
    capture_response = await client.capture(capture_request)
    print(f"Capture Status: {capture_response.status}")
    
    if capture_response.status in ["CAPTURED", "AUTHORIZED"]:
        print("✅ Payment captured successfully")
        return "success"
    else:
        print(f"⚠️  Capture returned: {capture_response.status}")
        return "partial"
```

</details>


<details>
<summary><b>Paysafe</b></summary>

**Configuration:**
```python
# Configuration not available
```

**Full Example:**
```python
async def process_checkout_card(
    connector_name: str,
    credentials: dict
) -> str:
    """
    Process a card payment with authorize + capture flow.
    
    Args:
        connector_name: Name of the connector (stripe, adyen, etc.)
        credentials: Connector-specific credentials
    
    Returns:
        "success" | "pending" | "failed"
    """
    print(f"\n{'='*60}")
    print(f"Processing Card Payment via {connector_name}")
    print(f"{'='*60}")
    
    # Load probe data for this connector
    probe_data = load_probe_data(connector_name)
    
    # Get configuration
    config = get_connector_config(connector_name, credentials)
    client = PaymentClient(config)
    
    # Build authorize request
    auth_request_dict = build_authorize_request(probe_data, capture_method="MANUAL")
    auth_request = ParseDict(auth_request_dict, payment_pb2.PaymentServiceAuthorizeRequest())
    
    print(f"\nRequest: {json.dumps(auth_request_dict, indent=2)}")
    
    # Step 1: Authorize
    print("\n[1/2] Authorizing...")
    auth_response = await client.authorize(auth_request)
    
    print(f"Status: {auth_response.status}")
    print(f"Connector Transaction ID: {auth_response.connector_transaction_id}")
    
    # Handle status
    if auth_response.status == "FAILED":
        print(f"❌ Payment declined: {auth_response.error_message}")
        return "failed"
    
    if auth_response.status == "PENDING":
        print("⏳ Payment pending - awaiting async confirmation")
        print("   (In production: wait for webhook, then poll get())")
        return "pending"
    
    if auth_response.status != "AUTHORIZED":
        print(f"⚠️  Unexpected status: {auth_response.status}")
        return "error"
    
    print("✅ Funds reserved successfully")
    
    # Step 2: Capture
    print("\n[2/2] Capturing...")
    capture_request_dict = build_capture_request(
        connector_transaction_id=auth_response.connector_transaction_id,
        amount=auth_request_dict["amount"],
    )
    capture_request = ParseDict(capture_request_dict, payment_pb2.PaymentServiceCaptureRequest())
    
    capture_response = await client.capture(capture_request)
    print(f"Capture Status: {capture_response.status}")
    
    if capture_response.status in ["CAPTURED", "AUTHORIZED"]:
        print("✅ Payment captured successfully")
        return "success"
    else:
        print(f"⚠️  Capture returned: {capture_response.status}")
        return "partial"
```

</details>


<details>
<summary><b>PlacetoPay</b></summary>

**Configuration:**
```python
# Configuration not available
```

**Full Example:**
```python
async def process_checkout_card(
    connector_name: str,
    credentials: dict
) -> str:
    """
    Process a card payment with authorize + capture flow.
    
    Args:
        connector_name: Name of the connector (stripe, adyen, etc.)
        credentials: Connector-specific credentials
    
    Returns:
        "success" | "pending" | "failed"
    """
    print(f"\n{'='*60}")
    print(f"Processing Card Payment via {connector_name}")
    print(f"{'='*60}")
    
    # Load probe data for this connector
    probe_data = load_probe_data(connector_name)
    
    # Get configuration
    config = get_connector_config(connector_name, credentials)
    client = PaymentClient(config)
    
    # Build authorize request
    auth_request_dict = build_authorize_request(probe_data, capture_method="MANUAL")
    auth_request = ParseDict(auth_request_dict, payment_pb2.PaymentServiceAuthorizeRequest())
    
    print(f"\nRequest: {json.dumps(auth_request_dict, indent=2)}")
    
    # Step 1: Authorize
    print("\n[1/2] Authorizing...")
    auth_response = await client.authorize(auth_request)
    
    print(f"Status: {auth_response.status}")
    print(f"Connector Transaction ID: {auth_response.connector_transaction_id}")
    
    # Handle status
    if auth_response.status == "FAILED":
        print(f"❌ Payment declined: {auth_response.error_message}")
        return "failed"
    
    if auth_response.status == "PENDING":
        print("⏳ Payment pending - awaiting async confirmation")
        print("   (In production: wait for webhook, then poll get())")
        return "pending"
    
    if auth_response.status != "AUTHORIZED":
        print(f"⚠️  Unexpected status: {auth_response.status}")
        return "error"
    
    print("✅ Funds reserved successfully")
    
    # Step 2: Capture
    print("\n[2/2] Capturing...")
    capture_request_dict = build_capture_request(
        connector_transaction_id=auth_response.connector_transaction_id,
        amount=auth_request_dict["amount"],
    )
    capture_request = ParseDict(capture_request_dict, payment_pb2.PaymentServiceCaptureRequest())
    
    capture_response = await client.capture(capture_request)
    print(f"Capture Status: {capture_response.status}")
    
    if capture_response.status in ["CAPTURED", "AUTHORIZED"]:
        print("✅ Payment captured successfully")
        return "success"
    else:
        print(f"⚠️  Capture returned: {capture_response.status}")
        return "partial"
```

</details>


<details>
<summary><b>PowerTranz</b></summary>

**Configuration:**
```python
# Configuration not available
```

**Full Example:**
```python
async def process_checkout_card(
    connector_name: str,
    credentials: dict
) -> str:
    """
    Process a card payment with authorize + capture flow.
    
    Args:
        connector_name: Name of the connector (stripe, adyen, etc.)
        credentials: Connector-specific credentials
    
    Returns:
        "success" | "pending" | "failed"
    """
    print(f"\n{'='*60}")
    print(f"Processing Card Payment via {connector_name}")
    print(f"{'='*60}")
    
    # Load probe data for this connector
    probe_data = load_probe_data(connector_name)
    
    # Get configuration
    config = get_connector_config(connector_name, credentials)
    client = PaymentClient(config)
    
    # Build authorize request
    auth_request_dict = build_authorize_request(probe_data, capture_method="MANUAL")
    auth_request = ParseDict(auth_request_dict, payment_pb2.PaymentServiceAuthorizeRequest())
    
    print(f"\nRequest: {json.dumps(auth_request_dict, indent=2)}")
    
    # Step 1: Authorize
    print("\n[1/2] Authorizing...")
    auth_response = await client.authorize(auth_request)
    
    print(f"Status: {auth_response.status}")
    print(f"Connector Transaction ID: {auth_response.connector_transaction_id}")
    
    # Handle status
    if auth_response.status == "FAILED":
        print(f"❌ Payment declined: {auth_response.error_message}")
        return "failed"
    
    if auth_response.status == "PENDING":
        print("⏳ Payment pending - awaiting async confirmation")
        print("   (In production: wait for webhook, then poll get())")
        return "pending"
    
    if auth_response.status != "AUTHORIZED":
        print(f"⚠️  Unexpected status: {auth_response.status}")
        return "error"
    
    print("✅ Funds reserved successfully")
    
    # Step 2: Capture
    print("\n[2/2] Capturing...")
    capture_request_dict = build_capture_request(
        connector_transaction_id=auth_response.connector_transaction_id,
        amount=auth_request_dict["amount"],
    )
    capture_request = ParseDict(capture_request_dict, payment_pb2.PaymentServiceCaptureRequest())
    
    capture_response = await client.capture(capture_request)
    print(f"Capture Status: {capture_response.status}")
    
    if capture_response.status in ["CAPTURED", "AUTHORIZED"]:
        print("✅ Payment captured successfully")
        return "success"
    else:
        print(f"⚠️  Capture returned: {capture_response.status}")
        return "partial"
```

</details>


<details>
<summary><b>Rapyd</b></summary>

**Configuration:**
```python
# Configuration not available
```

**Full Example:**
```python
async def process_checkout_card(
    connector_name: str,
    credentials: dict
) -> str:
    """
    Process a card payment with authorize + capture flow.
    
    Args:
        connector_name: Name of the connector (stripe, adyen, etc.)
        credentials: Connector-specific credentials
    
    Returns:
        "success" | "pending" | "failed"
    """
    print(f"\n{'='*60}")
    print(f"Processing Card Payment via {connector_name}")
    print(f"{'='*60}")
    
    # Load probe data for this connector
    probe_data = load_probe_data(connector_name)
    
    # Get configuration
    config = get_connector_config(connector_name, credentials)
    client = PaymentClient(config)
    
    # Build authorize request
    auth_request_dict = build_authorize_request(probe_data, capture_method="MANUAL")
    auth_request = ParseDict(auth_request_dict, payment_pb2.PaymentServiceAuthorizeRequest())
    
    print(f"\nRequest: {json.dumps(auth_request_dict, indent=2)}")
    
    # Step 1: Authorize
    print("\n[1/2] Authorizing...")
    auth_response = await client.authorize(auth_request)
    
    print(f"Status: {auth_response.status}")
    print(f"Connector Transaction ID: {auth_response.connector_transaction_id}")
    
    # Handle status
    if auth_response.status == "FAILED":
        print(f"❌ Payment declined: {auth_response.error_message}")
        return "failed"
    
    if auth_response.status == "PENDING":
        print("⏳ Payment pending - awaiting async confirmation")
        print("   (In production: wait for webhook, then poll get())")
        return "pending"
    
    if auth_response.status != "AUTHORIZED":
        print(f"⚠️  Unexpected status: {auth_response.status}")
        return "error"
    
    print("✅ Funds reserved successfully")
    
    # Step 2: Capture
    print("\n[2/2] Capturing...")
    capture_request_dict = build_capture_request(
        connector_transaction_id=auth_response.connector_transaction_id,
        amount=auth_request_dict["amount"],
    )
    capture_request = ParseDict(capture_request_dict, payment_pb2.PaymentServiceCaptureRequest())
    
    capture_response = await client.capture(capture_request)
    print(f"Capture Status: {capture_response.status}")
    
    if capture_response.status in ["CAPTURED", "AUTHORIZED"]:
        print("✅ Payment captured successfully")
        return "success"
    else:
        print(f"⚠️  Capture returned: {capture_response.status}")
        return "partial"
```

</details>


<details>
<summary><b>Razorpay</b></summary>

**Configuration:**
```python
# Configuration not available
```

**Full Example:**
```python
async def process_checkout_card(
    connector_name: str,
    credentials: dict
) -> str:
    """
    Process a card payment with authorize + capture flow.
    
    Args:
        connector_name: Name of the connector (stripe, adyen, etc.)
        credentials: Connector-specific credentials
    
    Returns:
        "success" | "pending" | "failed"
    """
    print(f"\n{'='*60}")
    print(f"Processing Card Payment via {connector_name}")
    print(f"{'='*60}")
    
    # Load probe data for this connector
    probe_data = load_probe_data(connector_name)
    
    # Get configuration
    config = get_connector_config(connector_name, credentials)
    client = PaymentClient(config)
    
    # Build authorize request
    auth_request_dict = build_authorize_request(probe_data, capture_method="MANUAL")
    auth_request = ParseDict(auth_request_dict, payment_pb2.PaymentServiceAuthorizeRequest())
    
    print(f"\nRequest: {json.dumps(auth_request_dict, indent=2)}")
    
    # Step 1: Authorize
    print("\n[1/2] Authorizing...")
    auth_response = await client.authorize(auth_request)
    
    print(f"Status: {auth_response.status}")
    print(f"Connector Transaction ID: {auth_response.connector_transaction_id}")
    
    # Handle status
    if auth_response.status == "FAILED":
        print(f"❌ Payment declined: {auth_response.error_message}")
        return "failed"
    
    if auth_response.status == "PENDING":
        print("⏳ Payment pending - awaiting async confirmation")
        print("   (In production: wait for webhook, then poll get())")
        return "pending"
    
    if auth_response.status != "AUTHORIZED":
        print(f"⚠️  Unexpected status: {auth_response.status}")
        return "error"
    
    print("✅ Funds reserved successfully")
    
    # Step 2: Capture
    print("\n[2/2] Capturing...")
    capture_request_dict = build_capture_request(
        connector_transaction_id=auth_response.connector_transaction_id,
        amount=auth_request_dict["amount"],
    )
    capture_request = ParseDict(capture_request_dict, payment_pb2.PaymentServiceCaptureRequest())
    
    capture_response = await client.capture(capture_request)
    print(f"Capture Status: {capture_response.status}")
    
    if capture_response.status in ["CAPTURED", "AUTHORIZED"]:
        print("✅ Payment captured successfully")
        return "success"
    else:
        print(f"⚠️  Capture returned: {capture_response.status}")
        return "partial"
```

</details>


<details>
<summary><b>Revolut</b></summary>

**Configuration:**
```python
# Configuration not available
```

**Full Example:**
```python
async def process_checkout_card(
    connector_name: str,
    credentials: dict
) -> str:
    """
    Process a card payment with authorize + capture flow.
    
    Args:
        connector_name: Name of the connector (stripe, adyen, etc.)
        credentials: Connector-specific credentials
    
    Returns:
        "success" | "pending" | "failed"
    """
    print(f"\n{'='*60}")
    print(f"Processing Card Payment via {connector_name}")
    print(f"{'='*60}")
    
    # Load probe data for this connector
    probe_data = load_probe_data(connector_name)
    
    # Get configuration
    config = get_connector_config(connector_name, credentials)
    client = PaymentClient(config)
    
    # Build authorize request
    auth_request_dict = build_authorize_request(probe_data, capture_method="MANUAL")
    auth_request = ParseDict(auth_request_dict, payment_pb2.PaymentServiceAuthorizeRequest())
    
    print(f"\nRequest: {json.dumps(auth_request_dict, indent=2)}")
    
    # Step 1: Authorize
    print("\n[1/2] Authorizing...")
    auth_response = await client.authorize(auth_request)
    
    print(f"Status: {auth_response.status}")
    print(f"Connector Transaction ID: {auth_response.connector_transaction_id}")
    
    # Handle status
    if auth_response.status == "FAILED":
        print(f"❌ Payment declined: {auth_response.error_message}")
        return "failed"
    
    if auth_response.status == "PENDING":
        print("⏳ Payment pending - awaiting async confirmation")
        print("   (In production: wait for webhook, then poll get())")
        return "pending"
    
    if auth_response.status != "AUTHORIZED":
        print(f"⚠️  Unexpected status: {auth_response.status}")
        return "error"
    
    print("✅ Funds reserved successfully")
    
    # Step 2: Capture
    print("\n[2/2] Capturing...")
    capture_request_dict = build_capture_request(
        connector_transaction_id=auth_response.connector_transaction_id,
        amount=auth_request_dict["amount"],
    )
    capture_request = ParseDict(capture_request_dict, payment_pb2.PaymentServiceCaptureRequest())
    
    capture_response = await client.capture(capture_request)
    print(f"Capture Status: {capture_response.status}")
    
    if capture_response.status in ["CAPTURED", "AUTHORIZED"]:
        print("✅ Payment captured successfully")
        return "success"
    else:
        print(f"⚠️  Capture returned: {capture_response.status}")
        return "partial"
```

</details>


<details>
<summary><b>Revolv3</b></summary>

**Configuration:**
```python
# Configuration not available
```

**Full Example:**
```python
async def process_checkout_card(
    connector_name: str,
    credentials: dict
) -> str:
    """
    Process a card payment with authorize + capture flow.
    
    Args:
        connector_name: Name of the connector (stripe, adyen, etc.)
        credentials: Connector-specific credentials
    
    Returns:
        "success" | "pending" | "failed"
    """
    print(f"\n{'='*60}")
    print(f"Processing Card Payment via {connector_name}")
    print(f"{'='*60}")
    
    # Load probe data for this connector
    probe_data = load_probe_data(connector_name)
    
    # Get configuration
    config = get_connector_config(connector_name, credentials)
    client = PaymentClient(config)
    
    # Build authorize request
    auth_request_dict = build_authorize_request(probe_data, capture_method="MANUAL")
    auth_request = ParseDict(auth_request_dict, payment_pb2.PaymentServiceAuthorizeRequest())
    
    print(f"\nRequest: {json.dumps(auth_request_dict, indent=2)}")
    
    # Step 1: Authorize
    print("\n[1/2] Authorizing...")
    auth_response = await client.authorize(auth_request)
    
    print(f"Status: {auth_response.status}")
    print(f"Connector Transaction ID: {auth_response.connector_transaction_id}")
    
    # Handle status
    if auth_response.status == "FAILED":
        print(f"❌ Payment declined: {auth_response.error_message}")
        return "failed"
    
    if auth_response.status == "PENDING":
        print("⏳ Payment pending - awaiting async confirmation")
        print("   (In production: wait for webhook, then poll get())")
        return "pending"
    
    if auth_response.status != "AUTHORIZED":
        print(f"⚠️  Unexpected status: {auth_response.status}")
        return "error"
    
    print("✅ Funds reserved successfully")
    
    # Step 2: Capture
    print("\n[2/2] Capturing...")
    capture_request_dict = build_capture_request(
        connector_transaction_id=auth_response.connector_transaction_id,
        amount=auth_request_dict["amount"],
    )
    capture_request = ParseDict(capture_request_dict, payment_pb2.PaymentServiceCaptureRequest())
    
    capture_response = await client.capture(capture_request)
    print(f"Capture Status: {capture_response.status}")
    
    if capture_response.status in ["CAPTURED", "AUTHORIZED"]:
        print("✅ Payment captured successfully")
        return "success"
    else:
        print(f"⚠️  Capture returned: {capture_response.status}")
        return "partial"
```

</details>


<details>
<summary><b>Shift4</b></summary>

**Configuration:**
```python
# Configuration not available
```

**Full Example:**
```python
async def process_checkout_card(
    connector_name: str,
    credentials: dict
) -> str:
    """
    Process a card payment with authorize + capture flow.
    
    Args:
        connector_name: Name of the connector (stripe, adyen, etc.)
        credentials: Connector-specific credentials
    
    Returns:
        "success" | "pending" | "failed"
    """
    print(f"\n{'='*60}")
    print(f"Processing Card Payment via {connector_name}")
    print(f"{'='*60}")
    
    # Load probe data for this connector
    probe_data = load_probe_data(connector_name)
    
    # Get configuration
    config = get_connector_config(connector_name, credentials)
    client = PaymentClient(config)
    
    # Build authorize request
    auth_request_dict = build_authorize_request(probe_data, capture_method="MANUAL")
    auth_request = ParseDict(auth_request_dict, payment_pb2.PaymentServiceAuthorizeRequest())
    
    print(f"\nRequest: {json.dumps(auth_request_dict, indent=2)}")
    
    # Step 1: Authorize
    print("\n[1/2] Authorizing...")
    auth_response = await client.authorize(auth_request)
    
    print(f"Status: {auth_response.status}")
    print(f"Connector Transaction ID: {auth_response.connector_transaction_id}")
    
    # Handle status
    if auth_response.status == "FAILED":
        print(f"❌ Payment declined: {auth_response.error_message}")
        return "failed"
    
    if auth_response.status == "PENDING":
        print("⏳ Payment pending - awaiting async confirmation")
        print("   (In production: wait for webhook, then poll get())")
        return "pending"
    
    if auth_response.status != "AUTHORIZED":
        print(f"⚠️  Unexpected status: {auth_response.status}")
        return "error"
    
    print("✅ Funds reserved successfully")
    
    # Step 2: Capture
    print("\n[2/2] Capturing...")
    capture_request_dict = build_capture_request(
        connector_transaction_id=auth_response.connector_transaction_id,
        amount=auth_request_dict["amount"],
    )
    capture_request = ParseDict(capture_request_dict, payment_pb2.PaymentServiceCaptureRequest())
    
    capture_response = await client.capture(capture_request)
    print(f"Capture Status: {capture_response.status}")
    
    if capture_response.status in ["CAPTURED", "AUTHORIZED"]:
        print("✅ Payment captured successfully")
        return "success"
    else:
        print(f"⚠️  Capture returned: {capture_response.status}")
        return "partial"
```

</details>


<details>
<summary><b>Silverflow</b></summary>

**Configuration:**
```python
# Configuration not available
```

**Full Example:**
```python
async def process_checkout_card(
    connector_name: str,
    credentials: dict
) -> str:
    """
    Process a card payment with authorize + capture flow.
    
    Args:
        connector_name: Name of the connector (stripe, adyen, etc.)
        credentials: Connector-specific credentials
    
    Returns:
        "success" | "pending" | "failed"
    """
    print(f"\n{'='*60}")
    print(f"Processing Card Payment via {connector_name}")
    print(f"{'='*60}")
    
    # Load probe data for this connector
    probe_data = load_probe_data(connector_name)
    
    # Get configuration
    config = get_connector_config(connector_name, credentials)
    client = PaymentClient(config)
    
    # Build authorize request
    auth_request_dict = build_authorize_request(probe_data, capture_method="MANUAL")
    auth_request = ParseDict(auth_request_dict, payment_pb2.PaymentServiceAuthorizeRequest())
    
    print(f"\nRequest: {json.dumps(auth_request_dict, indent=2)}")
    
    # Step 1: Authorize
    print("\n[1/2] Authorizing...")
    auth_response = await client.authorize(auth_request)
    
    print(f"Status: {auth_response.status}")
    print(f"Connector Transaction ID: {auth_response.connector_transaction_id}")
    
    # Handle status
    if auth_response.status == "FAILED":
        print(f"❌ Payment declined: {auth_response.error_message}")
        return "failed"
    
    if auth_response.status == "PENDING":
        print("⏳ Payment pending - awaiting async confirmation")
        print("   (In production: wait for webhook, then poll get())")
        return "pending"
    
    if auth_response.status != "AUTHORIZED":
        print(f"⚠️  Unexpected status: {auth_response.status}")
        return "error"
    
    print("✅ Funds reserved successfully")
    
    # Step 2: Capture
    print("\n[2/2] Capturing...")
    capture_request_dict = build_capture_request(
        connector_transaction_id=auth_response.connector_transaction_id,
        amount=auth_request_dict["amount"],
    )
    capture_request = ParseDict(capture_request_dict, payment_pb2.PaymentServiceCaptureRequest())
    
    capture_response = await client.capture(capture_request)
    print(f"Capture Status: {capture_response.status}")
    
    if capture_response.status in ["CAPTURED", "AUTHORIZED"]:
        print("✅ Payment captured successfully")
        return "success"
    else:
        print(f"⚠️  Capture returned: {capture_response.status}")
        return "partial"
```

</details>


<details>
<summary><b>Stax</b></summary>

**Configuration:**
```python
# Configuration not available
```

**Full Example:**
```python
async def process_checkout_card(
    connector_name: str,
    credentials: dict
) -> str:
    """
    Process a card payment with authorize + capture flow.
    
    Args:
        connector_name: Name of the connector (stripe, adyen, etc.)
        credentials: Connector-specific credentials
    
    Returns:
        "success" | "pending" | "failed"
    """
    print(f"\n{'='*60}")
    print(f"Processing Card Payment via {connector_name}")
    print(f"{'='*60}")
    
    # Load probe data for this connector
    probe_data = load_probe_data(connector_name)
    
    # Get configuration
    config = get_connector_config(connector_name, credentials)
    client = PaymentClient(config)
    
    # Build authorize request
    auth_request_dict = build_authorize_request(probe_data, capture_method="MANUAL")
    auth_request = ParseDict(auth_request_dict, payment_pb2.PaymentServiceAuthorizeRequest())
    
    print(f"\nRequest: {json.dumps(auth_request_dict, indent=2)}")
    
    # Step 1: Authorize
    print("\n[1/2] Authorizing...")
    auth_response = await client.authorize(auth_request)
    
    print(f"Status: {auth_response.status}")
    print(f"Connector Transaction ID: {auth_response.connector_transaction_id}")
    
    # Handle status
    if auth_response.status == "FAILED":
        print(f"❌ Payment declined: {auth_response.error_message}")
        return "failed"
    
    if auth_response.status == "PENDING":
        print("⏳ Payment pending - awaiting async confirmation")
        print("   (In production: wait for webhook, then poll get())")
        return "pending"
    
    if auth_response.status != "AUTHORIZED":
        print(f"⚠️  Unexpected status: {auth_response.status}")
        return "error"
    
    print("✅ Funds reserved successfully")
    
    # Step 2: Capture
    print("\n[2/2] Capturing...")
    capture_request_dict = build_capture_request(
        connector_transaction_id=auth_response.connector_transaction_id,
        amount=auth_request_dict["amount"],
    )
    capture_request = ParseDict(capture_request_dict, payment_pb2.PaymentServiceCaptureRequest())
    
    capture_response = await client.capture(capture_request)
    print(f"Capture Status: {capture_response.status}")
    
    if capture_response.status in ["CAPTURED", "AUTHORIZED"]:
        print("✅ Payment captured successfully")
        return "success"
    else:
        print(f"⚠️  Capture returned: {capture_response.status}")
        return "partial"
```

</details>


<details>
<summary><b>Stripe</b></summary>

**Configuration:**
```python
def get_stripe_config(api_key: str) -> sdk_config_pb2.ConnectorConfig:
    """Configuration for Stripe."""
    config = sdk_config_pb2.ConnectorConfig(
        options=sdk_config_pb2.SdkOptions(environment=sdk_config_pb2.Environment.SANDBOX),
    )
    config.connector_config.CopyFrom(payment_pb2.ConnectorSpecificConfig(
        stripe=payment_pb2.StripeConfig(api_key=api_key),
    ))
    return config
```

**Full Example:**
```python
async def process_checkout_card(
    connector_name: str,
    credentials: dict
) -> str:
    """
    Process a card payment with authorize + capture flow.
    
    Args:
        connector_name: Name of the connector (stripe, adyen, etc.)
        credentials: Connector-specific credentials
    
    Returns:
        "success" | "pending" | "failed"
    """
    print(f"\n{'='*60}")
    print(f"Processing Card Payment via {connector_name}")
    print(f"{'='*60}")
    
    # Load probe data for this connector
    probe_data = load_probe_data(connector_name)
    
    # Get configuration
    config = get_connector_config(connector_name, credentials)
    client = PaymentClient(config)
    
    # Build authorize request
    auth_request_dict = build_authorize_request(probe_data, capture_method="MANUAL")
    auth_request = ParseDict(auth_request_dict, payment_pb2.PaymentServiceAuthorizeRequest())
    
    print(f"\nRequest: {json.dumps(auth_request_dict, indent=2)}")
    
    # Step 1: Authorize
    print("\n[1/2] Authorizing...")
    auth_response = await client.authorize(auth_request)
    
    print(f"Status: {auth_response.status}")
    print(f"Connector Transaction ID: {auth_response.connector_transaction_id}")
    
    # Handle status
    if auth_response.status == "FAILED":
        print(f"❌ Payment declined: {auth_response.error_message}")
        return "failed"
    
    if auth_response.status == "PENDING":
        print("⏳ Payment pending - awaiting async confirmation")
        print("   (In production: wait for webhook, then poll get())")
        return "pending"
    
    if auth_response.status != "AUTHORIZED":
        print(f"⚠️  Unexpected status: {auth_response.status}")
        return "error"
    
    print("✅ Funds reserved successfully")
    
    # Step 2: Capture
    print("\n[2/2] Capturing...")
    capture_request_dict = build_capture_request(
        connector_transaction_id=auth_response.connector_transaction_id,
        amount=auth_request_dict["amount"],
    )
    capture_request = ParseDict(capture_request_dict, payment_pb2.PaymentServiceCaptureRequest())
    
    capture_response = await client.capture(capture_request)
    print(f"Capture Status: {capture_response.status}")
    
    if capture_response.status in ["CAPTURED", "AUTHORIZED"]:
        print("✅ Payment captured successfully")
        return "success"
    else:
        print(f"⚠️  Capture returned: {capture_response.status}")
        return "partial"
```

</details>


<details>
<summary><b>Trust Payments</b></summary>

**Configuration:**
```python
# Configuration not available
```

**Full Example:**
```python
async def process_checkout_card(
    connector_name: str,
    credentials: dict
) -> str:
    """
    Process a card payment with authorize + capture flow.
    
    Args:
        connector_name: Name of the connector (stripe, adyen, etc.)
        credentials: Connector-specific credentials
    
    Returns:
        "success" | "pending" | "failed"
    """
    print(f"\n{'='*60}")
    print(f"Processing Card Payment via {connector_name}")
    print(f"{'='*60}")
    
    # Load probe data for this connector
    probe_data = load_probe_data(connector_name)
    
    # Get configuration
    config = get_connector_config(connector_name, credentials)
    client = PaymentClient(config)
    
    # Build authorize request
    auth_request_dict = build_authorize_request(probe_data, capture_method="MANUAL")
    auth_request = ParseDict(auth_request_dict, payment_pb2.PaymentServiceAuthorizeRequest())
    
    print(f"\nRequest: {json.dumps(auth_request_dict, indent=2)}")
    
    # Step 1: Authorize
    print("\n[1/2] Authorizing...")
    auth_response = await client.authorize(auth_request)
    
    print(f"Status: {auth_response.status}")
    print(f"Connector Transaction ID: {auth_response.connector_transaction_id}")
    
    # Handle status
    if auth_response.status == "FAILED":
        print(f"❌ Payment declined: {auth_response.error_message}")
        return "failed"
    
    if auth_response.status == "PENDING":
        print("⏳ Payment pending - awaiting async confirmation")
        print("   (In production: wait for webhook, then poll get())")
        return "pending"
    
    if auth_response.status != "AUTHORIZED":
        print(f"⚠️  Unexpected status: {auth_response.status}")
        return "error"
    
    print("✅ Funds reserved successfully")
    
    # Step 2: Capture
    print("\n[2/2] Capturing...")
    capture_request_dict = build_capture_request(
        connector_transaction_id=auth_response.connector_transaction_id,
        amount=auth_request_dict["amount"],
    )
    capture_request = ParseDict(capture_request_dict, payment_pb2.PaymentServiceCaptureRequest())
    
    capture_response = await client.capture(capture_request)
    print(f"Capture Status: {capture_response.status}")
    
    if capture_response.status in ["CAPTURED", "AUTHORIZED"]:
        print("✅ Payment captured successfully")
        return "success"
    else:
        print(f"⚠️  Capture returned: {capture_response.status}")
        return "partial"
```

</details>


<details>
<summary><b>TSYS</b></summary>

**Configuration:**
```python
# Configuration not available
```

**Full Example:**
```python
async def process_checkout_card(
    connector_name: str,
    credentials: dict
) -> str:
    """
    Process a card payment with authorize + capture flow.
    
    Args:
        connector_name: Name of the connector (stripe, adyen, etc.)
        credentials: Connector-specific credentials
    
    Returns:
        "success" | "pending" | "failed"
    """
    print(f"\n{'='*60}")
    print(f"Processing Card Payment via {connector_name}")
    print(f"{'='*60}")
    
    # Load probe data for this connector
    probe_data = load_probe_data(connector_name)
    
    # Get configuration
    config = get_connector_config(connector_name, credentials)
    client = PaymentClient(config)
    
    # Build authorize request
    auth_request_dict = build_authorize_request(probe_data, capture_method="MANUAL")
    auth_request = ParseDict(auth_request_dict, payment_pb2.PaymentServiceAuthorizeRequest())
    
    print(f"\nRequest: {json.dumps(auth_request_dict, indent=2)}")
    
    # Step 1: Authorize
    print("\n[1/2] Authorizing...")
    auth_response = await client.authorize(auth_request)
    
    print(f"Status: {auth_response.status}")
    print(f"Connector Transaction ID: {auth_response.connector_transaction_id}")
    
    # Handle status
    if auth_response.status == "FAILED":
        print(f"❌ Payment declined: {auth_response.error_message}")
        return "failed"
    
    if auth_response.status == "PENDING":
        print("⏳ Payment pending - awaiting async confirmation")
        print("   (In production: wait for webhook, then poll get())")
        return "pending"
    
    if auth_response.status != "AUTHORIZED":
        print(f"⚠️  Unexpected status: {auth_response.status}")
        return "error"
    
    print("✅ Funds reserved successfully")
    
    # Step 2: Capture
    print("\n[2/2] Capturing...")
    capture_request_dict = build_capture_request(
        connector_transaction_id=auth_response.connector_transaction_id,
        amount=auth_request_dict["amount"],
    )
    capture_request = ParseDict(capture_request_dict, payment_pb2.PaymentServiceCaptureRequest())
    
    capture_response = await client.capture(capture_request)
    print(f"Capture Status: {capture_response.status}")
    
    if capture_response.status in ["CAPTURED", "AUTHORIZED"]:
        print("✅ Payment captured successfully")
        return "success"
    else:
        print(f"⚠️  Capture returned: {capture_response.status}")
        return "partial"
```

</details>


<details>
<summary><b>Wells Fargo</b></summary>

**Configuration:**
```python
# Configuration not available
```

**Full Example:**
```python
async def process_checkout_card(
    connector_name: str,
    credentials: dict
) -> str:
    """
    Process a card payment with authorize + capture flow.
    
    Args:
        connector_name: Name of the connector (stripe, adyen, etc.)
        credentials: Connector-specific credentials
    
    Returns:
        "success" | "pending" | "failed"
    """
    print(f"\n{'='*60}")
    print(f"Processing Card Payment via {connector_name}")
    print(f"{'='*60}")
    
    # Load probe data for this connector
    probe_data = load_probe_data(connector_name)
    
    # Get configuration
    config = get_connector_config(connector_name, credentials)
    client = PaymentClient(config)
    
    # Build authorize request
    auth_request_dict = build_authorize_request(probe_data, capture_method="MANUAL")
    auth_request = ParseDict(auth_request_dict, payment_pb2.PaymentServiceAuthorizeRequest())
    
    print(f"\nRequest: {json.dumps(auth_request_dict, indent=2)}")
    
    # Step 1: Authorize
    print("\n[1/2] Authorizing...")
    auth_response = await client.authorize(auth_request)
    
    print(f"Status: {auth_response.status}")
    print(f"Connector Transaction ID: {auth_response.connector_transaction_id}")
    
    # Handle status
    if auth_response.status == "FAILED":
        print(f"❌ Payment declined: {auth_response.error_message}")
        return "failed"
    
    if auth_response.status == "PENDING":
        print("⏳ Payment pending - awaiting async confirmation")
        print("   (In production: wait for webhook, then poll get())")
        return "pending"
    
    if auth_response.status != "AUTHORIZED":
        print(f"⚠️  Unexpected status: {auth_response.status}")
        return "error"
    
    print("✅ Funds reserved successfully")
    
    # Step 2: Capture
    print("\n[2/2] Capturing...")
    capture_request_dict = build_capture_request(
        connector_transaction_id=auth_response.connector_transaction_id,
        amount=auth_request_dict["amount"],
    )
    capture_request = ParseDict(capture_request_dict, payment_pb2.PaymentServiceCaptureRequest())
    
    capture_response = await client.capture(capture_request)
    print(f"Capture Status: {capture_response.status}")
    
    if capture_response.status in ["CAPTURED", "AUTHORIZED"]:
        print("✅ Payment captured successfully")
        return "success"
    else:
        print(f"⚠️  Capture returned: {capture_response.status}")
        return "partial"
```

</details>


<details>
<summary><b>Worldpay</b></summary>

**Configuration:**
```python
# Configuration not available
```

**Full Example:**
```python
async def process_checkout_card(
    connector_name: str,
    credentials: dict
) -> str:
    """
    Process a card payment with authorize + capture flow.
    
    Args:
        connector_name: Name of the connector (stripe, adyen, etc.)
        credentials: Connector-specific credentials
    
    Returns:
        "success" | "pending" | "failed"
    """
    print(f"\n{'='*60}")
    print(f"Processing Card Payment via {connector_name}")
    print(f"{'='*60}")
    
    # Load probe data for this connector
    probe_data = load_probe_data(connector_name)
    
    # Get configuration
    config = get_connector_config(connector_name, credentials)
    client = PaymentClient(config)
    
    # Build authorize request
    auth_request_dict = build_authorize_request(probe_data, capture_method="MANUAL")
    auth_request = ParseDict(auth_request_dict, payment_pb2.PaymentServiceAuthorizeRequest())
    
    print(f"\nRequest: {json.dumps(auth_request_dict, indent=2)}")
    
    # Step 1: Authorize
    print("\n[1/2] Authorizing...")
    auth_response = await client.authorize(auth_request)
    
    print(f"Status: {auth_response.status}")
    print(f"Connector Transaction ID: {auth_response.connector_transaction_id}")
    
    # Handle status
    if auth_response.status == "FAILED":
        print(f"❌ Payment declined: {auth_response.error_message}")
        return "failed"
    
    if auth_response.status == "PENDING":
        print("⏳ Payment pending - awaiting async confirmation")
        print("   (In production: wait for webhook, then poll get())")
        return "pending"
    
    if auth_response.status != "AUTHORIZED":
        print(f"⚠️  Unexpected status: {auth_response.status}")
        return "error"
    
    print("✅ Funds reserved successfully")
    
    # Step 2: Capture
    print("\n[2/2] Capturing...")
    capture_request_dict = build_capture_request(
        connector_transaction_id=auth_response.connector_transaction_id,
        amount=auth_request_dict["amount"],
    )
    capture_request = ParseDict(capture_request_dict, payment_pb2.PaymentServiceCaptureRequest())
    
    capture_response = await client.capture(capture_request)
    print(f"Capture Status: {capture_response.status}")
    
    if capture_response.status in ["CAPTURED", "AUTHORIZED"]:
        print("✅ Payment captured successfully")
        return "success"
    else:
        print(f"⚠️  Capture returned: {capture_response.status}")
        return "partial"
```

</details>


<details>
<summary><b>Worldpay Vantiv</b></summary>

**Configuration:**
```python
# Configuration not available
```

**Full Example:**
```python
async def process_checkout_card(
    connector_name: str,
    credentials: dict
) -> str:
    """
    Process a card payment with authorize + capture flow.
    
    Args:
        connector_name: Name of the connector (stripe, adyen, etc.)
        credentials: Connector-specific credentials
    
    Returns:
        "success" | "pending" | "failed"
    """
    print(f"\n{'='*60}")
    print(f"Processing Card Payment via {connector_name}")
    print(f"{'='*60}")
    
    # Load probe data for this connector
    probe_data = load_probe_data(connector_name)
    
    # Get configuration
    config = get_connector_config(connector_name, credentials)
    client = PaymentClient(config)
    
    # Build authorize request
    auth_request_dict = build_authorize_request(probe_data, capture_method="MANUAL")
    auth_request = ParseDict(auth_request_dict, payment_pb2.PaymentServiceAuthorizeRequest())
    
    print(f"\nRequest: {json.dumps(auth_request_dict, indent=2)}")
    
    # Step 1: Authorize
    print("\n[1/2] Authorizing...")
    auth_response = await client.authorize(auth_request)
    
    print(f"Status: {auth_response.status}")
    print(f"Connector Transaction ID: {auth_response.connector_transaction_id}")
    
    # Handle status
    if auth_response.status == "FAILED":
        print(f"❌ Payment declined: {auth_response.error_message}")
        return "failed"
    
    if auth_response.status == "PENDING":
        print("⏳ Payment pending - awaiting async confirmation")
        print("   (In production: wait for webhook, then poll get())")
        return "pending"
    
    if auth_response.status != "AUTHORIZED":
        print(f"⚠️  Unexpected status: {auth_response.status}")
        return "error"
    
    print("✅ Funds reserved successfully")
    
    # Step 2: Capture
    print("\n[2/2] Capturing...")
    capture_request_dict = build_capture_request(
        connector_transaction_id=auth_response.connector_transaction_id,
        amount=auth_request_dict["amount"],
    )
    capture_request = ParseDict(capture_request_dict, payment_pb2.PaymentServiceCaptureRequest())
    
    capture_response = await client.capture(capture_request)
    print(f"Capture Status: {capture_response.status}")
    
    if capture_response.status in ["CAPTURED", "AUTHORIZED"]:
        print("✅ Payment captured successfully")
        return "success"
    else:
        print(f"⚠️  Capture returned: {capture_response.status}")
        return "partial"
```

</details>


<details>
<summary><b>Worldpay XML</b></summary>

**Configuration:**
```python
# Configuration not available
```

**Full Example:**
```python
async def process_checkout_card(
    connector_name: str,
    credentials: dict
) -> str:
    """
    Process a card payment with authorize + capture flow.
    
    Args:
        connector_name: Name of the connector (stripe, adyen, etc.)
        credentials: Connector-specific credentials
    
    Returns:
        "success" | "pending" | "failed"
    """
    print(f"\n{'='*60}")
    print(f"Processing Card Payment via {connector_name}")
    print(f"{'='*60}")
    
    # Load probe data for this connector
    probe_data = load_probe_data(connector_name)
    
    # Get configuration
    config = get_connector_config(connector_name, credentials)
    client = PaymentClient(config)
    
    # Build authorize request
    auth_request_dict = build_authorize_request(probe_data, capture_method="MANUAL")
    auth_request = ParseDict(auth_request_dict, payment_pb2.PaymentServiceAuthorizeRequest())
    
    print(f"\nRequest: {json.dumps(auth_request_dict, indent=2)}")
    
    # Step 1: Authorize
    print("\n[1/2] Authorizing...")
    auth_response = await client.authorize(auth_request)
    
    print(f"Status: {auth_response.status}")
    print(f"Connector Transaction ID: {auth_response.connector_transaction_id}")
    
    # Handle status
    if auth_response.status == "FAILED":
        print(f"❌ Payment declined: {auth_response.error_message}")
        return "failed"
    
    if auth_response.status == "PENDING":
        print("⏳ Payment pending - awaiting async confirmation")
        print("   (In production: wait for webhook, then poll get())")
        return "pending"
    
    if auth_response.status != "AUTHORIZED":
        print(f"⚠️  Unexpected status: {auth_response.status}")
        return "error"
    
    print("✅ Funds reserved successfully")
    
    # Step 2: Capture
    print("\n[2/2] Capturing...")
    capture_request_dict = build_capture_request(
        connector_transaction_id=auth_response.connector_transaction_id,
        amount=auth_request_dict["amount"],
    )
    capture_request = ParseDict(capture_request_dict, payment_pb2.PaymentServiceCaptureRequest())
    
    capture_response = await client.capture(capture_request)
    print(f"Capture Status: {capture_response.status}")
    
    if capture_response.status in ["CAPTURED", "AUTHORIZED"]:
        print("✅ Payment captured successfully")
        return "success"
    else:
        print(f"⚠️  Capture returned: {capture_response.status}")
        return "partial"
```

</details>


<details>
<summary><b>Xendit</b></summary>

**Configuration:**
```python
# Configuration not available
```

**Full Example:**
```python
async def process_checkout_card(
    connector_name: str,
    credentials: dict
) -> str:
    """
    Process a card payment with authorize + capture flow.
    
    Args:
        connector_name: Name of the connector (stripe, adyen, etc.)
        credentials: Connector-specific credentials
    
    Returns:
        "success" | "pending" | "failed"
    """
    print(f"\n{'='*60}")
    print(f"Processing Card Payment via {connector_name}")
    print(f"{'='*60}")
    
    # Load probe data for this connector
    probe_data = load_probe_data(connector_name)
    
    # Get configuration
    config = get_connector_config(connector_name, credentials)
    client = PaymentClient(config)
    
    # Build authorize request
    auth_request_dict = build_authorize_request(probe_data, capture_method="MANUAL")
    auth_request = ParseDict(auth_request_dict, payment_pb2.PaymentServiceAuthorizeRequest())
    
    print(f"\nRequest: {json.dumps(auth_request_dict, indent=2)}")
    
    # Step 1: Authorize
    print("\n[1/2] Authorizing...")
    auth_response = await client.authorize(auth_request)
    
    print(f"Status: {auth_response.status}")
    print(f"Connector Transaction ID: {auth_response.connector_transaction_id}")
    
    # Handle status
    if auth_response.status == "FAILED":
        print(f"❌ Payment declined: {auth_response.error_message}")
        return "failed"
    
    if auth_response.status == "PENDING":
        print("⏳ Payment pending - awaiting async confirmation")
        print("   (In production: wait for webhook, then poll get())")
        return "pending"
    
    if auth_response.status != "AUTHORIZED":
        print(f"⚠️  Unexpected status: {auth_response.status}")
        return "error"
    
    print("✅ Funds reserved successfully")
    
    # Step 2: Capture
    print("\n[2/2] Capturing...")
    capture_request_dict = build_capture_request(
        connector_transaction_id=auth_response.connector_transaction_id,
        amount=auth_request_dict["amount"],
    )
    capture_request = ParseDict(capture_request_dict, payment_pb2.PaymentServiceCaptureRequest())
    
    capture_response = await client.capture(capture_request)
    print(f"Capture Status: {capture_response.status}")
    
    if capture_response.status in ["CAPTURED", "AUTHORIZED"]:
        print("✅ Payment captured successfully")
        return "success"
    else:
        print(f"⚠️  Capture returned: {capture_response.status}")
        return "partial"
```

</details>


<details>
<summary><b>Zift</b></summary>

**Configuration:**
```python
# Configuration not available
```

**Full Example:**
```python
async def process_checkout_card(
    connector_name: str,
    credentials: dict
) -> str:
    """
    Process a card payment with authorize + capture flow.
    
    Args:
        connector_name: Name of the connector (stripe, adyen, etc.)
        credentials: Connector-specific credentials
    
    Returns:
        "success" | "pending" | "failed"
    """
    print(f"\n{'='*60}")
    print(f"Processing Card Payment via {connector_name}")
    print(f"{'='*60}")
    
    # Load probe data for this connector
    probe_data = load_probe_data(connector_name)
    
    # Get configuration
    config = get_connector_config(connector_name, credentials)
    client = PaymentClient(config)
    
    # Build authorize request
    auth_request_dict = build_authorize_request(probe_data, capture_method="MANUAL")
    auth_request = ParseDict(auth_request_dict, payment_pb2.PaymentServiceAuthorizeRequest())
    
    print(f"\nRequest: {json.dumps(auth_request_dict, indent=2)}")
    
    # Step 1: Authorize
    print("\n[1/2] Authorizing...")
    auth_response = await client.authorize(auth_request)
    
    print(f"Status: {auth_response.status}")
    print(f"Connector Transaction ID: {auth_response.connector_transaction_id}")
    
    # Handle status
    if auth_response.status == "FAILED":
        print(f"❌ Payment declined: {auth_response.error_message}")
        return "failed"
    
    if auth_response.status == "PENDING":
        print("⏳ Payment pending - awaiting async confirmation")
        print("   (In production: wait for webhook, then poll get())")
        return "pending"
    
    if auth_response.status != "AUTHORIZED":
        print(f"⚠️  Unexpected status: {auth_response.status}")
        return "error"
    
    print("✅ Funds reserved successfully")
    
    # Step 2: Capture
    print("\n[2/2] Capturing...")
    capture_request_dict = build_capture_request(
        connector_transaction_id=auth_response.connector_transaction_id,
        amount=auth_request_dict["amount"],
    )
    capture_request = ParseDict(capture_request_dict, payment_pb2.PaymentServiceCaptureRequest())
    
    capture_response = await client.capture(capture_request)
    print(f"Capture Status: {capture_response.status}")
    
    if capture_response.status in ["CAPTURED", "AUTHORIZED"]:
        print("✅ Payment captured successfully")
        return "success"
    else:
        print(f"⚠️  Capture returned: {capture_response.status}")
        return "partial"
```

</details>



### Rust


<details>
<summary><b>ACI</b></summary>

**Configuration:**
```rust
// Configuration not available
```

**Full Example:**
```rust
pub async fn process_checkout_card(
    connector_name: &str,
    credentials: &HashMap<String, String>,
) -> Result<String, Box<dyn Error>> {
    println!("\n{}", "=".repeat(60));
    println!("Processing Card Payment via {}", connector_name);
    println!("{}", "=".repeat(60));
    
    // Load connector probe data
    let probe_data = load_probe_data(connector_name)?;
    
    // Build config
    let config = get_connector_config(connector_name, credentials)?;
    let client = ConnectorClient::new(config, None)?;
    
    // Build request from probe data
    let auth_request = build_authorize_request(&probe_data, "MANUAL")?;
    
    // Step 1: Authorize
    println!("\n[1/2] Authorizing...");
    let auth_response = client.authorize(auth_request, &HashMap::new(), None).await?;
    
    let status = auth_response.status();
    println!("Status: {:?}", status);
    println!("Connector Transaction ID: {:?}", auth_response.connector_transaction_id);
    
    // Handle status
    match status {
        PaymentStatus::Failure | PaymentStatus::AuthorizationFailed => {
            println!("❌ Payment declined");
            return Ok("failed".to_string());
        }
        PaymentStatus::Pending => {
            println!("⏳ Payment pending - awaiting async confirmation");
            println!("   (In production: wait for webhook, then poll get())");
            return Ok("pending".to_string());
        }
        PaymentStatus::Authorized => {
            println!("✅ Funds reserved successfully");
        }
        _ => {
            println!("⚠️  Unexpected status: {:?}", status);
            return Ok("error".to_string());
        }
    }
    
    // Step 2: Capture
    println!("\n[2/2] Capturing...");
    let capture_request = build_capture_request(
        auth_response.connector_transaction_id.as_deref().unwrap_or(""),
        auth_response.amount.as_ref().unwrap()
    );
    
    let capture_response = client.capture(capture_request, &HashMap::new(), None).await?;
    let capture_status = capture_response.status();
    println!("Capture Status: {:?}", capture_status);
    
    match capture_status {
        PaymentStatus::Captured | PaymentStatus::Authorized => {
            println!("✅ Payment captured successfully");
            Ok("success".to_string())
        }
        _ => {
            println!("⚠️  Capture returned: {:?}", capture_status);
            Ok("partial".to_string())
        }
    }
}
```

</details>


<details>
<summary><b>Adyen</b></summary>

**Configuration:**
```rust
fn get_adyen_config(api_key: &str, merchant_account: &str) -> ConnectorConfig {
    ConnectorConfig {
        connector: "adyen".to_string(),
        environment: Environment::Sandbox.into(),
        auth: ConnectorAuth::BodyKey { 
            api_key: api_key.to_string().into(),
            key1: merchant_account.to_string().into(),
        },
        ..Default::default()
    }
}
```

**Full Example:**
```rust
pub async fn process_checkout_card(
    connector_name: &str,
    credentials: &HashMap<String, String>,
) -> Result<String, Box<dyn Error>> {
    println!("\n{}", "=".repeat(60));
    println!("Processing Card Payment via {}", connector_name);
    println!("{}", "=".repeat(60));
    
    // Load connector probe data
    let probe_data = load_probe_data(connector_name)?;
    
    // Build config
    let config = get_connector_config(connector_name, credentials)?;
    let client = ConnectorClient::new(config, None)?;
    
    // Build request from probe data
    let auth_request = build_authorize_request(&probe_data, "MANUAL")?;
    
    // Step 1: Authorize
    println!("\n[1/2] Authorizing...");
    let auth_response = client.authorize(auth_request, &HashMap::new(), None).await?;
    
    let status = auth_response.status();
    println!("Status: {:?}", status);
    println!("Connector Transaction ID: {:?}", auth_response.connector_transaction_id);
    
    // Handle status
    match status {
        PaymentStatus::Failure | PaymentStatus::AuthorizationFailed => {
            println!("❌ Payment declined");
            return Ok("failed".to_string());
        }
        PaymentStatus::Pending => {
            println!("⏳ Payment pending - awaiting async confirmation");
            println!("   (In production: wait for webhook, then poll get())");
            return Ok("pending".to_string());
        }
        PaymentStatus::Authorized => {
            println!("✅ Funds reserved successfully");
        }
        _ => {
            println!("⚠️  Unexpected status: {:?}", status);
            return Ok("error".to_string());
        }
    }
    
    // Step 2: Capture
    println!("\n[2/2] Capturing...");
    let capture_request = build_capture_request(
        auth_response.connector_transaction_id.as_deref().unwrap_or(""),
        auth_response.amount.as_ref().unwrap()
    );
    
    let capture_response = client.capture(capture_request, &HashMap::new(), None).await?;
    let capture_status = capture_response.status();
    println!("Capture Status: {:?}", capture_status);
    
    match capture_status {
        PaymentStatus::Captured | PaymentStatus::Authorized => {
            println!("✅ Payment captured successfully");
            Ok("success".to_string())
        }
        _ => {
            println!("⚠️  Capture returned: {:?}", capture_status);
            Ok("partial".to_string())
        }
    }
}
```

</details>


<details>
<summary><b>Airwallex</b></summary>

**Configuration:**
```rust
// Configuration not available
```

**Full Example:**
```rust
pub async fn process_checkout_card(
    connector_name: &str,
    credentials: &HashMap<String, String>,
) -> Result<String, Box<dyn Error>> {
    println!("\n{}", "=".repeat(60));
    println!("Processing Card Payment via {}", connector_name);
    println!("{}", "=".repeat(60));
    
    // Load connector probe data
    let probe_data = load_probe_data(connector_name)?;
    
    // Build config
    let config = get_connector_config(connector_name, credentials)?;
    let client = ConnectorClient::new(config, None)?;
    
    // Build request from probe data
    let auth_request = build_authorize_request(&probe_data, "MANUAL")?;
    
    // Step 1: Authorize
    println!("\n[1/2] Authorizing...");
    let auth_response = client.authorize(auth_request, &HashMap::new(), None).await?;
    
    let status = auth_response.status();
    println!("Status: {:?}", status);
    println!("Connector Transaction ID: {:?}", auth_response.connector_transaction_id);
    
    // Handle status
    match status {
        PaymentStatus::Failure | PaymentStatus::AuthorizationFailed => {
            println!("❌ Payment declined");
            return Ok("failed".to_string());
        }
        PaymentStatus::Pending => {
            println!("⏳ Payment pending - awaiting async confirmation");
            println!("   (In production: wait for webhook, then poll get())");
            return Ok("pending".to_string());
        }
        PaymentStatus::Authorized => {
            println!("✅ Funds reserved successfully");
        }
        _ => {
            println!("⚠️  Unexpected status: {:?}", status);
            return Ok("error".to_string());
        }
    }
    
    // Step 2: Capture
    println!("\n[2/2] Capturing...");
    let capture_request = build_capture_request(
        auth_response.connector_transaction_id.as_deref().unwrap_or(""),
        auth_response.amount.as_ref().unwrap()
    );
    
    let capture_response = client.capture(capture_request, &HashMap::new(), None).await?;
    let capture_status = capture_response.status();
    println!("Capture Status: {:?}", capture_status);
    
    match capture_status {
        PaymentStatus::Captured | PaymentStatus::Authorized => {
            println!("✅ Payment captured successfully");
            Ok("success".to_string())
        }
        _ => {
            println!("⚠️  Capture returned: {:?}", capture_status);
            Ok("partial".to_string())
        }
    }
}
```

</details>


<details>
<summary><b>authipay</b></summary>

**Configuration:**
```rust
// Configuration not available
```

**Full Example:**
```rust
pub async fn process_checkout_card(
    connector_name: &str,
    credentials: &HashMap<String, String>,
) -> Result<String, Box<dyn Error>> {
    println!("\n{}", "=".repeat(60));
    println!("Processing Card Payment via {}", connector_name);
    println!("{}", "=".repeat(60));
    
    // Load connector probe data
    let probe_data = load_probe_data(connector_name)?;
    
    // Build config
    let config = get_connector_config(connector_name, credentials)?;
    let client = ConnectorClient::new(config, None)?;
    
    // Build request from probe data
    let auth_request = build_authorize_request(&probe_data, "MANUAL")?;
    
    // Step 1: Authorize
    println!("\n[1/2] Authorizing...");
    let auth_response = client.authorize(auth_request, &HashMap::new(), None).await?;
    
    let status = auth_response.status();
    println!("Status: {:?}", status);
    println!("Connector Transaction ID: {:?}", auth_response.connector_transaction_id);
    
    // Handle status
    match status {
        PaymentStatus::Failure | PaymentStatus::AuthorizationFailed => {
            println!("❌ Payment declined");
            return Ok("failed".to_string());
        }
        PaymentStatus::Pending => {
            println!("⏳ Payment pending - awaiting async confirmation");
            println!("   (In production: wait for webhook, then poll get())");
            return Ok("pending".to_string());
        }
        PaymentStatus::Authorized => {
            println!("✅ Funds reserved successfully");
        }
        _ => {
            println!("⚠️  Unexpected status: {:?}", status);
            return Ok("error".to_string());
        }
    }
    
    // Step 2: Capture
    println!("\n[2/2] Capturing...");
    let capture_request = build_capture_request(
        auth_response.connector_transaction_id.as_deref().unwrap_or(""),
        auth_response.amount.as_ref().unwrap()
    );
    
    let capture_response = client.capture(capture_request, &HashMap::new(), None).await?;
    let capture_status = capture_response.status();
    println!("Capture Status: {:?}", capture_status);
    
    match capture_status {
        PaymentStatus::Captured | PaymentStatus::Authorized => {
            println!("✅ Payment captured successfully");
            Ok("success".to_string())
        }
        _ => {
            println!("⚠️  Capture returned: {:?}", capture_status);
            Ok("partial".to_string())
        }
    }
}
```

</details>


<details>
<summary><b>Authorize.net</b></summary>

**Configuration:**
```rust
// Configuration not available
```

**Full Example:**
```rust
pub async fn process_checkout_card(
    connector_name: &str,
    credentials: &HashMap<String, String>,
) -> Result<String, Box<dyn Error>> {
    println!("\n{}", "=".repeat(60));
    println!("Processing Card Payment via {}", connector_name);
    println!("{}", "=".repeat(60));
    
    // Load connector probe data
    let probe_data = load_probe_data(connector_name)?;
    
    // Build config
    let config = get_connector_config(connector_name, credentials)?;
    let client = ConnectorClient::new(config, None)?;
    
    // Build request from probe data
    let auth_request = build_authorize_request(&probe_data, "MANUAL")?;
    
    // Step 1: Authorize
    println!("\n[1/2] Authorizing...");
    let auth_response = client.authorize(auth_request, &HashMap::new(), None).await?;
    
    let status = auth_response.status();
    println!("Status: {:?}", status);
    println!("Connector Transaction ID: {:?}", auth_response.connector_transaction_id);
    
    // Handle status
    match status {
        PaymentStatus::Failure | PaymentStatus::AuthorizationFailed => {
            println!("❌ Payment declined");
            return Ok("failed".to_string());
        }
        PaymentStatus::Pending => {
            println!("⏳ Payment pending - awaiting async confirmation");
            println!("   (In production: wait for webhook, then poll get())");
            return Ok("pending".to_string());
        }
        PaymentStatus::Authorized => {
            println!("✅ Funds reserved successfully");
        }
        _ => {
            println!("⚠️  Unexpected status: {:?}", status);
            return Ok("error".to_string());
        }
    }
    
    // Step 2: Capture
    println!("\n[2/2] Capturing...");
    let capture_request = build_capture_request(
        auth_response.connector_transaction_id.as_deref().unwrap_or(""),
        auth_response.amount.as_ref().unwrap()
    );
    
    let capture_response = client.capture(capture_request, &HashMap::new(), None).await?;
    let capture_status = capture_response.status();
    println!("Capture Status: {:?}", capture_status);
    
    match capture_status {
        PaymentStatus::Captured | PaymentStatus::Authorized => {
            println!("✅ Payment captured successfully");
            Ok("success".to_string())
        }
        _ => {
            println!("⚠️  Capture returned: {:?}", capture_status);
            Ok("partial".to_string())
        }
    }
}
```

</details>


<details>
<summary><b>Bambora</b></summary>

**Configuration:**
```rust
// Configuration not available
```

**Full Example:**
```rust
pub async fn process_checkout_card(
    connector_name: &str,
    credentials: &HashMap<String, String>,
) -> Result<String, Box<dyn Error>> {
    println!("\n{}", "=".repeat(60));
    println!("Processing Card Payment via {}", connector_name);
    println!("{}", "=".repeat(60));
    
    // Load connector probe data
    let probe_data = load_probe_data(connector_name)?;
    
    // Build config
    let config = get_connector_config(connector_name, credentials)?;
    let client = ConnectorClient::new(config, None)?;
    
    // Build request from probe data
    let auth_request = build_authorize_request(&probe_data, "MANUAL")?;
    
    // Step 1: Authorize
    println!("\n[1/2] Authorizing...");
    let auth_response = client.authorize(auth_request, &HashMap::new(), None).await?;
    
    let status = auth_response.status();
    println!("Status: {:?}", status);
    println!("Connector Transaction ID: {:?}", auth_response.connector_transaction_id);
    
    // Handle status
    match status {
        PaymentStatus::Failure | PaymentStatus::AuthorizationFailed => {
            println!("❌ Payment declined");
            return Ok("failed".to_string());
        }
        PaymentStatus::Pending => {
            println!("⏳ Payment pending - awaiting async confirmation");
            println!("   (In production: wait for webhook, then poll get())");
            return Ok("pending".to_string());
        }
        PaymentStatus::Authorized => {
            println!("✅ Funds reserved successfully");
        }
        _ => {
            println!("⚠️  Unexpected status: {:?}", status);
            return Ok("error".to_string());
        }
    }
    
    // Step 2: Capture
    println!("\n[2/2] Capturing...");
    let capture_request = build_capture_request(
        auth_response.connector_transaction_id.as_deref().unwrap_or(""),
        auth_response.amount.as_ref().unwrap()
    );
    
    let capture_response = client.capture(capture_request, &HashMap::new(), None).await?;
    let capture_status = capture_response.status();
    println!("Capture Status: {:?}", capture_status);
    
    match capture_status {
        PaymentStatus::Captured | PaymentStatus::Authorized => {
            println!("✅ Payment captured successfully");
            Ok("success".to_string())
        }
        _ => {
            println!("⚠️  Capture returned: {:?}", capture_status);
            Ok("partial".to_string())
        }
    }
}
```

</details>


<details>
<summary><b>Bambora APAC</b></summary>

**Configuration:**
```rust
// Configuration not available
```

**Full Example:**
```rust
pub async fn process_checkout_card(
    connector_name: &str,
    credentials: &HashMap<String, String>,
) -> Result<String, Box<dyn Error>> {
    println!("\n{}", "=".repeat(60));
    println!("Processing Card Payment via {}", connector_name);
    println!("{}", "=".repeat(60));
    
    // Load connector probe data
    let probe_data = load_probe_data(connector_name)?;
    
    // Build config
    let config = get_connector_config(connector_name, credentials)?;
    let client = ConnectorClient::new(config, None)?;
    
    // Build request from probe data
    let auth_request = build_authorize_request(&probe_data, "MANUAL")?;
    
    // Step 1: Authorize
    println!("\n[1/2] Authorizing...");
    let auth_response = client.authorize(auth_request, &HashMap::new(), None).await?;
    
    let status = auth_response.status();
    println!("Status: {:?}", status);
    println!("Connector Transaction ID: {:?}", auth_response.connector_transaction_id);
    
    // Handle status
    match status {
        PaymentStatus::Failure | PaymentStatus::AuthorizationFailed => {
            println!("❌ Payment declined");
            return Ok("failed".to_string());
        }
        PaymentStatus::Pending => {
            println!("⏳ Payment pending - awaiting async confirmation");
            println!("   (In production: wait for webhook, then poll get())");
            return Ok("pending".to_string());
        }
        PaymentStatus::Authorized => {
            println!("✅ Funds reserved successfully");
        }
        _ => {
            println!("⚠️  Unexpected status: {:?}", status);
            return Ok("error".to_string());
        }
    }
    
    // Step 2: Capture
    println!("\n[2/2] Capturing...");
    let capture_request = build_capture_request(
        auth_response.connector_transaction_id.as_deref().unwrap_or(""),
        auth_response.amount.as_ref().unwrap()
    );
    
    let capture_response = client.capture(capture_request, &HashMap::new(), None).await?;
    let capture_status = capture_response.status();
    println!("Capture Status: {:?}", capture_status);
    
    match capture_status {
        PaymentStatus::Captured | PaymentStatus::Authorized => {
            println!("✅ Payment captured successfully");
            Ok("success".to_string())
        }
        _ => {
            println!("⚠️  Capture returned: {:?}", capture_status);
            Ok("partial".to_string())
        }
    }
}
```

</details>


<details>
<summary><b>Bank of America</b></summary>

**Configuration:**
```rust
// Configuration not available
```

**Full Example:**
```rust
pub async fn process_checkout_card(
    connector_name: &str,
    credentials: &HashMap<String, String>,
) -> Result<String, Box<dyn Error>> {
    println!("\n{}", "=".repeat(60));
    println!("Processing Card Payment via {}", connector_name);
    println!("{}", "=".repeat(60));
    
    // Load connector probe data
    let probe_data = load_probe_data(connector_name)?;
    
    // Build config
    let config = get_connector_config(connector_name, credentials)?;
    let client = ConnectorClient::new(config, None)?;
    
    // Build request from probe data
    let auth_request = build_authorize_request(&probe_data, "MANUAL")?;
    
    // Step 1: Authorize
    println!("\n[1/2] Authorizing...");
    let auth_response = client.authorize(auth_request, &HashMap::new(), None).await?;
    
    let status = auth_response.status();
    println!("Status: {:?}", status);
    println!("Connector Transaction ID: {:?}", auth_response.connector_transaction_id);
    
    // Handle status
    match status {
        PaymentStatus::Failure | PaymentStatus::AuthorizationFailed => {
            println!("❌ Payment declined");
            return Ok("failed".to_string());
        }
        PaymentStatus::Pending => {
            println!("⏳ Payment pending - awaiting async confirmation");
            println!("   (In production: wait for webhook, then poll get())");
            return Ok("pending".to_string());
        }
        PaymentStatus::Authorized => {
            println!("✅ Funds reserved successfully");
        }
        _ => {
            println!("⚠️  Unexpected status: {:?}", status);
            return Ok("error".to_string());
        }
    }
    
    // Step 2: Capture
    println!("\n[2/2] Capturing...");
    let capture_request = build_capture_request(
        auth_response.connector_transaction_id.as_deref().unwrap_or(""),
        auth_response.amount.as_ref().unwrap()
    );
    
    let capture_response = client.capture(capture_request, &HashMap::new(), None).await?;
    let capture_status = capture_response.status();
    println!("Capture Status: {:?}", capture_status);
    
    match capture_status {
        PaymentStatus::Captured | PaymentStatus::Authorized => {
            println!("✅ Payment captured successfully");
            Ok("success".to_string())
        }
        _ => {
            println!("⚠️  Capture returned: {:?}", capture_status);
            Ok("partial".to_string())
        }
    }
}
```

</details>


<details>
<summary><b>Barclaycard</b></summary>

**Configuration:**
```rust
// Configuration not available
```

**Full Example:**
```rust
pub async fn process_checkout_card(
    connector_name: &str,
    credentials: &HashMap<String, String>,
) -> Result<String, Box<dyn Error>> {
    println!("\n{}", "=".repeat(60));
    println!("Processing Card Payment via {}", connector_name);
    println!("{}", "=".repeat(60));
    
    // Load connector probe data
    let probe_data = load_probe_data(connector_name)?;
    
    // Build config
    let config = get_connector_config(connector_name, credentials)?;
    let client = ConnectorClient::new(config, None)?;
    
    // Build request from probe data
    let auth_request = build_authorize_request(&probe_data, "MANUAL")?;
    
    // Step 1: Authorize
    println!("\n[1/2] Authorizing...");
    let auth_response = client.authorize(auth_request, &HashMap::new(), None).await?;
    
    let status = auth_response.status();
    println!("Status: {:?}", status);
    println!("Connector Transaction ID: {:?}", auth_response.connector_transaction_id);
    
    // Handle status
    match status {
        PaymentStatus::Failure | PaymentStatus::AuthorizationFailed => {
            println!("❌ Payment declined");
            return Ok("failed".to_string());
        }
        PaymentStatus::Pending => {
            println!("⏳ Payment pending - awaiting async confirmation");
            println!("   (In production: wait for webhook, then poll get())");
            return Ok("pending".to_string());
        }
        PaymentStatus::Authorized => {
            println!("✅ Funds reserved successfully");
        }
        _ => {
            println!("⚠️  Unexpected status: {:?}", status);
            return Ok("error".to_string());
        }
    }
    
    // Step 2: Capture
    println!("\n[2/2] Capturing...");
    let capture_request = build_capture_request(
        auth_response.connector_transaction_id.as_deref().unwrap_or(""),
        auth_response.amount.as_ref().unwrap()
    );
    
    let capture_response = client.capture(capture_request, &HashMap::new(), None).await?;
    let capture_status = capture_response.status();
    println!("Capture Status: {:?}", capture_status);
    
    match capture_status {
        PaymentStatus::Captured | PaymentStatus::Authorized => {
            println!("✅ Payment captured successfully");
            Ok("success".to_string())
        }
        _ => {
            println!("⚠️  Capture returned: {:?}", capture_status);
            Ok("partial".to_string())
        }
    }
}
```

</details>


<details>
<summary><b>Billwerk</b></summary>

**Configuration:**
```rust
// Configuration not available
```

**Full Example:**
```rust
pub async fn process_checkout_card(
    connector_name: &str,
    credentials: &HashMap<String, String>,
) -> Result<String, Box<dyn Error>> {
    println!("\n{}", "=".repeat(60));
    println!("Processing Card Payment via {}", connector_name);
    println!("{}", "=".repeat(60));
    
    // Load connector probe data
    let probe_data = load_probe_data(connector_name)?;
    
    // Build config
    let config = get_connector_config(connector_name, credentials)?;
    let client = ConnectorClient::new(config, None)?;
    
    // Build request from probe data
    let auth_request = build_authorize_request(&probe_data, "MANUAL")?;
    
    // Step 1: Authorize
    println!("\n[1/2] Authorizing...");
    let auth_response = client.authorize(auth_request, &HashMap::new(), None).await?;
    
    let status = auth_response.status();
    println!("Status: {:?}", status);
    println!("Connector Transaction ID: {:?}", auth_response.connector_transaction_id);
    
    // Handle status
    match status {
        PaymentStatus::Failure | PaymentStatus::AuthorizationFailed => {
            println!("❌ Payment declined");
            return Ok("failed".to_string());
        }
        PaymentStatus::Pending => {
            println!("⏳ Payment pending - awaiting async confirmation");
            println!("   (In production: wait for webhook, then poll get())");
            return Ok("pending".to_string());
        }
        PaymentStatus::Authorized => {
            println!("✅ Funds reserved successfully");
        }
        _ => {
            println!("⚠️  Unexpected status: {:?}", status);
            return Ok("error".to_string());
        }
    }
    
    // Step 2: Capture
    println!("\n[2/2] Capturing...");
    let capture_request = build_capture_request(
        auth_response.connector_transaction_id.as_deref().unwrap_or(""),
        auth_response.amount.as_ref().unwrap()
    );
    
    let capture_response = client.capture(capture_request, &HashMap::new(), None).await?;
    let capture_status = capture_response.status();
    println!("Capture Status: {:?}", capture_status);
    
    match capture_status {
        PaymentStatus::Captured | PaymentStatus::Authorized => {
            println!("✅ Payment captured successfully");
            Ok("success".to_string())
        }
        _ => {
            println!("⚠️  Capture returned: {:?}", capture_status);
            Ok("partial".to_string())
        }
    }
}
```

</details>


<details>
<summary><b>BlueSnap</b></summary>

**Configuration:**
```rust
// Configuration not available
```

**Full Example:**
```rust
pub async fn process_checkout_card(
    connector_name: &str,
    credentials: &HashMap<String, String>,
) -> Result<String, Box<dyn Error>> {
    println!("\n{}", "=".repeat(60));
    println!("Processing Card Payment via {}", connector_name);
    println!("{}", "=".repeat(60));
    
    // Load connector probe data
    let probe_data = load_probe_data(connector_name)?;
    
    // Build config
    let config = get_connector_config(connector_name, credentials)?;
    let client = ConnectorClient::new(config, None)?;
    
    // Build request from probe data
    let auth_request = build_authorize_request(&probe_data, "MANUAL")?;
    
    // Step 1: Authorize
    println!("\n[1/2] Authorizing...");
    let auth_response = client.authorize(auth_request, &HashMap::new(), None).await?;
    
    let status = auth_response.status();
    println!("Status: {:?}", status);
    println!("Connector Transaction ID: {:?}", auth_response.connector_transaction_id);
    
    // Handle status
    match status {
        PaymentStatus::Failure | PaymentStatus::AuthorizationFailed => {
            println!("❌ Payment declined");
            return Ok("failed".to_string());
        }
        PaymentStatus::Pending => {
            println!("⏳ Payment pending - awaiting async confirmation");
            println!("   (In production: wait for webhook, then poll get())");
            return Ok("pending".to_string());
        }
        PaymentStatus::Authorized => {
            println!("✅ Funds reserved successfully");
        }
        _ => {
            println!("⚠️  Unexpected status: {:?}", status);
            return Ok("error".to_string());
        }
    }
    
    // Step 2: Capture
    println!("\n[2/2] Capturing...");
    let capture_request = build_capture_request(
        auth_response.connector_transaction_id.as_deref().unwrap_or(""),
        auth_response.amount.as_ref().unwrap()
    );
    
    let capture_response = client.capture(capture_request, &HashMap::new(), None).await?;
    let capture_status = capture_response.status();
    println!("Capture Status: {:?}", capture_status);
    
    match capture_status {
        PaymentStatus::Captured | PaymentStatus::Authorized => {
            println!("✅ Payment captured successfully");
            Ok("success".to_string())
        }
        _ => {
            println!("⚠️  Capture returned: {:?}", capture_status);
            Ok("partial".to_string())
        }
    }
}
```

</details>


<details>
<summary><b>Braintree</b></summary>

**Configuration:**
```rust
// Configuration not available
```

**Full Example:**
```rust
pub async fn process_checkout_card(
    connector_name: &str,
    credentials: &HashMap<String, String>,
) -> Result<String, Box<dyn Error>> {
    println!("\n{}", "=".repeat(60));
    println!("Processing Card Payment via {}", connector_name);
    println!("{}", "=".repeat(60));
    
    // Load connector probe data
    let probe_data = load_probe_data(connector_name)?;
    
    // Build config
    let config = get_connector_config(connector_name, credentials)?;
    let client = ConnectorClient::new(config, None)?;
    
    // Build request from probe data
    let auth_request = build_authorize_request(&probe_data, "MANUAL")?;
    
    // Step 1: Authorize
    println!("\n[1/2] Authorizing...");
    let auth_response = client.authorize(auth_request, &HashMap::new(), None).await?;
    
    let status = auth_response.status();
    println!("Status: {:?}", status);
    println!("Connector Transaction ID: {:?}", auth_response.connector_transaction_id);
    
    // Handle status
    match status {
        PaymentStatus::Failure | PaymentStatus::AuthorizationFailed => {
            println!("❌ Payment declined");
            return Ok("failed".to_string());
        }
        PaymentStatus::Pending => {
            println!("⏳ Payment pending - awaiting async confirmation");
            println!("   (In production: wait for webhook, then poll get())");
            return Ok("pending".to_string());
        }
        PaymentStatus::Authorized => {
            println!("✅ Funds reserved successfully");
        }
        _ => {
            println!("⚠️  Unexpected status: {:?}", status);
            return Ok("error".to_string());
        }
    }
    
    // Step 2: Capture
    println!("\n[2/2] Capturing...");
    let capture_request = build_capture_request(
        auth_response.connector_transaction_id.as_deref().unwrap_or(""),
        auth_response.amount.as_ref().unwrap()
    );
    
    let capture_response = client.capture(capture_request, &HashMap::new(), None).await?;
    let capture_status = capture_response.status();
    println!("Capture Status: {:?}", capture_status);
    
    match capture_status {
        PaymentStatus::Captured | PaymentStatus::Authorized => {
            println!("✅ Payment captured successfully");
            Ok("success".to_string())
        }
        _ => {
            println!("⚠️  Capture returned: {:?}", capture_status);
            Ok("partial".to_string())
        }
    }
}
```

</details>


<details>
<summary><b>Celero</b></summary>

**Configuration:**
```rust
// Configuration not available
```

**Full Example:**
```rust
pub async fn process_checkout_card(
    connector_name: &str,
    credentials: &HashMap<String, String>,
) -> Result<String, Box<dyn Error>> {
    println!("\n{}", "=".repeat(60));
    println!("Processing Card Payment via {}", connector_name);
    println!("{}", "=".repeat(60));
    
    // Load connector probe data
    let probe_data = load_probe_data(connector_name)?;
    
    // Build config
    let config = get_connector_config(connector_name, credentials)?;
    let client = ConnectorClient::new(config, None)?;
    
    // Build request from probe data
    let auth_request = build_authorize_request(&probe_data, "MANUAL")?;
    
    // Step 1: Authorize
    println!("\n[1/2] Authorizing...");
    let auth_response = client.authorize(auth_request, &HashMap::new(), None).await?;
    
    let status = auth_response.status();
    println!("Status: {:?}", status);
    println!("Connector Transaction ID: {:?}", auth_response.connector_transaction_id);
    
    // Handle status
    match status {
        PaymentStatus::Failure | PaymentStatus::AuthorizationFailed => {
            println!("❌ Payment declined");
            return Ok("failed".to_string());
        }
        PaymentStatus::Pending => {
            println!("⏳ Payment pending - awaiting async confirmation");
            println!("   (In production: wait for webhook, then poll get())");
            return Ok("pending".to_string());
        }
        PaymentStatus::Authorized => {
            println!("✅ Funds reserved successfully");
        }
        _ => {
            println!("⚠️  Unexpected status: {:?}", status);
            return Ok("error".to_string());
        }
    }
    
    // Step 2: Capture
    println!("\n[2/2] Capturing...");
    let capture_request = build_capture_request(
        auth_response.connector_transaction_id.as_deref().unwrap_or(""),
        auth_response.amount.as_ref().unwrap()
    );
    
    let capture_response = client.capture(capture_request, &HashMap::new(), None).await?;
    let capture_status = capture_response.status();
    println!("Capture Status: {:?}", capture_status);
    
    match capture_status {
        PaymentStatus::Captured | PaymentStatus::Authorized => {
            println!("✅ Payment captured successfully");
            Ok("success".to_string())
        }
        _ => {
            println!("⚠️  Capture returned: {:?}", capture_status);
            Ok("partial".to_string())
        }
    }
}
```

</details>


<details>
<summary><b>Checkout.com</b></summary>

**Configuration:**
```rust
fn get_checkout_config(api_key: &str) -> ConnectorConfig {
    ConnectorConfig {
        connector: "checkout".to_string(),
        environment: Environment::Sandbox.into(),
        auth: ConnectorAuth::HeaderKey { 
            api_key: api_key.to_string().into() 
        },
        ..Default::default()
    }
}
```

**Full Example:**
```rust
pub async fn process_checkout_card(
    connector_name: &str,
    credentials: &HashMap<String, String>,
) -> Result<String, Box<dyn Error>> {
    println!("\n{}", "=".repeat(60));
    println!("Processing Card Payment via {}", connector_name);
    println!("{}", "=".repeat(60));
    
    // Load connector probe data
    let probe_data = load_probe_data(connector_name)?;
    
    // Build config
    let config = get_connector_config(connector_name, credentials)?;
    let client = ConnectorClient::new(config, None)?;
    
    // Build request from probe data
    let auth_request = build_authorize_request(&probe_data, "MANUAL")?;
    
    // Step 1: Authorize
    println!("\n[1/2] Authorizing...");
    let auth_response = client.authorize(auth_request, &HashMap::new(), None).await?;
    
    let status = auth_response.status();
    println!("Status: {:?}", status);
    println!("Connector Transaction ID: {:?}", auth_response.connector_transaction_id);
    
    // Handle status
    match status {
        PaymentStatus::Failure | PaymentStatus::AuthorizationFailed => {
            println!("❌ Payment declined");
            return Ok("failed".to_string());
        }
        PaymentStatus::Pending => {
            println!("⏳ Payment pending - awaiting async confirmation");
            println!("   (In production: wait for webhook, then poll get())");
            return Ok("pending".to_string());
        }
        PaymentStatus::Authorized => {
            println!("✅ Funds reserved successfully");
        }
        _ => {
            println!("⚠️  Unexpected status: {:?}", status);
            return Ok("error".to_string());
        }
    }
    
    // Step 2: Capture
    println!("\n[2/2] Capturing...");
    let capture_request = build_capture_request(
        auth_response.connector_transaction_id.as_deref().unwrap_or(""),
        auth_response.amount.as_ref().unwrap()
    );
    
    let capture_response = client.capture(capture_request, &HashMap::new(), None).await?;
    let capture_status = capture_response.status();
    println!("Capture Status: {:?}", capture_status);
    
    match capture_status {
        PaymentStatus::Captured | PaymentStatus::Authorized => {
            println!("✅ Payment captured successfully");
            Ok("success".to_string())
        }
        _ => {
            println!("⚠️  Capture returned: {:?}", capture_status);
            Ok("partial".to_string())
        }
    }
}
```

</details>


<details>
<summary><b>CyberSource</b></summary>

**Configuration:**
```rust
// Configuration not available
```

**Full Example:**
```rust
pub async fn process_checkout_card(
    connector_name: &str,
    credentials: &HashMap<String, String>,
) -> Result<String, Box<dyn Error>> {
    println!("\n{}", "=".repeat(60));
    println!("Processing Card Payment via {}", connector_name);
    println!("{}", "=".repeat(60));
    
    // Load connector probe data
    let probe_data = load_probe_data(connector_name)?;
    
    // Build config
    let config = get_connector_config(connector_name, credentials)?;
    let client = ConnectorClient::new(config, None)?;
    
    // Build request from probe data
    let auth_request = build_authorize_request(&probe_data, "MANUAL")?;
    
    // Step 1: Authorize
    println!("\n[1/2] Authorizing...");
    let auth_response = client.authorize(auth_request, &HashMap::new(), None).await?;
    
    let status = auth_response.status();
    println!("Status: {:?}", status);
    println!("Connector Transaction ID: {:?}", auth_response.connector_transaction_id);
    
    // Handle status
    match status {
        PaymentStatus::Failure | PaymentStatus::AuthorizationFailed => {
            println!("❌ Payment declined");
            return Ok("failed".to_string());
        }
        PaymentStatus::Pending => {
            println!("⏳ Payment pending - awaiting async confirmation");
            println!("   (In production: wait for webhook, then poll get())");
            return Ok("pending".to_string());
        }
        PaymentStatus::Authorized => {
            println!("✅ Funds reserved successfully");
        }
        _ => {
            println!("⚠️  Unexpected status: {:?}", status);
            return Ok("error".to_string());
        }
    }
    
    // Step 2: Capture
    println!("\n[2/2] Capturing...");
    let capture_request = build_capture_request(
        auth_response.connector_transaction_id.as_deref().unwrap_or(""),
        auth_response.amount.as_ref().unwrap()
    );
    
    let capture_response = client.capture(capture_request, &HashMap::new(), None).await?;
    let capture_status = capture_response.status();
    println!("Capture Status: {:?}", capture_status);
    
    match capture_status {
        PaymentStatus::Captured | PaymentStatus::Authorized => {
            println!("✅ Payment captured successfully");
            Ok("success".to_string())
        }
        _ => {
            println!("⚠️  Capture returned: {:?}", capture_status);
            Ok("partial".to_string())
        }
    }
}
```

</details>


<details>
<summary><b>Datatrans</b></summary>

**Configuration:**
```rust
// Configuration not available
```

**Full Example:**
```rust
pub async fn process_checkout_card(
    connector_name: &str,
    credentials: &HashMap<String, String>,
) -> Result<String, Box<dyn Error>> {
    println!("\n{}", "=".repeat(60));
    println!("Processing Card Payment via {}", connector_name);
    println!("{}", "=".repeat(60));
    
    // Load connector probe data
    let probe_data = load_probe_data(connector_name)?;
    
    // Build config
    let config = get_connector_config(connector_name, credentials)?;
    let client = ConnectorClient::new(config, None)?;
    
    // Build request from probe data
    let auth_request = build_authorize_request(&probe_data, "MANUAL")?;
    
    // Step 1: Authorize
    println!("\n[1/2] Authorizing...");
    let auth_response = client.authorize(auth_request, &HashMap::new(), None).await?;
    
    let status = auth_response.status();
    println!("Status: {:?}", status);
    println!("Connector Transaction ID: {:?}", auth_response.connector_transaction_id);
    
    // Handle status
    match status {
        PaymentStatus::Failure | PaymentStatus::AuthorizationFailed => {
            println!("❌ Payment declined");
            return Ok("failed".to_string());
        }
        PaymentStatus::Pending => {
            println!("⏳ Payment pending - awaiting async confirmation");
            println!("   (In production: wait for webhook, then poll get())");
            return Ok("pending".to_string());
        }
        PaymentStatus::Authorized => {
            println!("✅ Funds reserved successfully");
        }
        _ => {
            println!("⚠️  Unexpected status: {:?}", status);
            return Ok("error".to_string());
        }
    }
    
    // Step 2: Capture
    println!("\n[2/2] Capturing...");
    let capture_request = build_capture_request(
        auth_response.connector_transaction_id.as_deref().unwrap_or(""),
        auth_response.amount.as_ref().unwrap()
    );
    
    let capture_response = client.capture(capture_request, &HashMap::new(), None).await?;
    let capture_status = capture_response.status();
    println!("Capture Status: {:?}", capture_status);
    
    match capture_status {
        PaymentStatus::Captured | PaymentStatus::Authorized => {
            println!("✅ Payment captured successfully");
            Ok("success".to_string())
        }
        _ => {
            println!("⚠️  Capture returned: {:?}", capture_status);
            Ok("partial".to_string())
        }
    }
}
```

</details>


<details>
<summary><b>dLocal</b></summary>

**Configuration:**
```rust
// Configuration not available
```

**Full Example:**
```rust
pub async fn process_checkout_card(
    connector_name: &str,
    credentials: &HashMap<String, String>,
) -> Result<String, Box<dyn Error>> {
    println!("\n{}", "=".repeat(60));
    println!("Processing Card Payment via {}", connector_name);
    println!("{}", "=".repeat(60));
    
    // Load connector probe data
    let probe_data = load_probe_data(connector_name)?;
    
    // Build config
    let config = get_connector_config(connector_name, credentials)?;
    let client = ConnectorClient::new(config, None)?;
    
    // Build request from probe data
    let auth_request = build_authorize_request(&probe_data, "MANUAL")?;
    
    // Step 1: Authorize
    println!("\n[1/2] Authorizing...");
    let auth_response = client.authorize(auth_request, &HashMap::new(), None).await?;
    
    let status = auth_response.status();
    println!("Status: {:?}", status);
    println!("Connector Transaction ID: {:?}", auth_response.connector_transaction_id);
    
    // Handle status
    match status {
        PaymentStatus::Failure | PaymentStatus::AuthorizationFailed => {
            println!("❌ Payment declined");
            return Ok("failed".to_string());
        }
        PaymentStatus::Pending => {
            println!("⏳ Payment pending - awaiting async confirmation");
            println!("   (In production: wait for webhook, then poll get())");
            return Ok("pending".to_string());
        }
        PaymentStatus::Authorized => {
            println!("✅ Funds reserved successfully");
        }
        _ => {
            println!("⚠️  Unexpected status: {:?}", status);
            return Ok("error".to_string());
        }
    }
    
    // Step 2: Capture
    println!("\n[2/2] Capturing...");
    let capture_request = build_capture_request(
        auth_response.connector_transaction_id.as_deref().unwrap_or(""),
        auth_response.amount.as_ref().unwrap()
    );
    
    let capture_response = client.capture(capture_request, &HashMap::new(), None).await?;
    let capture_status = capture_response.status();
    println!("Capture Status: {:?}", capture_status);
    
    match capture_status {
        PaymentStatus::Captured | PaymentStatus::Authorized => {
            println!("✅ Payment captured successfully");
            Ok("success".to_string())
        }
        _ => {
            println!("⚠️  Capture returned: {:?}", capture_status);
            Ok("partial".to_string())
        }
    }
}
```

</details>


<details>
<summary><b>Elavon</b></summary>

**Configuration:**
```rust
// Configuration not available
```

**Full Example:**
```rust
pub async fn process_checkout_card(
    connector_name: &str,
    credentials: &HashMap<String, String>,
) -> Result<String, Box<dyn Error>> {
    println!("\n{}", "=".repeat(60));
    println!("Processing Card Payment via {}", connector_name);
    println!("{}", "=".repeat(60));
    
    // Load connector probe data
    let probe_data = load_probe_data(connector_name)?;
    
    // Build config
    let config = get_connector_config(connector_name, credentials)?;
    let client = ConnectorClient::new(config, None)?;
    
    // Build request from probe data
    let auth_request = build_authorize_request(&probe_data, "MANUAL")?;
    
    // Step 1: Authorize
    println!("\n[1/2] Authorizing...");
    let auth_response = client.authorize(auth_request, &HashMap::new(), None).await?;
    
    let status = auth_response.status();
    println!("Status: {:?}", status);
    println!("Connector Transaction ID: {:?}", auth_response.connector_transaction_id);
    
    // Handle status
    match status {
        PaymentStatus::Failure | PaymentStatus::AuthorizationFailed => {
            println!("❌ Payment declined");
            return Ok("failed".to_string());
        }
        PaymentStatus::Pending => {
            println!("⏳ Payment pending - awaiting async confirmation");
            println!("   (In production: wait for webhook, then poll get())");
            return Ok("pending".to_string());
        }
        PaymentStatus::Authorized => {
            println!("✅ Funds reserved successfully");
        }
        _ => {
            println!("⚠️  Unexpected status: {:?}", status);
            return Ok("error".to_string());
        }
    }
    
    // Step 2: Capture
    println!("\n[2/2] Capturing...");
    let capture_request = build_capture_request(
        auth_response.connector_transaction_id.as_deref().unwrap_or(""),
        auth_response.amount.as_ref().unwrap()
    );
    
    let capture_response = client.capture(capture_request, &HashMap::new(), None).await?;
    let capture_status = capture_response.status();
    println!("Capture Status: {:?}", capture_status);
    
    match capture_status {
        PaymentStatus::Captured | PaymentStatus::Authorized => {
            println!("✅ Payment captured successfully");
            Ok("success".to_string())
        }
        _ => {
            println!("⚠️  Capture returned: {:?}", capture_status);
            Ok("partial".to_string())
        }
    }
}
```

</details>


<details>
<summary><b>Finix</b></summary>

**Configuration:**
```rust
// Configuration not available
```

**Full Example:**
```rust
pub async fn process_checkout_card(
    connector_name: &str,
    credentials: &HashMap<String, String>,
) -> Result<String, Box<dyn Error>> {
    println!("\n{}", "=".repeat(60));
    println!("Processing Card Payment via {}", connector_name);
    println!("{}", "=".repeat(60));
    
    // Load connector probe data
    let probe_data = load_probe_data(connector_name)?;
    
    // Build config
    let config = get_connector_config(connector_name, credentials)?;
    let client = ConnectorClient::new(config, None)?;
    
    // Build request from probe data
    let auth_request = build_authorize_request(&probe_data, "MANUAL")?;
    
    // Step 1: Authorize
    println!("\n[1/2] Authorizing...");
    let auth_response = client.authorize(auth_request, &HashMap::new(), None).await?;
    
    let status = auth_response.status();
    println!("Status: {:?}", status);
    println!("Connector Transaction ID: {:?}", auth_response.connector_transaction_id);
    
    // Handle status
    match status {
        PaymentStatus::Failure | PaymentStatus::AuthorizationFailed => {
            println!("❌ Payment declined");
            return Ok("failed".to_string());
        }
        PaymentStatus::Pending => {
            println!("⏳ Payment pending - awaiting async confirmation");
            println!("   (In production: wait for webhook, then poll get())");
            return Ok("pending".to_string());
        }
        PaymentStatus::Authorized => {
            println!("✅ Funds reserved successfully");
        }
        _ => {
            println!("⚠️  Unexpected status: {:?}", status);
            return Ok("error".to_string());
        }
    }
    
    // Step 2: Capture
    println!("\n[2/2] Capturing...");
    let capture_request = build_capture_request(
        auth_response.connector_transaction_id.as_deref().unwrap_or(""),
        auth_response.amount.as_ref().unwrap()
    );
    
    let capture_response = client.capture(capture_request, &HashMap::new(), None).await?;
    let capture_status = capture_response.status();
    println!("Capture Status: {:?}", capture_status);
    
    match capture_status {
        PaymentStatus::Captured | PaymentStatus::Authorized => {
            println!("✅ Payment captured successfully");
            Ok("success".to_string())
        }
        _ => {
            println!("⚠️  Capture returned: {:?}", capture_status);
            Ok("partial".to_string())
        }
    }
}
```

</details>


<details>
<summary><b>Fiserv</b></summary>

**Configuration:**
```rust
// Configuration not available
```

**Full Example:**
```rust
pub async fn process_checkout_card(
    connector_name: &str,
    credentials: &HashMap<String, String>,
) -> Result<String, Box<dyn Error>> {
    println!("\n{}", "=".repeat(60));
    println!("Processing Card Payment via {}", connector_name);
    println!("{}", "=".repeat(60));
    
    // Load connector probe data
    let probe_data = load_probe_data(connector_name)?;
    
    // Build config
    let config = get_connector_config(connector_name, credentials)?;
    let client = ConnectorClient::new(config, None)?;
    
    // Build request from probe data
    let auth_request = build_authorize_request(&probe_data, "MANUAL")?;
    
    // Step 1: Authorize
    println!("\n[1/2] Authorizing...");
    let auth_response = client.authorize(auth_request, &HashMap::new(), None).await?;
    
    let status = auth_response.status();
    println!("Status: {:?}", status);
    println!("Connector Transaction ID: {:?}", auth_response.connector_transaction_id);
    
    // Handle status
    match status {
        PaymentStatus::Failure | PaymentStatus::AuthorizationFailed => {
            println!("❌ Payment declined");
            return Ok("failed".to_string());
        }
        PaymentStatus::Pending => {
            println!("⏳ Payment pending - awaiting async confirmation");
            println!("   (In production: wait for webhook, then poll get())");
            return Ok("pending".to_string());
        }
        PaymentStatus::Authorized => {
            println!("✅ Funds reserved successfully");
        }
        _ => {
            println!("⚠️  Unexpected status: {:?}", status);
            return Ok("error".to_string());
        }
    }
    
    // Step 2: Capture
    println!("\n[2/2] Capturing...");
    let capture_request = build_capture_request(
        auth_response.connector_transaction_id.as_deref().unwrap_or(""),
        auth_response.amount.as_ref().unwrap()
    );
    
    let capture_response = client.capture(capture_request, &HashMap::new(), None).await?;
    let capture_status = capture_response.status();
    println!("Capture Status: {:?}", capture_status);
    
    match capture_status {
        PaymentStatus::Captured | PaymentStatus::Authorized => {
            println!("✅ Payment captured successfully");
            Ok("success".to_string())
        }
        _ => {
            println!("⚠️  Capture returned: {:?}", capture_status);
            Ok("partial".to_string())
        }
    }
}
```

</details>


<details>
<summary><b>Fiserv EMEA</b></summary>

**Configuration:**
```rust
// Configuration not available
```

**Full Example:**
```rust
pub async fn process_checkout_card(
    connector_name: &str,
    credentials: &HashMap<String, String>,
) -> Result<String, Box<dyn Error>> {
    println!("\n{}", "=".repeat(60));
    println!("Processing Card Payment via {}", connector_name);
    println!("{}", "=".repeat(60));
    
    // Load connector probe data
    let probe_data = load_probe_data(connector_name)?;
    
    // Build config
    let config = get_connector_config(connector_name, credentials)?;
    let client = ConnectorClient::new(config, None)?;
    
    // Build request from probe data
    let auth_request = build_authorize_request(&probe_data, "MANUAL")?;
    
    // Step 1: Authorize
    println!("\n[1/2] Authorizing...");
    let auth_response = client.authorize(auth_request, &HashMap::new(), None).await?;
    
    let status = auth_response.status();
    println!("Status: {:?}", status);
    println!("Connector Transaction ID: {:?}", auth_response.connector_transaction_id);
    
    // Handle status
    match status {
        PaymentStatus::Failure | PaymentStatus::AuthorizationFailed => {
            println!("❌ Payment declined");
            return Ok("failed".to_string());
        }
        PaymentStatus::Pending => {
            println!("⏳ Payment pending - awaiting async confirmation");
            println!("   (In production: wait for webhook, then poll get())");
            return Ok("pending".to_string());
        }
        PaymentStatus::Authorized => {
            println!("✅ Funds reserved successfully");
        }
        _ => {
            println!("⚠️  Unexpected status: {:?}", status);
            return Ok("error".to_string());
        }
    }
    
    // Step 2: Capture
    println!("\n[2/2] Capturing...");
    let capture_request = build_capture_request(
        auth_response.connector_transaction_id.as_deref().unwrap_or(""),
        auth_response.amount.as_ref().unwrap()
    );
    
    let capture_response = client.capture(capture_request, &HashMap::new(), None).await?;
    let capture_status = capture_response.status();
    println!("Capture Status: {:?}", capture_status);
    
    match capture_status {
        PaymentStatus::Captured | PaymentStatus::Authorized => {
            println!("✅ Payment captured successfully");
            Ok("success".to_string())
        }
        _ => {
            println!("⚠️  Capture returned: {:?}", capture_status);
            Ok("partial".to_string())
        }
    }
}
```

</details>


<details>
<summary><b>Fiuu</b></summary>

**Configuration:**
```rust
// Configuration not available
```

**Full Example:**
```rust
pub async fn process_checkout_card(
    connector_name: &str,
    credentials: &HashMap<String, String>,
) -> Result<String, Box<dyn Error>> {
    println!("\n{}", "=".repeat(60));
    println!("Processing Card Payment via {}", connector_name);
    println!("{}", "=".repeat(60));
    
    // Load connector probe data
    let probe_data = load_probe_data(connector_name)?;
    
    // Build config
    let config = get_connector_config(connector_name, credentials)?;
    let client = ConnectorClient::new(config, None)?;
    
    // Build request from probe data
    let auth_request = build_authorize_request(&probe_data, "MANUAL")?;
    
    // Step 1: Authorize
    println!("\n[1/2] Authorizing...");
    let auth_response = client.authorize(auth_request, &HashMap::new(), None).await?;
    
    let status = auth_response.status();
    println!("Status: {:?}", status);
    println!("Connector Transaction ID: {:?}", auth_response.connector_transaction_id);
    
    // Handle status
    match status {
        PaymentStatus::Failure | PaymentStatus::AuthorizationFailed => {
            println!("❌ Payment declined");
            return Ok("failed".to_string());
        }
        PaymentStatus::Pending => {
            println!("⏳ Payment pending - awaiting async confirmation");
            println!("   (In production: wait for webhook, then poll get())");
            return Ok("pending".to_string());
        }
        PaymentStatus::Authorized => {
            println!("✅ Funds reserved successfully");
        }
        _ => {
            println!("⚠️  Unexpected status: {:?}", status);
            return Ok("error".to_string());
        }
    }
    
    // Step 2: Capture
    println!("\n[2/2] Capturing...");
    let capture_request = build_capture_request(
        auth_response.connector_transaction_id.as_deref().unwrap_or(""),
        auth_response.amount.as_ref().unwrap()
    );
    
    let capture_response = client.capture(capture_request, &HashMap::new(), None).await?;
    let capture_status = capture_response.status();
    println!("Capture Status: {:?}", capture_status);
    
    match capture_status {
        PaymentStatus::Captured | PaymentStatus::Authorized => {
            println!("✅ Payment captured successfully");
            Ok("success".to_string())
        }
        _ => {
            println!("⚠️  Capture returned: {:?}", capture_status);
            Ok("partial".to_string())
        }
    }
}
```

</details>


<details>
<summary><b>Getnet</b></summary>

**Configuration:**
```rust
// Configuration not available
```

**Full Example:**
```rust
pub async fn process_checkout_card(
    connector_name: &str,
    credentials: &HashMap<String, String>,
) -> Result<String, Box<dyn Error>> {
    println!("\n{}", "=".repeat(60));
    println!("Processing Card Payment via {}", connector_name);
    println!("{}", "=".repeat(60));
    
    // Load connector probe data
    let probe_data = load_probe_data(connector_name)?;
    
    // Build config
    let config = get_connector_config(connector_name, credentials)?;
    let client = ConnectorClient::new(config, None)?;
    
    // Build request from probe data
    let auth_request = build_authorize_request(&probe_data, "MANUAL")?;
    
    // Step 1: Authorize
    println!("\n[1/2] Authorizing...");
    let auth_response = client.authorize(auth_request, &HashMap::new(), None).await?;
    
    let status = auth_response.status();
    println!("Status: {:?}", status);
    println!("Connector Transaction ID: {:?}", auth_response.connector_transaction_id);
    
    // Handle status
    match status {
        PaymentStatus::Failure | PaymentStatus::AuthorizationFailed => {
            println!("❌ Payment declined");
            return Ok("failed".to_string());
        }
        PaymentStatus::Pending => {
            println!("⏳ Payment pending - awaiting async confirmation");
            println!("   (In production: wait for webhook, then poll get())");
            return Ok("pending".to_string());
        }
        PaymentStatus::Authorized => {
            println!("✅ Funds reserved successfully");
        }
        _ => {
            println!("⚠️  Unexpected status: {:?}", status);
            return Ok("error".to_string());
        }
    }
    
    // Step 2: Capture
    println!("\n[2/2] Capturing...");
    let capture_request = build_capture_request(
        auth_response.connector_transaction_id.as_deref().unwrap_or(""),
        auth_response.amount.as_ref().unwrap()
    );
    
    let capture_response = client.capture(capture_request, &HashMap::new(), None).await?;
    let capture_status = capture_response.status();
    println!("Capture Status: {:?}", capture_status);
    
    match capture_status {
        PaymentStatus::Captured | PaymentStatus::Authorized => {
            println!("✅ Payment captured successfully");
            Ok("success".to_string())
        }
        _ => {
            println!("⚠️  Capture returned: {:?}", capture_status);
            Ok("partial".to_string())
        }
    }
}
```

</details>


<details>
<summary><b>Global Payments</b></summary>

**Configuration:**
```rust
// Configuration not available
```

**Full Example:**
```rust
pub async fn process_checkout_card(
    connector_name: &str,
    credentials: &HashMap<String, String>,
) -> Result<String, Box<dyn Error>> {
    println!("\n{}", "=".repeat(60));
    println!("Processing Card Payment via {}", connector_name);
    println!("{}", "=".repeat(60));
    
    // Load connector probe data
    let probe_data = load_probe_data(connector_name)?;
    
    // Build config
    let config = get_connector_config(connector_name, credentials)?;
    let client = ConnectorClient::new(config, None)?;
    
    // Build request from probe data
    let auth_request = build_authorize_request(&probe_data, "MANUAL")?;
    
    // Step 1: Authorize
    println!("\n[1/2] Authorizing...");
    let auth_response = client.authorize(auth_request, &HashMap::new(), None).await?;
    
    let status = auth_response.status();
    println!("Status: {:?}", status);
    println!("Connector Transaction ID: {:?}", auth_response.connector_transaction_id);
    
    // Handle status
    match status {
        PaymentStatus::Failure | PaymentStatus::AuthorizationFailed => {
            println!("❌ Payment declined");
            return Ok("failed".to_string());
        }
        PaymentStatus::Pending => {
            println!("⏳ Payment pending - awaiting async confirmation");
            println!("   (In production: wait for webhook, then poll get())");
            return Ok("pending".to_string());
        }
        PaymentStatus::Authorized => {
            println!("✅ Funds reserved successfully");
        }
        _ => {
            println!("⚠️  Unexpected status: {:?}", status);
            return Ok("error".to_string());
        }
    }
    
    // Step 2: Capture
    println!("\n[2/2] Capturing...");
    let capture_request = build_capture_request(
        auth_response.connector_transaction_id.as_deref().unwrap_or(""),
        auth_response.amount.as_ref().unwrap()
    );
    
    let capture_response = client.capture(capture_request, &HashMap::new(), None).await?;
    let capture_status = capture_response.status();
    println!("Capture Status: {:?}", capture_status);
    
    match capture_status {
        PaymentStatus::Captured | PaymentStatus::Authorized => {
            println!("✅ Payment captured successfully");
            Ok("success".to_string())
        }
        _ => {
            println!("⚠️  Capture returned: {:?}", capture_status);
            Ok("partial".to_string())
        }
    }
}
```

</details>


<details>
<summary><b>HiPay</b></summary>

**Configuration:**
```rust
// Configuration not available
```

**Full Example:**
```rust
pub async fn process_checkout_card(
    connector_name: &str,
    credentials: &HashMap<String, String>,
) -> Result<String, Box<dyn Error>> {
    println!("\n{}", "=".repeat(60));
    println!("Processing Card Payment via {}", connector_name);
    println!("{}", "=".repeat(60));
    
    // Load connector probe data
    let probe_data = load_probe_data(connector_name)?;
    
    // Build config
    let config = get_connector_config(connector_name, credentials)?;
    let client = ConnectorClient::new(config, None)?;
    
    // Build request from probe data
    let auth_request = build_authorize_request(&probe_data, "MANUAL")?;
    
    // Step 1: Authorize
    println!("\n[1/2] Authorizing...");
    let auth_response = client.authorize(auth_request, &HashMap::new(), None).await?;
    
    let status = auth_response.status();
    println!("Status: {:?}", status);
    println!("Connector Transaction ID: {:?}", auth_response.connector_transaction_id);
    
    // Handle status
    match status {
        PaymentStatus::Failure | PaymentStatus::AuthorizationFailed => {
            println!("❌ Payment declined");
            return Ok("failed".to_string());
        }
        PaymentStatus::Pending => {
            println!("⏳ Payment pending - awaiting async confirmation");
            println!("   (In production: wait for webhook, then poll get())");
            return Ok("pending".to_string());
        }
        PaymentStatus::Authorized => {
            println!("✅ Funds reserved successfully");
        }
        _ => {
            println!("⚠️  Unexpected status: {:?}", status);
            return Ok("error".to_string());
        }
    }
    
    // Step 2: Capture
    println!("\n[2/2] Capturing...");
    let capture_request = build_capture_request(
        auth_response.connector_transaction_id.as_deref().unwrap_or(""),
        auth_response.amount.as_ref().unwrap()
    );
    
    let capture_response = client.capture(capture_request, &HashMap::new(), None).await?;
    let capture_status = capture_response.status();
    println!("Capture Status: {:?}", capture_status);
    
    match capture_status {
        PaymentStatus::Captured | PaymentStatus::Authorized => {
            println!("✅ Payment captured successfully");
            Ok("success".to_string())
        }
        _ => {
            println!("⚠️  Capture returned: {:?}", capture_status);
            Ok("partial".to_string())
        }
    }
}
```

</details>


<details>
<summary><b>J.P. Morgan</b></summary>

**Configuration:**
```rust
// Configuration not available
```

**Full Example:**
```rust
pub async fn process_checkout_card(
    connector_name: &str,
    credentials: &HashMap<String, String>,
) -> Result<String, Box<dyn Error>> {
    println!("\n{}", "=".repeat(60));
    println!("Processing Card Payment via {}", connector_name);
    println!("{}", "=".repeat(60));
    
    // Load connector probe data
    let probe_data = load_probe_data(connector_name)?;
    
    // Build config
    let config = get_connector_config(connector_name, credentials)?;
    let client = ConnectorClient::new(config, None)?;
    
    // Build request from probe data
    let auth_request = build_authorize_request(&probe_data, "MANUAL")?;
    
    // Step 1: Authorize
    println!("\n[1/2] Authorizing...");
    let auth_response = client.authorize(auth_request, &HashMap::new(), None).await?;
    
    let status = auth_response.status();
    println!("Status: {:?}", status);
    println!("Connector Transaction ID: {:?}", auth_response.connector_transaction_id);
    
    // Handle status
    match status {
        PaymentStatus::Failure | PaymentStatus::AuthorizationFailed => {
            println!("❌ Payment declined");
            return Ok("failed".to_string());
        }
        PaymentStatus::Pending => {
            println!("⏳ Payment pending - awaiting async confirmation");
            println!("   (In production: wait for webhook, then poll get())");
            return Ok("pending".to_string());
        }
        PaymentStatus::Authorized => {
            println!("✅ Funds reserved successfully");
        }
        _ => {
            println!("⚠️  Unexpected status: {:?}", status);
            return Ok("error".to_string());
        }
    }
    
    // Step 2: Capture
    println!("\n[2/2] Capturing...");
    let capture_request = build_capture_request(
        auth_response.connector_transaction_id.as_deref().unwrap_or(""),
        auth_response.amount.as_ref().unwrap()
    );
    
    let capture_response = client.capture(capture_request, &HashMap::new(), None).await?;
    let capture_status = capture_response.status();
    println!("Capture Status: {:?}", capture_status);
    
    match capture_status {
        PaymentStatus::Captured | PaymentStatus::Authorized => {
            println!("✅ Payment captured successfully");
            Ok("success".to_string())
        }
        _ => {
            println!("⚠️  Capture returned: {:?}", capture_status);
            Ok("partial".to_string())
        }
    }
}
```

</details>


<details>
<summary><b>NMI</b></summary>

**Configuration:**
```rust
// Configuration not available
```

**Full Example:**
```rust
pub async fn process_checkout_card(
    connector_name: &str,
    credentials: &HashMap<String, String>,
) -> Result<String, Box<dyn Error>> {
    println!("\n{}", "=".repeat(60));
    println!("Processing Card Payment via {}", connector_name);
    println!("{}", "=".repeat(60));
    
    // Load connector probe data
    let probe_data = load_probe_data(connector_name)?;
    
    // Build config
    let config = get_connector_config(connector_name, credentials)?;
    let client = ConnectorClient::new(config, None)?;
    
    // Build request from probe data
    let auth_request = build_authorize_request(&probe_data, "MANUAL")?;
    
    // Step 1: Authorize
    println!("\n[1/2] Authorizing...");
    let auth_response = client.authorize(auth_request, &HashMap::new(), None).await?;
    
    let status = auth_response.status();
    println!("Status: {:?}", status);
    println!("Connector Transaction ID: {:?}", auth_response.connector_transaction_id);
    
    // Handle status
    match status {
        PaymentStatus::Failure | PaymentStatus::AuthorizationFailed => {
            println!("❌ Payment declined");
            return Ok("failed".to_string());
        }
        PaymentStatus::Pending => {
            println!("⏳ Payment pending - awaiting async confirmation");
            println!("   (In production: wait for webhook, then poll get())");
            return Ok("pending".to_string());
        }
        PaymentStatus::Authorized => {
            println!("✅ Funds reserved successfully");
        }
        _ => {
            println!("⚠️  Unexpected status: {:?}", status);
            return Ok("error".to_string());
        }
    }
    
    // Step 2: Capture
    println!("\n[2/2] Capturing...");
    let capture_request = build_capture_request(
        auth_response.connector_transaction_id.as_deref().unwrap_or(""),
        auth_response.amount.as_ref().unwrap()
    );
    
    let capture_response = client.capture(capture_request, &HashMap::new(), None).await?;
    let capture_status = capture_response.status();
    println!("Capture Status: {:?}", capture_status);
    
    match capture_status {
        PaymentStatus::Captured | PaymentStatus::Authorized => {
            println!("✅ Payment captured successfully");
            Ok("success".to_string())
        }
        _ => {
            println!("⚠️  Capture returned: {:?}", capture_status);
            Ok("partial".to_string())
        }
    }
}
```

</details>


<details>
<summary><b>Noon</b></summary>

**Configuration:**
```rust
// Configuration not available
```

**Full Example:**
```rust
pub async fn process_checkout_card(
    connector_name: &str,
    credentials: &HashMap<String, String>,
) -> Result<String, Box<dyn Error>> {
    println!("\n{}", "=".repeat(60));
    println!("Processing Card Payment via {}", connector_name);
    println!("{}", "=".repeat(60));
    
    // Load connector probe data
    let probe_data = load_probe_data(connector_name)?;
    
    // Build config
    let config = get_connector_config(connector_name, credentials)?;
    let client = ConnectorClient::new(config, None)?;
    
    // Build request from probe data
    let auth_request = build_authorize_request(&probe_data, "MANUAL")?;
    
    // Step 1: Authorize
    println!("\n[1/2] Authorizing...");
    let auth_response = client.authorize(auth_request, &HashMap::new(), None).await?;
    
    let status = auth_response.status();
    println!("Status: {:?}", status);
    println!("Connector Transaction ID: {:?}", auth_response.connector_transaction_id);
    
    // Handle status
    match status {
        PaymentStatus::Failure | PaymentStatus::AuthorizationFailed => {
            println!("❌ Payment declined");
            return Ok("failed".to_string());
        }
        PaymentStatus::Pending => {
            println!("⏳ Payment pending - awaiting async confirmation");
            println!("   (In production: wait for webhook, then poll get())");
            return Ok("pending".to_string());
        }
        PaymentStatus::Authorized => {
            println!("✅ Funds reserved successfully");
        }
        _ => {
            println!("⚠️  Unexpected status: {:?}", status);
            return Ok("error".to_string());
        }
    }
    
    // Step 2: Capture
    println!("\n[2/2] Capturing...");
    let capture_request = build_capture_request(
        auth_response.connector_transaction_id.as_deref().unwrap_or(""),
        auth_response.amount.as_ref().unwrap()
    );
    
    let capture_response = client.capture(capture_request, &HashMap::new(), None).await?;
    let capture_status = capture_response.status();
    println!("Capture Status: {:?}", capture_status);
    
    match capture_status {
        PaymentStatus::Captured | PaymentStatus::Authorized => {
            println!("✅ Payment captured successfully");
            Ok("success".to_string())
        }
        _ => {
            println!("⚠️  Capture returned: {:?}", capture_status);
            Ok("partial".to_string())
        }
    }
}
```

</details>


<details>
<summary><b>Novalnet</b></summary>

**Configuration:**
```rust
// Configuration not available
```

**Full Example:**
```rust
pub async fn process_checkout_card(
    connector_name: &str,
    credentials: &HashMap<String, String>,
) -> Result<String, Box<dyn Error>> {
    println!("\n{}", "=".repeat(60));
    println!("Processing Card Payment via {}", connector_name);
    println!("{}", "=".repeat(60));
    
    // Load connector probe data
    let probe_data = load_probe_data(connector_name)?;
    
    // Build config
    let config = get_connector_config(connector_name, credentials)?;
    let client = ConnectorClient::new(config, None)?;
    
    // Build request from probe data
    let auth_request = build_authorize_request(&probe_data, "MANUAL")?;
    
    // Step 1: Authorize
    println!("\n[1/2] Authorizing...");
    let auth_response = client.authorize(auth_request, &HashMap::new(), None).await?;
    
    let status = auth_response.status();
    println!("Status: {:?}", status);
    println!("Connector Transaction ID: {:?}", auth_response.connector_transaction_id);
    
    // Handle status
    match status {
        PaymentStatus::Failure | PaymentStatus::AuthorizationFailed => {
            println!("❌ Payment declined");
            return Ok("failed".to_string());
        }
        PaymentStatus::Pending => {
            println!("⏳ Payment pending - awaiting async confirmation");
            println!("   (In production: wait for webhook, then poll get())");
            return Ok("pending".to_string());
        }
        PaymentStatus::Authorized => {
            println!("✅ Funds reserved successfully");
        }
        _ => {
            println!("⚠️  Unexpected status: {:?}", status);
            return Ok("error".to_string());
        }
    }
    
    // Step 2: Capture
    println!("\n[2/2] Capturing...");
    let capture_request = build_capture_request(
        auth_response.connector_transaction_id.as_deref().unwrap_or(""),
        auth_response.amount.as_ref().unwrap()
    );
    
    let capture_response = client.capture(capture_request, &HashMap::new(), None).await?;
    let capture_status = capture_response.status();
    println!("Capture Status: {:?}", capture_status);
    
    match capture_status {
        PaymentStatus::Captured | PaymentStatus::Authorized => {
            println!("✅ Payment captured successfully");
            Ok("success".to_string())
        }
        _ => {
            println!("⚠️  Capture returned: {:?}", capture_status);
            Ok("partial".to_string())
        }
    }
}
```

</details>


<details>
<summary><b>Nuvei</b></summary>

**Configuration:**
```rust
// Configuration not available
```

**Full Example:**
```rust
pub async fn process_checkout_card(
    connector_name: &str,
    credentials: &HashMap<String, String>,
) -> Result<String, Box<dyn Error>> {
    println!("\n{}", "=".repeat(60));
    println!("Processing Card Payment via {}", connector_name);
    println!("{}", "=".repeat(60));
    
    // Load connector probe data
    let probe_data = load_probe_data(connector_name)?;
    
    // Build config
    let config = get_connector_config(connector_name, credentials)?;
    let client = ConnectorClient::new(config, None)?;
    
    // Build request from probe data
    let auth_request = build_authorize_request(&probe_data, "MANUAL")?;
    
    // Step 1: Authorize
    println!("\n[1/2] Authorizing...");
    let auth_response = client.authorize(auth_request, &HashMap::new(), None).await?;
    
    let status = auth_response.status();
    println!("Status: {:?}", status);
    println!("Connector Transaction ID: {:?}", auth_response.connector_transaction_id);
    
    // Handle status
    match status {
        PaymentStatus::Failure | PaymentStatus::AuthorizationFailed => {
            println!("❌ Payment declined");
            return Ok("failed".to_string());
        }
        PaymentStatus::Pending => {
            println!("⏳ Payment pending - awaiting async confirmation");
            println!("   (In production: wait for webhook, then poll get())");
            return Ok("pending".to_string());
        }
        PaymentStatus::Authorized => {
            println!("✅ Funds reserved successfully");
        }
        _ => {
            println!("⚠️  Unexpected status: {:?}", status);
            return Ok("error".to_string());
        }
    }
    
    // Step 2: Capture
    println!("\n[2/2] Capturing...");
    let capture_request = build_capture_request(
        auth_response.connector_transaction_id.as_deref().unwrap_or(""),
        auth_response.amount.as_ref().unwrap()
    );
    
    let capture_response = client.capture(capture_request, &HashMap::new(), None).await?;
    let capture_status = capture_response.status();
    println!("Capture Status: {:?}", capture_status);
    
    match capture_status {
        PaymentStatus::Captured | PaymentStatus::Authorized => {
            println!("✅ Payment captured successfully");
            Ok("success".to_string())
        }
        _ => {
            println!("⚠️  Capture returned: {:?}", capture_status);
            Ok("partial".to_string())
        }
    }
}
```

</details>


<details>
<summary><b>Paybox</b></summary>

**Configuration:**
```rust
// Configuration not available
```

**Full Example:**
```rust
pub async fn process_checkout_card(
    connector_name: &str,
    credentials: &HashMap<String, String>,
) -> Result<String, Box<dyn Error>> {
    println!("\n{}", "=".repeat(60));
    println!("Processing Card Payment via {}", connector_name);
    println!("{}", "=".repeat(60));
    
    // Load connector probe data
    let probe_data = load_probe_data(connector_name)?;
    
    // Build config
    let config = get_connector_config(connector_name, credentials)?;
    let client = ConnectorClient::new(config, None)?;
    
    // Build request from probe data
    let auth_request = build_authorize_request(&probe_data, "MANUAL")?;
    
    // Step 1: Authorize
    println!("\n[1/2] Authorizing...");
    let auth_response = client.authorize(auth_request, &HashMap::new(), None).await?;
    
    let status = auth_response.status();
    println!("Status: {:?}", status);
    println!("Connector Transaction ID: {:?}", auth_response.connector_transaction_id);
    
    // Handle status
    match status {
        PaymentStatus::Failure | PaymentStatus::AuthorizationFailed => {
            println!("❌ Payment declined");
            return Ok("failed".to_string());
        }
        PaymentStatus::Pending => {
            println!("⏳ Payment pending - awaiting async confirmation");
            println!("   (In production: wait for webhook, then poll get())");
            return Ok("pending".to_string());
        }
        PaymentStatus::Authorized => {
            println!("✅ Funds reserved successfully");
        }
        _ => {
            println!("⚠️  Unexpected status: {:?}", status);
            return Ok("error".to_string());
        }
    }
    
    // Step 2: Capture
    println!("\n[2/2] Capturing...");
    let capture_request = build_capture_request(
        auth_response.connector_transaction_id.as_deref().unwrap_or(""),
        auth_response.amount.as_ref().unwrap()
    );
    
    let capture_response = client.capture(capture_request, &HashMap::new(), None).await?;
    let capture_status = capture_response.status();
    println!("Capture Status: {:?}", capture_status);
    
    match capture_status {
        PaymentStatus::Captured | PaymentStatus::Authorized => {
            println!("✅ Payment captured successfully");
            Ok("success".to_string())
        }
        _ => {
            println!("⚠️  Capture returned: {:?}", capture_status);
            Ok("partial".to_string())
        }
    }
}
```

</details>


<details>
<summary><b>PayPal</b></summary>

**Configuration:**
```rust
// Configuration not available
```

**Full Example:**
```rust
pub async fn process_checkout_card(
    connector_name: &str,
    credentials: &HashMap<String, String>,
) -> Result<String, Box<dyn Error>> {
    println!("\n{}", "=".repeat(60));
    println!("Processing Card Payment via {}", connector_name);
    println!("{}", "=".repeat(60));
    
    // Load connector probe data
    let probe_data = load_probe_data(connector_name)?;
    
    // Build config
    let config = get_connector_config(connector_name, credentials)?;
    let client = ConnectorClient::new(config, None)?;
    
    // Build request from probe data
    let auth_request = build_authorize_request(&probe_data, "MANUAL")?;
    
    // Step 1: Authorize
    println!("\n[1/2] Authorizing...");
    let auth_response = client.authorize(auth_request, &HashMap::new(), None).await?;
    
    let status = auth_response.status();
    println!("Status: {:?}", status);
    println!("Connector Transaction ID: {:?}", auth_response.connector_transaction_id);
    
    // Handle status
    match status {
        PaymentStatus::Failure | PaymentStatus::AuthorizationFailed => {
            println!("❌ Payment declined");
            return Ok("failed".to_string());
        }
        PaymentStatus::Pending => {
            println!("⏳ Payment pending - awaiting async confirmation");
            println!("   (In production: wait for webhook, then poll get())");
            return Ok("pending".to_string());
        }
        PaymentStatus::Authorized => {
            println!("✅ Funds reserved successfully");
        }
        _ => {
            println!("⚠️  Unexpected status: {:?}", status);
            return Ok("error".to_string());
        }
    }
    
    // Step 2: Capture
    println!("\n[2/2] Capturing...");
    let capture_request = build_capture_request(
        auth_response.connector_transaction_id.as_deref().unwrap_or(""),
        auth_response.amount.as_ref().unwrap()
    );
    
    let capture_response = client.capture(capture_request, &HashMap::new(), None).await?;
    let capture_status = capture_response.status();
    println!("Capture Status: {:?}", capture_status);
    
    match capture_status {
        PaymentStatus::Captured | PaymentStatus::Authorized => {
            println!("✅ Payment captured successfully");
            Ok("success".to_string())
        }
        _ => {
            println!("⚠️  Capture returned: {:?}", capture_status);
            Ok("partial".to_string())
        }
    }
}
```

</details>


<details>
<summary><b>Paysafe</b></summary>

**Configuration:**
```rust
// Configuration not available
```

**Full Example:**
```rust
pub async fn process_checkout_card(
    connector_name: &str,
    credentials: &HashMap<String, String>,
) -> Result<String, Box<dyn Error>> {
    println!("\n{}", "=".repeat(60));
    println!("Processing Card Payment via {}", connector_name);
    println!("{}", "=".repeat(60));
    
    // Load connector probe data
    let probe_data = load_probe_data(connector_name)?;
    
    // Build config
    let config = get_connector_config(connector_name, credentials)?;
    let client = ConnectorClient::new(config, None)?;
    
    // Build request from probe data
    let auth_request = build_authorize_request(&probe_data, "MANUAL")?;
    
    // Step 1: Authorize
    println!("\n[1/2] Authorizing...");
    let auth_response = client.authorize(auth_request, &HashMap::new(), None).await?;
    
    let status = auth_response.status();
    println!("Status: {:?}", status);
    println!("Connector Transaction ID: {:?}", auth_response.connector_transaction_id);
    
    // Handle status
    match status {
        PaymentStatus::Failure | PaymentStatus::AuthorizationFailed => {
            println!("❌ Payment declined");
            return Ok("failed".to_string());
        }
        PaymentStatus::Pending => {
            println!("⏳ Payment pending - awaiting async confirmation");
            println!("   (In production: wait for webhook, then poll get())");
            return Ok("pending".to_string());
        }
        PaymentStatus::Authorized => {
            println!("✅ Funds reserved successfully");
        }
        _ => {
            println!("⚠️  Unexpected status: {:?}", status);
            return Ok("error".to_string());
        }
    }
    
    // Step 2: Capture
    println!("\n[2/2] Capturing...");
    let capture_request = build_capture_request(
        auth_response.connector_transaction_id.as_deref().unwrap_or(""),
        auth_response.amount.as_ref().unwrap()
    );
    
    let capture_response = client.capture(capture_request, &HashMap::new(), None).await?;
    let capture_status = capture_response.status();
    println!("Capture Status: {:?}", capture_status);
    
    match capture_status {
        PaymentStatus::Captured | PaymentStatus::Authorized => {
            println!("✅ Payment captured successfully");
            Ok("success".to_string())
        }
        _ => {
            println!("⚠️  Capture returned: {:?}", capture_status);
            Ok("partial".to_string())
        }
    }
}
```

</details>


<details>
<summary><b>PlacetoPay</b></summary>

**Configuration:**
```rust
// Configuration not available
```

**Full Example:**
```rust
pub async fn process_checkout_card(
    connector_name: &str,
    credentials: &HashMap<String, String>,
) -> Result<String, Box<dyn Error>> {
    println!("\n{}", "=".repeat(60));
    println!("Processing Card Payment via {}", connector_name);
    println!("{}", "=".repeat(60));
    
    // Load connector probe data
    let probe_data = load_probe_data(connector_name)?;
    
    // Build config
    let config = get_connector_config(connector_name, credentials)?;
    let client = ConnectorClient::new(config, None)?;
    
    // Build request from probe data
    let auth_request = build_authorize_request(&probe_data, "MANUAL")?;
    
    // Step 1: Authorize
    println!("\n[1/2] Authorizing...");
    let auth_response = client.authorize(auth_request, &HashMap::new(), None).await?;
    
    let status = auth_response.status();
    println!("Status: {:?}", status);
    println!("Connector Transaction ID: {:?}", auth_response.connector_transaction_id);
    
    // Handle status
    match status {
        PaymentStatus::Failure | PaymentStatus::AuthorizationFailed => {
            println!("❌ Payment declined");
            return Ok("failed".to_string());
        }
        PaymentStatus::Pending => {
            println!("⏳ Payment pending - awaiting async confirmation");
            println!("   (In production: wait for webhook, then poll get())");
            return Ok("pending".to_string());
        }
        PaymentStatus::Authorized => {
            println!("✅ Funds reserved successfully");
        }
        _ => {
            println!("⚠️  Unexpected status: {:?}", status);
            return Ok("error".to_string());
        }
    }
    
    // Step 2: Capture
    println!("\n[2/2] Capturing...");
    let capture_request = build_capture_request(
        auth_response.connector_transaction_id.as_deref().unwrap_or(""),
        auth_response.amount.as_ref().unwrap()
    );
    
    let capture_response = client.capture(capture_request, &HashMap::new(), None).await?;
    let capture_status = capture_response.status();
    println!("Capture Status: {:?}", capture_status);
    
    match capture_status {
        PaymentStatus::Captured | PaymentStatus::Authorized => {
            println!("✅ Payment captured successfully");
            Ok("success".to_string())
        }
        _ => {
            println!("⚠️  Capture returned: {:?}", capture_status);
            Ok("partial".to_string())
        }
    }
}
```

</details>


<details>
<summary><b>PowerTranz</b></summary>

**Configuration:**
```rust
// Configuration not available
```

**Full Example:**
```rust
pub async fn process_checkout_card(
    connector_name: &str,
    credentials: &HashMap<String, String>,
) -> Result<String, Box<dyn Error>> {
    println!("\n{}", "=".repeat(60));
    println!("Processing Card Payment via {}", connector_name);
    println!("{}", "=".repeat(60));
    
    // Load connector probe data
    let probe_data = load_probe_data(connector_name)?;
    
    // Build config
    let config = get_connector_config(connector_name, credentials)?;
    let client = ConnectorClient::new(config, None)?;
    
    // Build request from probe data
    let auth_request = build_authorize_request(&probe_data, "MANUAL")?;
    
    // Step 1: Authorize
    println!("\n[1/2] Authorizing...");
    let auth_response = client.authorize(auth_request, &HashMap::new(), None).await?;
    
    let status = auth_response.status();
    println!("Status: {:?}", status);
    println!("Connector Transaction ID: {:?}", auth_response.connector_transaction_id);
    
    // Handle status
    match status {
        PaymentStatus::Failure | PaymentStatus::AuthorizationFailed => {
            println!("❌ Payment declined");
            return Ok("failed".to_string());
        }
        PaymentStatus::Pending => {
            println!("⏳ Payment pending - awaiting async confirmation");
            println!("   (In production: wait for webhook, then poll get())");
            return Ok("pending".to_string());
        }
        PaymentStatus::Authorized => {
            println!("✅ Funds reserved successfully");
        }
        _ => {
            println!("⚠️  Unexpected status: {:?}", status);
            return Ok("error".to_string());
        }
    }
    
    // Step 2: Capture
    println!("\n[2/2] Capturing...");
    let capture_request = build_capture_request(
        auth_response.connector_transaction_id.as_deref().unwrap_or(""),
        auth_response.amount.as_ref().unwrap()
    );
    
    let capture_response = client.capture(capture_request, &HashMap::new(), None).await?;
    let capture_status = capture_response.status();
    println!("Capture Status: {:?}", capture_status);
    
    match capture_status {
        PaymentStatus::Captured | PaymentStatus::Authorized => {
            println!("✅ Payment captured successfully");
            Ok("success".to_string())
        }
        _ => {
            println!("⚠️  Capture returned: {:?}", capture_status);
            Ok("partial".to_string())
        }
    }
}
```

</details>


<details>
<summary><b>Rapyd</b></summary>

**Configuration:**
```rust
// Configuration not available
```

**Full Example:**
```rust
pub async fn process_checkout_card(
    connector_name: &str,
    credentials: &HashMap<String, String>,
) -> Result<String, Box<dyn Error>> {
    println!("\n{}", "=".repeat(60));
    println!("Processing Card Payment via {}", connector_name);
    println!("{}", "=".repeat(60));
    
    // Load connector probe data
    let probe_data = load_probe_data(connector_name)?;
    
    // Build config
    let config = get_connector_config(connector_name, credentials)?;
    let client = ConnectorClient::new(config, None)?;
    
    // Build request from probe data
    let auth_request = build_authorize_request(&probe_data, "MANUAL")?;
    
    // Step 1: Authorize
    println!("\n[1/2] Authorizing...");
    let auth_response = client.authorize(auth_request, &HashMap::new(), None).await?;
    
    let status = auth_response.status();
    println!("Status: {:?}", status);
    println!("Connector Transaction ID: {:?}", auth_response.connector_transaction_id);
    
    // Handle status
    match status {
        PaymentStatus::Failure | PaymentStatus::AuthorizationFailed => {
            println!("❌ Payment declined");
            return Ok("failed".to_string());
        }
        PaymentStatus::Pending => {
            println!("⏳ Payment pending - awaiting async confirmation");
            println!("   (In production: wait for webhook, then poll get())");
            return Ok("pending".to_string());
        }
        PaymentStatus::Authorized => {
            println!("✅ Funds reserved successfully");
        }
        _ => {
            println!("⚠️  Unexpected status: {:?}", status);
            return Ok("error".to_string());
        }
    }
    
    // Step 2: Capture
    println!("\n[2/2] Capturing...");
    let capture_request = build_capture_request(
        auth_response.connector_transaction_id.as_deref().unwrap_or(""),
        auth_response.amount.as_ref().unwrap()
    );
    
    let capture_response = client.capture(capture_request, &HashMap::new(), None).await?;
    let capture_status = capture_response.status();
    println!("Capture Status: {:?}", capture_status);
    
    match capture_status {
        PaymentStatus::Captured | PaymentStatus::Authorized => {
            println!("✅ Payment captured successfully");
            Ok("success".to_string())
        }
        _ => {
            println!("⚠️  Capture returned: {:?}", capture_status);
            Ok("partial".to_string())
        }
    }
}
```

</details>


<details>
<summary><b>Razorpay</b></summary>

**Configuration:**
```rust
// Configuration not available
```

**Full Example:**
```rust
pub async fn process_checkout_card(
    connector_name: &str,
    credentials: &HashMap<String, String>,
) -> Result<String, Box<dyn Error>> {
    println!("\n{}", "=".repeat(60));
    println!("Processing Card Payment via {}", connector_name);
    println!("{}", "=".repeat(60));
    
    // Load connector probe data
    let probe_data = load_probe_data(connector_name)?;
    
    // Build config
    let config = get_connector_config(connector_name, credentials)?;
    let client = ConnectorClient::new(config, None)?;
    
    // Build request from probe data
    let auth_request = build_authorize_request(&probe_data, "MANUAL")?;
    
    // Step 1: Authorize
    println!("\n[1/2] Authorizing...");
    let auth_response = client.authorize(auth_request, &HashMap::new(), None).await?;
    
    let status = auth_response.status();
    println!("Status: {:?}", status);
    println!("Connector Transaction ID: {:?}", auth_response.connector_transaction_id);
    
    // Handle status
    match status {
        PaymentStatus::Failure | PaymentStatus::AuthorizationFailed => {
            println!("❌ Payment declined");
            return Ok("failed".to_string());
        }
        PaymentStatus::Pending => {
            println!("⏳ Payment pending - awaiting async confirmation");
            println!("   (In production: wait for webhook, then poll get())");
            return Ok("pending".to_string());
        }
        PaymentStatus::Authorized => {
            println!("✅ Funds reserved successfully");
        }
        _ => {
            println!("⚠️  Unexpected status: {:?}", status);
            return Ok("error".to_string());
        }
    }
    
    // Step 2: Capture
    println!("\n[2/2] Capturing...");
    let capture_request = build_capture_request(
        auth_response.connector_transaction_id.as_deref().unwrap_or(""),
        auth_response.amount.as_ref().unwrap()
    );
    
    let capture_response = client.capture(capture_request, &HashMap::new(), None).await?;
    let capture_status = capture_response.status();
    println!("Capture Status: {:?}", capture_status);
    
    match capture_status {
        PaymentStatus::Captured | PaymentStatus::Authorized => {
            println!("✅ Payment captured successfully");
            Ok("success".to_string())
        }
        _ => {
            println!("⚠️  Capture returned: {:?}", capture_status);
            Ok("partial".to_string())
        }
    }
}
```

</details>


<details>
<summary><b>Revolut</b></summary>

**Configuration:**
```rust
// Configuration not available
```

**Full Example:**
```rust
pub async fn process_checkout_card(
    connector_name: &str,
    credentials: &HashMap<String, String>,
) -> Result<String, Box<dyn Error>> {
    println!("\n{}", "=".repeat(60));
    println!("Processing Card Payment via {}", connector_name);
    println!("{}", "=".repeat(60));
    
    // Load connector probe data
    let probe_data = load_probe_data(connector_name)?;
    
    // Build config
    let config = get_connector_config(connector_name, credentials)?;
    let client = ConnectorClient::new(config, None)?;
    
    // Build request from probe data
    let auth_request = build_authorize_request(&probe_data, "MANUAL")?;
    
    // Step 1: Authorize
    println!("\n[1/2] Authorizing...");
    let auth_response = client.authorize(auth_request, &HashMap::new(), None).await?;
    
    let status = auth_response.status();
    println!("Status: {:?}", status);
    println!("Connector Transaction ID: {:?}", auth_response.connector_transaction_id);
    
    // Handle status
    match status {
        PaymentStatus::Failure | PaymentStatus::AuthorizationFailed => {
            println!("❌ Payment declined");
            return Ok("failed".to_string());
        }
        PaymentStatus::Pending => {
            println!("⏳ Payment pending - awaiting async confirmation");
            println!("   (In production: wait for webhook, then poll get())");
            return Ok("pending".to_string());
        }
        PaymentStatus::Authorized => {
            println!("✅ Funds reserved successfully");
        }
        _ => {
            println!("⚠️  Unexpected status: {:?}", status);
            return Ok("error".to_string());
        }
    }
    
    // Step 2: Capture
    println!("\n[2/2] Capturing...");
    let capture_request = build_capture_request(
        auth_response.connector_transaction_id.as_deref().unwrap_or(""),
        auth_response.amount.as_ref().unwrap()
    );
    
    let capture_response = client.capture(capture_request, &HashMap::new(), None).await?;
    let capture_status = capture_response.status();
    println!("Capture Status: {:?}", capture_status);
    
    match capture_status {
        PaymentStatus::Captured | PaymentStatus::Authorized => {
            println!("✅ Payment captured successfully");
            Ok("success".to_string())
        }
        _ => {
            println!("⚠️  Capture returned: {:?}", capture_status);
            Ok("partial".to_string())
        }
    }
}
```

</details>


<details>
<summary><b>Revolv3</b></summary>

**Configuration:**
```rust
// Configuration not available
```

**Full Example:**
```rust
pub async fn process_checkout_card(
    connector_name: &str,
    credentials: &HashMap<String, String>,
) -> Result<String, Box<dyn Error>> {
    println!("\n{}", "=".repeat(60));
    println!("Processing Card Payment via {}", connector_name);
    println!("{}", "=".repeat(60));
    
    // Load connector probe data
    let probe_data = load_probe_data(connector_name)?;
    
    // Build config
    let config = get_connector_config(connector_name, credentials)?;
    let client = ConnectorClient::new(config, None)?;
    
    // Build request from probe data
    let auth_request = build_authorize_request(&probe_data, "MANUAL")?;
    
    // Step 1: Authorize
    println!("\n[1/2] Authorizing...");
    let auth_response = client.authorize(auth_request, &HashMap::new(), None).await?;
    
    let status = auth_response.status();
    println!("Status: {:?}", status);
    println!("Connector Transaction ID: {:?}", auth_response.connector_transaction_id);
    
    // Handle status
    match status {
        PaymentStatus::Failure | PaymentStatus::AuthorizationFailed => {
            println!("❌ Payment declined");
            return Ok("failed".to_string());
        }
        PaymentStatus::Pending => {
            println!("⏳ Payment pending - awaiting async confirmation");
            println!("   (In production: wait for webhook, then poll get())");
            return Ok("pending".to_string());
        }
        PaymentStatus::Authorized => {
            println!("✅ Funds reserved successfully");
        }
        _ => {
            println!("⚠️  Unexpected status: {:?}", status);
            return Ok("error".to_string());
        }
    }
    
    // Step 2: Capture
    println!("\n[2/2] Capturing...");
    let capture_request = build_capture_request(
        auth_response.connector_transaction_id.as_deref().unwrap_or(""),
        auth_response.amount.as_ref().unwrap()
    );
    
    let capture_response = client.capture(capture_request, &HashMap::new(), None).await?;
    let capture_status = capture_response.status();
    println!("Capture Status: {:?}", capture_status);
    
    match capture_status {
        PaymentStatus::Captured | PaymentStatus::Authorized => {
            println!("✅ Payment captured successfully");
            Ok("success".to_string())
        }
        _ => {
            println!("⚠️  Capture returned: {:?}", capture_status);
            Ok("partial".to_string())
        }
    }
}
```

</details>


<details>
<summary><b>Shift4</b></summary>

**Configuration:**
```rust
// Configuration not available
```

**Full Example:**
```rust
pub async fn process_checkout_card(
    connector_name: &str,
    credentials: &HashMap<String, String>,
) -> Result<String, Box<dyn Error>> {
    println!("\n{}", "=".repeat(60));
    println!("Processing Card Payment via {}", connector_name);
    println!("{}", "=".repeat(60));
    
    // Load connector probe data
    let probe_data = load_probe_data(connector_name)?;
    
    // Build config
    let config = get_connector_config(connector_name, credentials)?;
    let client = ConnectorClient::new(config, None)?;
    
    // Build request from probe data
    let auth_request = build_authorize_request(&probe_data, "MANUAL")?;
    
    // Step 1: Authorize
    println!("\n[1/2] Authorizing...");
    let auth_response = client.authorize(auth_request, &HashMap::new(), None).await?;
    
    let status = auth_response.status();
    println!("Status: {:?}", status);
    println!("Connector Transaction ID: {:?}", auth_response.connector_transaction_id);
    
    // Handle status
    match status {
        PaymentStatus::Failure | PaymentStatus::AuthorizationFailed => {
            println!("❌ Payment declined");
            return Ok("failed".to_string());
        }
        PaymentStatus::Pending => {
            println!("⏳ Payment pending - awaiting async confirmation");
            println!("   (In production: wait for webhook, then poll get())");
            return Ok("pending".to_string());
        }
        PaymentStatus::Authorized => {
            println!("✅ Funds reserved successfully");
        }
        _ => {
            println!("⚠️  Unexpected status: {:?}", status);
            return Ok("error".to_string());
        }
    }
    
    // Step 2: Capture
    println!("\n[2/2] Capturing...");
    let capture_request = build_capture_request(
        auth_response.connector_transaction_id.as_deref().unwrap_or(""),
        auth_response.amount.as_ref().unwrap()
    );
    
    let capture_response = client.capture(capture_request, &HashMap::new(), None).await?;
    let capture_status = capture_response.status();
    println!("Capture Status: {:?}", capture_status);
    
    match capture_status {
        PaymentStatus::Captured | PaymentStatus::Authorized => {
            println!("✅ Payment captured successfully");
            Ok("success".to_string())
        }
        _ => {
            println!("⚠️  Capture returned: {:?}", capture_status);
            Ok("partial".to_string())
        }
    }
}
```

</details>


<details>
<summary><b>Silverflow</b></summary>

**Configuration:**
```rust
// Configuration not available
```

**Full Example:**
```rust
pub async fn process_checkout_card(
    connector_name: &str,
    credentials: &HashMap<String, String>,
) -> Result<String, Box<dyn Error>> {
    println!("\n{}", "=".repeat(60));
    println!("Processing Card Payment via {}", connector_name);
    println!("{}", "=".repeat(60));
    
    // Load connector probe data
    let probe_data = load_probe_data(connector_name)?;
    
    // Build config
    let config = get_connector_config(connector_name, credentials)?;
    let client = ConnectorClient::new(config, None)?;
    
    // Build request from probe data
    let auth_request = build_authorize_request(&probe_data, "MANUAL")?;
    
    // Step 1: Authorize
    println!("\n[1/2] Authorizing...");
    let auth_response = client.authorize(auth_request, &HashMap::new(), None).await?;
    
    let status = auth_response.status();
    println!("Status: {:?}", status);
    println!("Connector Transaction ID: {:?}", auth_response.connector_transaction_id);
    
    // Handle status
    match status {
        PaymentStatus::Failure | PaymentStatus::AuthorizationFailed => {
            println!("❌ Payment declined");
            return Ok("failed".to_string());
        }
        PaymentStatus::Pending => {
            println!("⏳ Payment pending - awaiting async confirmation");
            println!("   (In production: wait for webhook, then poll get())");
            return Ok("pending".to_string());
        }
        PaymentStatus::Authorized => {
            println!("✅ Funds reserved successfully");
        }
        _ => {
            println!("⚠️  Unexpected status: {:?}", status);
            return Ok("error".to_string());
        }
    }
    
    // Step 2: Capture
    println!("\n[2/2] Capturing...");
    let capture_request = build_capture_request(
        auth_response.connector_transaction_id.as_deref().unwrap_or(""),
        auth_response.amount.as_ref().unwrap()
    );
    
    let capture_response = client.capture(capture_request, &HashMap::new(), None).await?;
    let capture_status = capture_response.status();
    println!("Capture Status: {:?}", capture_status);
    
    match capture_status {
        PaymentStatus::Captured | PaymentStatus::Authorized => {
            println!("✅ Payment captured successfully");
            Ok("success".to_string())
        }
        _ => {
            println!("⚠️  Capture returned: {:?}", capture_status);
            Ok("partial".to_string())
        }
    }
}
```

</details>


<details>
<summary><b>Stax</b></summary>

**Configuration:**
```rust
// Configuration not available
```

**Full Example:**
```rust
pub async fn process_checkout_card(
    connector_name: &str,
    credentials: &HashMap<String, String>,
) -> Result<String, Box<dyn Error>> {
    println!("\n{}", "=".repeat(60));
    println!("Processing Card Payment via {}", connector_name);
    println!("{}", "=".repeat(60));
    
    // Load connector probe data
    let probe_data = load_probe_data(connector_name)?;
    
    // Build config
    let config = get_connector_config(connector_name, credentials)?;
    let client = ConnectorClient::new(config, None)?;
    
    // Build request from probe data
    let auth_request = build_authorize_request(&probe_data, "MANUAL")?;
    
    // Step 1: Authorize
    println!("\n[1/2] Authorizing...");
    let auth_response = client.authorize(auth_request, &HashMap::new(), None).await?;
    
    let status = auth_response.status();
    println!("Status: {:?}", status);
    println!("Connector Transaction ID: {:?}", auth_response.connector_transaction_id);
    
    // Handle status
    match status {
        PaymentStatus::Failure | PaymentStatus::AuthorizationFailed => {
            println!("❌ Payment declined");
            return Ok("failed".to_string());
        }
        PaymentStatus::Pending => {
            println!("⏳ Payment pending - awaiting async confirmation");
            println!("   (In production: wait for webhook, then poll get())");
            return Ok("pending".to_string());
        }
        PaymentStatus::Authorized => {
            println!("✅ Funds reserved successfully");
        }
        _ => {
            println!("⚠️  Unexpected status: {:?}", status);
            return Ok("error".to_string());
        }
    }
    
    // Step 2: Capture
    println!("\n[2/2] Capturing...");
    let capture_request = build_capture_request(
        auth_response.connector_transaction_id.as_deref().unwrap_or(""),
        auth_response.amount.as_ref().unwrap()
    );
    
    let capture_response = client.capture(capture_request, &HashMap::new(), None).await?;
    let capture_status = capture_response.status();
    println!("Capture Status: {:?}", capture_status);
    
    match capture_status {
        PaymentStatus::Captured | PaymentStatus::Authorized => {
            println!("✅ Payment captured successfully");
            Ok("success".to_string())
        }
        _ => {
            println!("⚠️  Capture returned: {:?}", capture_status);
            Ok("partial".to_string())
        }
    }
}
```

</details>


<details>
<summary><b>Stripe</b></summary>

**Configuration:**
```rust
fn get_stripe_config(api_key: &str) -> ConnectorConfig {
    ConnectorConfig {
        connector: "stripe".to_string(),
        environment: Environment::Sandbox.into(),
        auth: ConnectorAuth::HeaderKey { 
            api_key: api_key.to_string().into() 
        },
        ..Default::default()
    }
}
```

**Full Example:**
```rust
pub async fn process_checkout_card(
    connector_name: &str,
    credentials: &HashMap<String, String>,
) -> Result<String, Box<dyn Error>> {
    println!("\n{}", "=".repeat(60));
    println!("Processing Card Payment via {}", connector_name);
    println!("{}", "=".repeat(60));
    
    // Load connector probe data
    let probe_data = load_probe_data(connector_name)?;
    
    // Build config
    let config = get_connector_config(connector_name, credentials)?;
    let client = ConnectorClient::new(config, None)?;
    
    // Build request from probe data
    let auth_request = build_authorize_request(&probe_data, "MANUAL")?;
    
    // Step 1: Authorize
    println!("\n[1/2] Authorizing...");
    let auth_response = client.authorize(auth_request, &HashMap::new(), None).await?;
    
    let status = auth_response.status();
    println!("Status: {:?}", status);
    println!("Connector Transaction ID: {:?}", auth_response.connector_transaction_id);
    
    // Handle status
    match status {
        PaymentStatus::Failure | PaymentStatus::AuthorizationFailed => {
            println!("❌ Payment declined");
            return Ok("failed".to_string());
        }
        PaymentStatus::Pending => {
            println!("⏳ Payment pending - awaiting async confirmation");
            println!("   (In production: wait for webhook, then poll get())");
            return Ok("pending".to_string());
        }
        PaymentStatus::Authorized => {
            println!("✅ Funds reserved successfully");
        }
        _ => {
            println!("⚠️  Unexpected status: {:?}", status);
            return Ok("error".to_string());
        }
    }
    
    // Step 2: Capture
    println!("\n[2/2] Capturing...");
    let capture_request = build_capture_request(
        auth_response.connector_transaction_id.as_deref().unwrap_or(""),
        auth_response.amount.as_ref().unwrap()
    );
    
    let capture_response = client.capture(capture_request, &HashMap::new(), None).await?;
    let capture_status = capture_response.status();
    println!("Capture Status: {:?}", capture_status);
    
    match capture_status {
        PaymentStatus::Captured | PaymentStatus::Authorized => {
            println!("✅ Payment captured successfully");
            Ok("success".to_string())
        }
        _ => {
            println!("⚠️  Capture returned: {:?}", capture_status);
            Ok("partial".to_string())
        }
    }
}
```

</details>


<details>
<summary><b>Trust Payments</b></summary>

**Configuration:**
```rust
// Configuration not available
```

**Full Example:**
```rust
pub async fn process_checkout_card(
    connector_name: &str,
    credentials: &HashMap<String, String>,
) -> Result<String, Box<dyn Error>> {
    println!("\n{}", "=".repeat(60));
    println!("Processing Card Payment via {}", connector_name);
    println!("{}", "=".repeat(60));
    
    // Load connector probe data
    let probe_data = load_probe_data(connector_name)?;
    
    // Build config
    let config = get_connector_config(connector_name, credentials)?;
    let client = ConnectorClient::new(config, None)?;
    
    // Build request from probe data
    let auth_request = build_authorize_request(&probe_data, "MANUAL")?;
    
    // Step 1: Authorize
    println!("\n[1/2] Authorizing...");
    let auth_response = client.authorize(auth_request, &HashMap::new(), None).await?;
    
    let status = auth_response.status();
    println!("Status: {:?}", status);
    println!("Connector Transaction ID: {:?}", auth_response.connector_transaction_id);
    
    // Handle status
    match status {
        PaymentStatus::Failure | PaymentStatus::AuthorizationFailed => {
            println!("❌ Payment declined");
            return Ok("failed".to_string());
        }
        PaymentStatus::Pending => {
            println!("⏳ Payment pending - awaiting async confirmation");
            println!("   (In production: wait for webhook, then poll get())");
            return Ok("pending".to_string());
        }
        PaymentStatus::Authorized => {
            println!("✅ Funds reserved successfully");
        }
        _ => {
            println!("⚠️  Unexpected status: {:?}", status);
            return Ok("error".to_string());
        }
    }
    
    // Step 2: Capture
    println!("\n[2/2] Capturing...");
    let capture_request = build_capture_request(
        auth_response.connector_transaction_id.as_deref().unwrap_or(""),
        auth_response.amount.as_ref().unwrap()
    );
    
    let capture_response = client.capture(capture_request, &HashMap::new(), None).await?;
    let capture_status = capture_response.status();
    println!("Capture Status: {:?}", capture_status);
    
    match capture_status {
        PaymentStatus::Captured | PaymentStatus::Authorized => {
            println!("✅ Payment captured successfully");
            Ok("success".to_string())
        }
        _ => {
            println!("⚠️  Capture returned: {:?}", capture_status);
            Ok("partial".to_string())
        }
    }
}
```

</details>


<details>
<summary><b>TSYS</b></summary>

**Configuration:**
```rust
// Configuration not available
```

**Full Example:**
```rust
pub async fn process_checkout_card(
    connector_name: &str,
    credentials: &HashMap<String, String>,
) -> Result<String, Box<dyn Error>> {
    println!("\n{}", "=".repeat(60));
    println!("Processing Card Payment via {}", connector_name);
    println!("{}", "=".repeat(60));
    
    // Load connector probe data
    let probe_data = load_probe_data(connector_name)?;
    
    // Build config
    let config = get_connector_config(connector_name, credentials)?;
    let client = ConnectorClient::new(config, None)?;
    
    // Build request from probe data
    let auth_request = build_authorize_request(&probe_data, "MANUAL")?;
    
    // Step 1: Authorize
    println!("\n[1/2] Authorizing...");
    let auth_response = client.authorize(auth_request, &HashMap::new(), None).await?;
    
    let status = auth_response.status();
    println!("Status: {:?}", status);
    println!("Connector Transaction ID: {:?}", auth_response.connector_transaction_id);
    
    // Handle status
    match status {
        PaymentStatus::Failure | PaymentStatus::AuthorizationFailed => {
            println!("❌ Payment declined");
            return Ok("failed".to_string());
        }
        PaymentStatus::Pending => {
            println!("⏳ Payment pending - awaiting async confirmation");
            println!("   (In production: wait for webhook, then poll get())");
            return Ok("pending".to_string());
        }
        PaymentStatus::Authorized => {
            println!("✅ Funds reserved successfully");
        }
        _ => {
            println!("⚠️  Unexpected status: {:?}", status);
            return Ok("error".to_string());
        }
    }
    
    // Step 2: Capture
    println!("\n[2/2] Capturing...");
    let capture_request = build_capture_request(
        auth_response.connector_transaction_id.as_deref().unwrap_or(""),
        auth_response.amount.as_ref().unwrap()
    );
    
    let capture_response = client.capture(capture_request, &HashMap::new(), None).await?;
    let capture_status = capture_response.status();
    println!("Capture Status: {:?}", capture_status);
    
    match capture_status {
        PaymentStatus::Captured | PaymentStatus::Authorized => {
            println!("✅ Payment captured successfully");
            Ok("success".to_string())
        }
        _ => {
            println!("⚠️  Capture returned: {:?}", capture_status);
            Ok("partial".to_string())
        }
    }
}
```

</details>


<details>
<summary><b>Wells Fargo</b></summary>

**Configuration:**
```rust
// Configuration not available
```

**Full Example:**
```rust
pub async fn process_checkout_card(
    connector_name: &str,
    credentials: &HashMap<String, String>,
) -> Result<String, Box<dyn Error>> {
    println!("\n{}", "=".repeat(60));
    println!("Processing Card Payment via {}", connector_name);
    println!("{}", "=".repeat(60));
    
    // Load connector probe data
    let probe_data = load_probe_data(connector_name)?;
    
    // Build config
    let config = get_connector_config(connector_name, credentials)?;
    let client = ConnectorClient::new(config, None)?;
    
    // Build request from probe data
    let auth_request = build_authorize_request(&probe_data, "MANUAL")?;
    
    // Step 1: Authorize
    println!("\n[1/2] Authorizing...");
    let auth_response = client.authorize(auth_request, &HashMap::new(), None).await?;
    
    let status = auth_response.status();
    println!("Status: {:?}", status);
    println!("Connector Transaction ID: {:?}", auth_response.connector_transaction_id);
    
    // Handle status
    match status {
        PaymentStatus::Failure | PaymentStatus::AuthorizationFailed => {
            println!("❌ Payment declined");
            return Ok("failed".to_string());
        }
        PaymentStatus::Pending => {
            println!("⏳ Payment pending - awaiting async confirmation");
            println!("   (In production: wait for webhook, then poll get())");
            return Ok("pending".to_string());
        }
        PaymentStatus::Authorized => {
            println!("✅ Funds reserved successfully");
        }
        _ => {
            println!("⚠️  Unexpected status: {:?}", status);
            return Ok("error".to_string());
        }
    }
    
    // Step 2: Capture
    println!("\n[2/2] Capturing...");
    let capture_request = build_capture_request(
        auth_response.connector_transaction_id.as_deref().unwrap_or(""),
        auth_response.amount.as_ref().unwrap()
    );
    
    let capture_response = client.capture(capture_request, &HashMap::new(), None).await?;
    let capture_status = capture_response.status();
    println!("Capture Status: {:?}", capture_status);
    
    match capture_status {
        PaymentStatus::Captured | PaymentStatus::Authorized => {
            println!("✅ Payment captured successfully");
            Ok("success".to_string())
        }
        _ => {
            println!("⚠️  Capture returned: {:?}", capture_status);
            Ok("partial".to_string())
        }
    }
}
```

</details>


<details>
<summary><b>Worldpay</b></summary>

**Configuration:**
```rust
// Configuration not available
```

**Full Example:**
```rust
pub async fn process_checkout_card(
    connector_name: &str,
    credentials: &HashMap<String, String>,
) -> Result<String, Box<dyn Error>> {
    println!("\n{}", "=".repeat(60));
    println!("Processing Card Payment via {}", connector_name);
    println!("{}", "=".repeat(60));
    
    // Load connector probe data
    let probe_data = load_probe_data(connector_name)?;
    
    // Build config
    let config = get_connector_config(connector_name, credentials)?;
    let client = ConnectorClient::new(config, None)?;
    
    // Build request from probe data
    let auth_request = build_authorize_request(&probe_data, "MANUAL")?;
    
    // Step 1: Authorize
    println!("\n[1/2] Authorizing...");
    let auth_response = client.authorize(auth_request, &HashMap::new(), None).await?;
    
    let status = auth_response.status();
    println!("Status: {:?}", status);
    println!("Connector Transaction ID: {:?}", auth_response.connector_transaction_id);
    
    // Handle status
    match status {
        PaymentStatus::Failure | PaymentStatus::AuthorizationFailed => {
            println!("❌ Payment declined");
            return Ok("failed".to_string());
        }
        PaymentStatus::Pending => {
            println!("⏳ Payment pending - awaiting async confirmation");
            println!("   (In production: wait for webhook, then poll get())");
            return Ok("pending".to_string());
        }
        PaymentStatus::Authorized => {
            println!("✅ Funds reserved successfully");
        }
        _ => {
            println!("⚠️  Unexpected status: {:?}", status);
            return Ok("error".to_string());
        }
    }
    
    // Step 2: Capture
    println!("\n[2/2] Capturing...");
    let capture_request = build_capture_request(
        auth_response.connector_transaction_id.as_deref().unwrap_or(""),
        auth_response.amount.as_ref().unwrap()
    );
    
    let capture_response = client.capture(capture_request, &HashMap::new(), None).await?;
    let capture_status = capture_response.status();
    println!("Capture Status: {:?}", capture_status);
    
    match capture_status {
        PaymentStatus::Captured | PaymentStatus::Authorized => {
            println!("✅ Payment captured successfully");
            Ok("success".to_string())
        }
        _ => {
            println!("⚠️  Capture returned: {:?}", capture_status);
            Ok("partial".to_string())
        }
    }
}
```

</details>


<details>
<summary><b>Worldpay Vantiv</b></summary>

**Configuration:**
```rust
// Configuration not available
```

**Full Example:**
```rust
pub async fn process_checkout_card(
    connector_name: &str,
    credentials: &HashMap<String, String>,
) -> Result<String, Box<dyn Error>> {
    println!("\n{}", "=".repeat(60));
    println!("Processing Card Payment via {}", connector_name);
    println!("{}", "=".repeat(60));
    
    // Load connector probe data
    let probe_data = load_probe_data(connector_name)?;
    
    // Build config
    let config = get_connector_config(connector_name, credentials)?;
    let client = ConnectorClient::new(config, None)?;
    
    // Build request from probe data
    let auth_request = build_authorize_request(&probe_data, "MANUAL")?;
    
    // Step 1: Authorize
    println!("\n[1/2] Authorizing...");
    let auth_response = client.authorize(auth_request, &HashMap::new(), None).await?;
    
    let status = auth_response.status();
    println!("Status: {:?}", status);
    println!("Connector Transaction ID: {:?}", auth_response.connector_transaction_id);
    
    // Handle status
    match status {
        PaymentStatus::Failure | PaymentStatus::AuthorizationFailed => {
            println!("❌ Payment declined");
            return Ok("failed".to_string());
        }
        PaymentStatus::Pending => {
            println!("⏳ Payment pending - awaiting async confirmation");
            println!("   (In production: wait for webhook, then poll get())");
            return Ok("pending".to_string());
        }
        PaymentStatus::Authorized => {
            println!("✅ Funds reserved successfully");
        }
        _ => {
            println!("⚠️  Unexpected status: {:?}", status);
            return Ok("error".to_string());
        }
    }
    
    // Step 2: Capture
    println!("\n[2/2] Capturing...");
    let capture_request = build_capture_request(
        auth_response.connector_transaction_id.as_deref().unwrap_or(""),
        auth_response.amount.as_ref().unwrap()
    );
    
    let capture_response = client.capture(capture_request, &HashMap::new(), None).await?;
    let capture_status = capture_response.status();
    println!("Capture Status: {:?}", capture_status);
    
    match capture_status {
        PaymentStatus::Captured | PaymentStatus::Authorized => {
            println!("✅ Payment captured successfully");
            Ok("success".to_string())
        }
        _ => {
            println!("⚠️  Capture returned: {:?}", capture_status);
            Ok("partial".to_string())
        }
    }
}
```

</details>


<details>
<summary><b>Worldpay XML</b></summary>

**Configuration:**
```rust
// Configuration not available
```

**Full Example:**
```rust
pub async fn process_checkout_card(
    connector_name: &str,
    credentials: &HashMap<String, String>,
) -> Result<String, Box<dyn Error>> {
    println!("\n{}", "=".repeat(60));
    println!("Processing Card Payment via {}", connector_name);
    println!("{}", "=".repeat(60));
    
    // Load connector probe data
    let probe_data = load_probe_data(connector_name)?;
    
    // Build config
    let config = get_connector_config(connector_name, credentials)?;
    let client = ConnectorClient::new(config, None)?;
    
    // Build request from probe data
    let auth_request = build_authorize_request(&probe_data, "MANUAL")?;
    
    // Step 1: Authorize
    println!("\n[1/2] Authorizing...");
    let auth_response = client.authorize(auth_request, &HashMap::new(), None).await?;
    
    let status = auth_response.status();
    println!("Status: {:?}", status);
    println!("Connector Transaction ID: {:?}", auth_response.connector_transaction_id);
    
    // Handle status
    match status {
        PaymentStatus::Failure | PaymentStatus::AuthorizationFailed => {
            println!("❌ Payment declined");
            return Ok("failed".to_string());
        }
        PaymentStatus::Pending => {
            println!("⏳ Payment pending - awaiting async confirmation");
            println!("   (In production: wait for webhook, then poll get())");
            return Ok("pending".to_string());
        }
        PaymentStatus::Authorized => {
            println!("✅ Funds reserved successfully");
        }
        _ => {
            println!("⚠️  Unexpected status: {:?}", status);
            return Ok("error".to_string());
        }
    }
    
    // Step 2: Capture
    println!("\n[2/2] Capturing...");
    let capture_request = build_capture_request(
        auth_response.connector_transaction_id.as_deref().unwrap_or(""),
        auth_response.amount.as_ref().unwrap()
    );
    
    let capture_response = client.capture(capture_request, &HashMap::new(), None).await?;
    let capture_status = capture_response.status();
    println!("Capture Status: {:?}", capture_status);
    
    match capture_status {
        PaymentStatus::Captured | PaymentStatus::Authorized => {
            println!("✅ Payment captured successfully");
            Ok("success".to_string())
        }
        _ => {
            println!("⚠️  Capture returned: {:?}", capture_status);
            Ok("partial".to_string())
        }
    }
}
```

</details>


<details>
<summary><b>Xendit</b></summary>

**Configuration:**
```rust
// Configuration not available
```

**Full Example:**
```rust
pub async fn process_checkout_card(
    connector_name: &str,
    credentials: &HashMap<String, String>,
) -> Result<String, Box<dyn Error>> {
    println!("\n{}", "=".repeat(60));
    println!("Processing Card Payment via {}", connector_name);
    println!("{}", "=".repeat(60));
    
    // Load connector probe data
    let probe_data = load_probe_data(connector_name)?;
    
    // Build config
    let config = get_connector_config(connector_name, credentials)?;
    let client = ConnectorClient::new(config, None)?;
    
    // Build request from probe data
    let auth_request = build_authorize_request(&probe_data, "MANUAL")?;
    
    // Step 1: Authorize
    println!("\n[1/2] Authorizing...");
    let auth_response = client.authorize(auth_request, &HashMap::new(), None).await?;
    
    let status = auth_response.status();
    println!("Status: {:?}", status);
    println!("Connector Transaction ID: {:?}", auth_response.connector_transaction_id);
    
    // Handle status
    match status {
        PaymentStatus::Failure | PaymentStatus::AuthorizationFailed => {
            println!("❌ Payment declined");
            return Ok("failed".to_string());
        }
        PaymentStatus::Pending => {
            println!("⏳ Payment pending - awaiting async confirmation");
            println!("   (In production: wait for webhook, then poll get())");
            return Ok("pending".to_string());
        }
        PaymentStatus::Authorized => {
            println!("✅ Funds reserved successfully");
        }
        _ => {
            println!("⚠️  Unexpected status: {:?}", status);
            return Ok("error".to_string());
        }
    }
    
    // Step 2: Capture
    println!("\n[2/2] Capturing...");
    let capture_request = build_capture_request(
        auth_response.connector_transaction_id.as_deref().unwrap_or(""),
        auth_response.amount.as_ref().unwrap()
    );
    
    let capture_response = client.capture(capture_request, &HashMap::new(), None).await?;
    let capture_status = capture_response.status();
    println!("Capture Status: {:?}", capture_status);
    
    match capture_status {
        PaymentStatus::Captured | PaymentStatus::Authorized => {
            println!("✅ Payment captured successfully");
            Ok("success".to_string())
        }
        _ => {
            println!("⚠️  Capture returned: {:?}", capture_status);
            Ok("partial".to_string())
        }
    }
}
```

</details>


<details>
<summary><b>Zift</b></summary>

**Configuration:**
```rust
// Configuration not available
```

**Full Example:**
```rust
pub async fn process_checkout_card(
    connector_name: &str,
    credentials: &HashMap<String, String>,
) -> Result<String, Box<dyn Error>> {
    println!("\n{}", "=".repeat(60));
    println!("Processing Card Payment via {}", connector_name);
    println!("{}", "=".repeat(60));
    
    // Load connector probe data
    let probe_data = load_probe_data(connector_name)?;
    
    // Build config
    let config = get_connector_config(connector_name, credentials)?;
    let client = ConnectorClient::new(config, None)?;
    
    // Build request from probe data
    let auth_request = build_authorize_request(&probe_data, "MANUAL")?;
    
    // Step 1: Authorize
    println!("\n[1/2] Authorizing...");
    let auth_response = client.authorize(auth_request, &HashMap::new(), None).await?;
    
    let status = auth_response.status();
    println!("Status: {:?}", status);
    println!("Connector Transaction ID: {:?}", auth_response.connector_transaction_id);
    
    // Handle status
    match status {
        PaymentStatus::Failure | PaymentStatus::AuthorizationFailed => {
            println!("❌ Payment declined");
            return Ok("failed".to_string());
        }
        PaymentStatus::Pending => {
            println!("⏳ Payment pending - awaiting async confirmation");
            println!("   (In production: wait for webhook, then poll get())");
            return Ok("pending".to_string());
        }
        PaymentStatus::Authorized => {
            println!("✅ Funds reserved successfully");
        }
        _ => {
            println!("⚠️  Unexpected status: {:?}", status);
            return Ok("error".to_string());
        }
    }
    
    // Step 2: Capture
    println!("\n[2/2] Capturing...");
    let capture_request = build_capture_request(
        auth_response.connector_transaction_id.as_deref().unwrap_or(""),
        auth_response.amount.as_ref().unwrap()
    );
    
    let capture_response = client.capture(capture_request, &HashMap::new(), None).await?;
    let capture_status = capture_response.status();
    println!("Capture Status: {:?}", capture_status);
    
    match capture_status {
        PaymentStatus::Captured | PaymentStatus::Authorized => {
            println!("✅ Payment captured successfully");
            Ok("success".to_string())
        }
        _ => {
            println!("⚠️  Capture returned: {:?}", capture_status);
            Ok("partial".to_string())
        }
    }
}
```

</details>



### JavaScript


<details>
<summary><b>ACI</b></summary>

**Configuration:**
```javascript
// Configuration not available
```

**Full Example:**
```javascript
async function processCheckoutCard(connectorName, credentials) {
    console.log('\n' + '='.repeat(60));
    console.log(`Processing Card Payment via ${connectorName}`);
    console.log('='.repeat(60));
    
    // Load probe data for this connector
    const probeData = loadProbeData(connectorName);
    
    // Get configuration
    const config = getConnectorConfig(connectorName, credentials);
    const client = new PaymentClient(config);
    
    // Build authorize request
    const authRequest = buildAuthorizeRequest(probeData, 'MANUAL');
    
    console.log('\nRequest:', JSON.stringify(authRequest, null, 2));
    
    // Step 1: Authorize
    console.log('\n[1/2] Authorizing...');
    const authResponse = await client.authorize(authRequest);
    
    console.log(`Status: ${authResponse.status}`);
    console.log(`Connector Transaction ID: ${authResponse.connectorTransactionId}`);
    
    // Handle status
    if (authResponse.status === 'FAILED') {
        console.log(`❌ Payment declined: ${authResponse.errorMessage || 'Unknown error'}`);
        return 'failed';
    }
    
    if (authResponse.status === 'PENDING') {
        console.log('⏳ Payment pending - awaiting async confirmation');
        console.log('   (In production: wait for webhook, then poll get())');
        return 'pending';
    }
    
    if (authResponse.status !== 'AUTHORIZED') {
        console.log(`⚠️  Unexpected status: ${authResponse.status}`);
        return 'error';
    }
    
    console.log('✅ Funds reserved successfully');
    
    // Step 2: Capture
    console.log('\n[2/2] Capturing...');
    const captureRequest = buildCaptureRequest(
        authResponse.connectorTransactionId,
        authRequest.amount
    );
    
    const captureResponse = await client.capture(captureRequest);
    console.log(`Capture Status: ${captureResponse.status}`);
    
    if (captureResponse.status === 'CAPTURED' || captureResponse.status === 'AUTHORIZED') {
        console.log('✅ Payment captured successfully');
        return 'success';
    } else {
        console.log(`⚠️  Capture returned: ${captureResponse.status}`);
        return 'partial';
    }
}
```

</details>


<details>
<summary><b>Adyen</b></summary>

**Configuration:**
```javascript
function getAdyenConfig(apiKey, merchantAccount) {
    const config = ConnectorConfig.create({
        options: SdkOptions.create({ environment: Environment.SANDBOX }),
    });
    config.connectorConfig = ConnectorSpecificConfig.create({
        adyen: { 
            apiKey: { value: apiKey },
            merchantAccount: { value: merchantAccount }
        }
    });
    return config;
}
```

**Full Example:**
```javascript
async function processCheckoutCard(connectorName, credentials) {
    console.log('\n' + '='.repeat(60));
    console.log(`Processing Card Payment via ${connectorName}`);
    console.log('='.repeat(60));
    
    // Load probe data for this connector
    const probeData = loadProbeData(connectorName);
    
    // Get configuration
    const config = getConnectorConfig(connectorName, credentials);
    const client = new PaymentClient(config);
    
    // Build authorize request
    const authRequest = buildAuthorizeRequest(probeData, 'MANUAL');
    
    console.log('\nRequest:', JSON.stringify(authRequest, null, 2));
    
    // Step 1: Authorize
    console.log('\n[1/2] Authorizing...');
    const authResponse = await client.authorize(authRequest);
    
    console.log(`Status: ${authResponse.status}`);
    console.log(`Connector Transaction ID: ${authResponse.connectorTransactionId}`);
    
    // Handle status
    if (authResponse.status === 'FAILED') {
        console.log(`❌ Payment declined: ${authResponse.errorMessage || 'Unknown error'}`);
        return 'failed';
    }
    
    if (authResponse.status === 'PENDING') {
        console.log('⏳ Payment pending - awaiting async confirmation');
        console.log('   (In production: wait for webhook, then poll get())');
        return 'pending';
    }
    
    if (authResponse.status !== 'AUTHORIZED') {
        console.log(`⚠️  Unexpected status: ${authResponse.status}`);
        return 'error';
    }
    
    console.log('✅ Funds reserved successfully');
    
    // Step 2: Capture
    console.log('\n[2/2] Capturing...');
    const captureRequest = buildCaptureRequest(
        authResponse.connectorTransactionId,
        authRequest.amount
    );
    
    const captureResponse = await client.capture(captureRequest);
    console.log(`Capture Status: ${captureResponse.status}`);
    
    if (captureResponse.status === 'CAPTURED' || captureResponse.status === 'AUTHORIZED') {
        console.log('✅ Payment captured successfully');
        return 'success';
    } else {
        console.log(`⚠️  Capture returned: ${captureResponse.status}`);
        return 'partial';
    }
}
```

</details>


<details>
<summary><b>Airwallex</b></summary>

**Configuration:**
```javascript
// Configuration not available
```

**Full Example:**
```javascript
async function processCheckoutCard(connectorName, credentials) {
    console.log('\n' + '='.repeat(60));
    console.log(`Processing Card Payment via ${connectorName}`);
    console.log('='.repeat(60));
    
    // Load probe data for this connector
    const probeData = loadProbeData(connectorName);
    
    // Get configuration
    const config = getConnectorConfig(connectorName, credentials);
    const client = new PaymentClient(config);
    
    // Build authorize request
    const authRequest = buildAuthorizeRequest(probeData, 'MANUAL');
    
    console.log('\nRequest:', JSON.stringify(authRequest, null, 2));
    
    // Step 1: Authorize
    console.log('\n[1/2] Authorizing...');
    const authResponse = await client.authorize(authRequest);
    
    console.log(`Status: ${authResponse.status}`);
    console.log(`Connector Transaction ID: ${authResponse.connectorTransactionId}`);
    
    // Handle status
    if (authResponse.status === 'FAILED') {
        console.log(`❌ Payment declined: ${authResponse.errorMessage || 'Unknown error'}`);
        return 'failed';
    }
    
    if (authResponse.status === 'PENDING') {
        console.log('⏳ Payment pending - awaiting async confirmation');
        console.log('   (In production: wait for webhook, then poll get())');
        return 'pending';
    }
    
    if (authResponse.status !== 'AUTHORIZED') {
        console.log(`⚠️  Unexpected status: ${authResponse.status}`);
        return 'error';
    }
    
    console.log('✅ Funds reserved successfully');
    
    // Step 2: Capture
    console.log('\n[2/2] Capturing...');
    const captureRequest = buildCaptureRequest(
        authResponse.connectorTransactionId,
        authRequest.amount
    );
    
    const captureResponse = await client.capture(captureRequest);
    console.log(`Capture Status: ${captureResponse.status}`);
    
    if (captureResponse.status === 'CAPTURED' || captureResponse.status === 'AUTHORIZED') {
        console.log('✅ Payment captured successfully');
        return 'success';
    } else {
        console.log(`⚠️  Capture returned: ${captureResponse.status}`);
        return 'partial';
    }
}
```

</details>


<details>
<summary><b>authipay</b></summary>

**Configuration:**
```javascript
// Configuration not available
```

**Full Example:**
```javascript
async function processCheckoutCard(connectorName, credentials) {
    console.log('\n' + '='.repeat(60));
    console.log(`Processing Card Payment via ${connectorName}`);
    console.log('='.repeat(60));
    
    // Load probe data for this connector
    const probeData = loadProbeData(connectorName);
    
    // Get configuration
    const config = getConnectorConfig(connectorName, credentials);
    const client = new PaymentClient(config);
    
    // Build authorize request
    const authRequest = buildAuthorizeRequest(probeData, 'MANUAL');
    
    console.log('\nRequest:', JSON.stringify(authRequest, null, 2));
    
    // Step 1: Authorize
    console.log('\n[1/2] Authorizing...');
    const authResponse = await client.authorize(authRequest);
    
    console.log(`Status: ${authResponse.status}`);
    console.log(`Connector Transaction ID: ${authResponse.connectorTransactionId}`);
    
    // Handle status
    if (authResponse.status === 'FAILED') {
        console.log(`❌ Payment declined: ${authResponse.errorMessage || 'Unknown error'}`);
        return 'failed';
    }
    
    if (authResponse.status === 'PENDING') {
        console.log('⏳ Payment pending - awaiting async confirmation');
        console.log('   (In production: wait for webhook, then poll get())');
        return 'pending';
    }
    
    if (authResponse.status !== 'AUTHORIZED') {
        console.log(`⚠️  Unexpected status: ${authResponse.status}`);
        return 'error';
    }
    
    console.log('✅ Funds reserved successfully');
    
    // Step 2: Capture
    console.log('\n[2/2] Capturing...');
    const captureRequest = buildCaptureRequest(
        authResponse.connectorTransactionId,
        authRequest.amount
    );
    
    const captureResponse = await client.capture(captureRequest);
    console.log(`Capture Status: ${captureResponse.status}`);
    
    if (captureResponse.status === 'CAPTURED' || captureResponse.status === 'AUTHORIZED') {
        console.log('✅ Payment captured successfully');
        return 'success';
    } else {
        console.log(`⚠️  Capture returned: ${captureResponse.status}`);
        return 'partial';
    }
}
```

</details>


<details>
<summary><b>Authorize.net</b></summary>

**Configuration:**
```javascript
// Configuration not available
```

**Full Example:**
```javascript
async function processCheckoutCard(connectorName, credentials) {
    console.log('\n' + '='.repeat(60));
    console.log(`Processing Card Payment via ${connectorName}`);
    console.log('='.repeat(60));
    
    // Load probe data for this connector
    const probeData = loadProbeData(connectorName);
    
    // Get configuration
    const config = getConnectorConfig(connectorName, credentials);
    const client = new PaymentClient(config);
    
    // Build authorize request
    const authRequest = buildAuthorizeRequest(probeData, 'MANUAL');
    
    console.log('\nRequest:', JSON.stringify(authRequest, null, 2));
    
    // Step 1: Authorize
    console.log('\n[1/2] Authorizing...');
    const authResponse = await client.authorize(authRequest);
    
    console.log(`Status: ${authResponse.status}`);
    console.log(`Connector Transaction ID: ${authResponse.connectorTransactionId}`);
    
    // Handle status
    if (authResponse.status === 'FAILED') {
        console.log(`❌ Payment declined: ${authResponse.errorMessage || 'Unknown error'}`);
        return 'failed';
    }
    
    if (authResponse.status === 'PENDING') {
        console.log('⏳ Payment pending - awaiting async confirmation');
        console.log('   (In production: wait for webhook, then poll get())');
        return 'pending';
    }
    
    if (authResponse.status !== 'AUTHORIZED') {
        console.log(`⚠️  Unexpected status: ${authResponse.status}`);
        return 'error';
    }
    
    console.log('✅ Funds reserved successfully');
    
    // Step 2: Capture
    console.log('\n[2/2] Capturing...');
    const captureRequest = buildCaptureRequest(
        authResponse.connectorTransactionId,
        authRequest.amount
    );
    
    const captureResponse = await client.capture(captureRequest);
    console.log(`Capture Status: ${captureResponse.status}`);
    
    if (captureResponse.status === 'CAPTURED' || captureResponse.status === 'AUTHORIZED') {
        console.log('✅ Payment captured successfully');
        return 'success';
    } else {
        console.log(`⚠️  Capture returned: ${captureResponse.status}`);
        return 'partial';
    }
}
```

</details>


<details>
<summary><b>Bambora</b></summary>

**Configuration:**
```javascript
// Configuration not available
```

**Full Example:**
```javascript
async function processCheckoutCard(connectorName, credentials) {
    console.log('\n' + '='.repeat(60));
    console.log(`Processing Card Payment via ${connectorName}`);
    console.log('='.repeat(60));
    
    // Load probe data for this connector
    const probeData = loadProbeData(connectorName);
    
    // Get configuration
    const config = getConnectorConfig(connectorName, credentials);
    const client = new PaymentClient(config);
    
    // Build authorize request
    const authRequest = buildAuthorizeRequest(probeData, 'MANUAL');
    
    console.log('\nRequest:', JSON.stringify(authRequest, null, 2));
    
    // Step 1: Authorize
    console.log('\n[1/2] Authorizing...');
    const authResponse = await client.authorize(authRequest);
    
    console.log(`Status: ${authResponse.status}`);
    console.log(`Connector Transaction ID: ${authResponse.connectorTransactionId}`);
    
    // Handle status
    if (authResponse.status === 'FAILED') {
        console.log(`❌ Payment declined: ${authResponse.errorMessage || 'Unknown error'}`);
        return 'failed';
    }
    
    if (authResponse.status === 'PENDING') {
        console.log('⏳ Payment pending - awaiting async confirmation');
        console.log('   (In production: wait for webhook, then poll get())');
        return 'pending';
    }
    
    if (authResponse.status !== 'AUTHORIZED') {
        console.log(`⚠️  Unexpected status: ${authResponse.status}`);
        return 'error';
    }
    
    console.log('✅ Funds reserved successfully');
    
    // Step 2: Capture
    console.log('\n[2/2] Capturing...');
    const captureRequest = buildCaptureRequest(
        authResponse.connectorTransactionId,
        authRequest.amount
    );
    
    const captureResponse = await client.capture(captureRequest);
    console.log(`Capture Status: ${captureResponse.status}`);
    
    if (captureResponse.status === 'CAPTURED' || captureResponse.status === 'AUTHORIZED') {
        console.log('✅ Payment captured successfully');
        return 'success';
    } else {
        console.log(`⚠️  Capture returned: ${captureResponse.status}`);
        return 'partial';
    }
}
```

</details>


<details>
<summary><b>Bambora APAC</b></summary>

**Configuration:**
```javascript
// Configuration not available
```

**Full Example:**
```javascript
async function processCheckoutCard(connectorName, credentials) {
    console.log('\n' + '='.repeat(60));
    console.log(`Processing Card Payment via ${connectorName}`);
    console.log('='.repeat(60));
    
    // Load probe data for this connector
    const probeData = loadProbeData(connectorName);
    
    // Get configuration
    const config = getConnectorConfig(connectorName, credentials);
    const client = new PaymentClient(config);
    
    // Build authorize request
    const authRequest = buildAuthorizeRequest(probeData, 'MANUAL');
    
    console.log('\nRequest:', JSON.stringify(authRequest, null, 2));
    
    // Step 1: Authorize
    console.log('\n[1/2] Authorizing...');
    const authResponse = await client.authorize(authRequest);
    
    console.log(`Status: ${authResponse.status}`);
    console.log(`Connector Transaction ID: ${authResponse.connectorTransactionId}`);
    
    // Handle status
    if (authResponse.status === 'FAILED') {
        console.log(`❌ Payment declined: ${authResponse.errorMessage || 'Unknown error'}`);
        return 'failed';
    }
    
    if (authResponse.status === 'PENDING') {
        console.log('⏳ Payment pending - awaiting async confirmation');
        console.log('   (In production: wait for webhook, then poll get())');
        return 'pending';
    }
    
    if (authResponse.status !== 'AUTHORIZED') {
        console.log(`⚠️  Unexpected status: ${authResponse.status}`);
        return 'error';
    }
    
    console.log('✅ Funds reserved successfully');
    
    // Step 2: Capture
    console.log('\n[2/2] Capturing...');
    const captureRequest = buildCaptureRequest(
        authResponse.connectorTransactionId,
        authRequest.amount
    );
    
    const captureResponse = await client.capture(captureRequest);
    console.log(`Capture Status: ${captureResponse.status}`);
    
    if (captureResponse.status === 'CAPTURED' || captureResponse.status === 'AUTHORIZED') {
        console.log('✅ Payment captured successfully');
        return 'success';
    } else {
        console.log(`⚠️  Capture returned: ${captureResponse.status}`);
        return 'partial';
    }
}
```

</details>


<details>
<summary><b>Bank of America</b></summary>

**Configuration:**
```javascript
// Configuration not available
```

**Full Example:**
```javascript
async function processCheckoutCard(connectorName, credentials) {
    console.log('\n' + '='.repeat(60));
    console.log(`Processing Card Payment via ${connectorName}`);
    console.log('='.repeat(60));
    
    // Load probe data for this connector
    const probeData = loadProbeData(connectorName);
    
    // Get configuration
    const config = getConnectorConfig(connectorName, credentials);
    const client = new PaymentClient(config);
    
    // Build authorize request
    const authRequest = buildAuthorizeRequest(probeData, 'MANUAL');
    
    console.log('\nRequest:', JSON.stringify(authRequest, null, 2));
    
    // Step 1: Authorize
    console.log('\n[1/2] Authorizing...');
    const authResponse = await client.authorize(authRequest);
    
    console.log(`Status: ${authResponse.status}`);
    console.log(`Connector Transaction ID: ${authResponse.connectorTransactionId}`);
    
    // Handle status
    if (authResponse.status === 'FAILED') {
        console.log(`❌ Payment declined: ${authResponse.errorMessage || 'Unknown error'}`);
        return 'failed';
    }
    
    if (authResponse.status === 'PENDING') {
        console.log('⏳ Payment pending - awaiting async confirmation');
        console.log('   (In production: wait for webhook, then poll get())');
        return 'pending';
    }
    
    if (authResponse.status !== 'AUTHORIZED') {
        console.log(`⚠️  Unexpected status: ${authResponse.status}`);
        return 'error';
    }
    
    console.log('✅ Funds reserved successfully');
    
    // Step 2: Capture
    console.log('\n[2/2] Capturing...');
    const captureRequest = buildCaptureRequest(
        authResponse.connectorTransactionId,
        authRequest.amount
    );
    
    const captureResponse = await client.capture(captureRequest);
    console.log(`Capture Status: ${captureResponse.status}`);
    
    if (captureResponse.status === 'CAPTURED' || captureResponse.status === 'AUTHORIZED') {
        console.log('✅ Payment captured successfully');
        return 'success';
    } else {
        console.log(`⚠️  Capture returned: ${captureResponse.status}`);
        return 'partial';
    }
}
```

</details>


<details>
<summary><b>Barclaycard</b></summary>

**Configuration:**
```javascript
// Configuration not available
```

**Full Example:**
```javascript
async function processCheckoutCard(connectorName, credentials) {
    console.log('\n' + '='.repeat(60));
    console.log(`Processing Card Payment via ${connectorName}`);
    console.log('='.repeat(60));
    
    // Load probe data for this connector
    const probeData = loadProbeData(connectorName);
    
    // Get configuration
    const config = getConnectorConfig(connectorName, credentials);
    const client = new PaymentClient(config);
    
    // Build authorize request
    const authRequest = buildAuthorizeRequest(probeData, 'MANUAL');
    
    console.log('\nRequest:', JSON.stringify(authRequest, null, 2));
    
    // Step 1: Authorize
    console.log('\n[1/2] Authorizing...');
    const authResponse = await client.authorize(authRequest);
    
    console.log(`Status: ${authResponse.status}`);
    console.log(`Connector Transaction ID: ${authResponse.connectorTransactionId}`);
    
    // Handle status
    if (authResponse.status === 'FAILED') {
        console.log(`❌ Payment declined: ${authResponse.errorMessage || 'Unknown error'}`);
        return 'failed';
    }
    
    if (authResponse.status === 'PENDING') {
        console.log('⏳ Payment pending - awaiting async confirmation');
        console.log('   (In production: wait for webhook, then poll get())');
        return 'pending';
    }
    
    if (authResponse.status !== 'AUTHORIZED') {
        console.log(`⚠️  Unexpected status: ${authResponse.status}`);
        return 'error';
    }
    
    console.log('✅ Funds reserved successfully');
    
    // Step 2: Capture
    console.log('\n[2/2] Capturing...');
    const captureRequest = buildCaptureRequest(
        authResponse.connectorTransactionId,
        authRequest.amount
    );
    
    const captureResponse = await client.capture(captureRequest);
    console.log(`Capture Status: ${captureResponse.status}`);
    
    if (captureResponse.status === 'CAPTURED' || captureResponse.status === 'AUTHORIZED') {
        console.log('✅ Payment captured successfully');
        return 'success';
    } else {
        console.log(`⚠️  Capture returned: ${captureResponse.status}`);
        return 'partial';
    }
}
```

</details>


<details>
<summary><b>Billwerk</b></summary>

**Configuration:**
```javascript
// Configuration not available
```

**Full Example:**
```javascript
async function processCheckoutCard(connectorName, credentials) {
    console.log('\n' + '='.repeat(60));
    console.log(`Processing Card Payment via ${connectorName}`);
    console.log('='.repeat(60));
    
    // Load probe data for this connector
    const probeData = loadProbeData(connectorName);
    
    // Get configuration
    const config = getConnectorConfig(connectorName, credentials);
    const client = new PaymentClient(config);
    
    // Build authorize request
    const authRequest = buildAuthorizeRequest(probeData, 'MANUAL');
    
    console.log('\nRequest:', JSON.stringify(authRequest, null, 2));
    
    // Step 1: Authorize
    console.log('\n[1/2] Authorizing...');
    const authResponse = await client.authorize(authRequest);
    
    console.log(`Status: ${authResponse.status}`);
    console.log(`Connector Transaction ID: ${authResponse.connectorTransactionId}`);
    
    // Handle status
    if (authResponse.status === 'FAILED') {
        console.log(`❌ Payment declined: ${authResponse.errorMessage || 'Unknown error'}`);
        return 'failed';
    }
    
    if (authResponse.status === 'PENDING') {
        console.log('⏳ Payment pending - awaiting async confirmation');
        console.log('   (In production: wait for webhook, then poll get())');
        return 'pending';
    }
    
    if (authResponse.status !== 'AUTHORIZED') {
        console.log(`⚠️  Unexpected status: ${authResponse.status}`);
        return 'error';
    }
    
    console.log('✅ Funds reserved successfully');
    
    // Step 2: Capture
    console.log('\n[2/2] Capturing...');
    const captureRequest = buildCaptureRequest(
        authResponse.connectorTransactionId,
        authRequest.amount
    );
    
    const captureResponse = await client.capture(captureRequest);
    console.log(`Capture Status: ${captureResponse.status}`);
    
    if (captureResponse.status === 'CAPTURED' || captureResponse.status === 'AUTHORIZED') {
        console.log('✅ Payment captured successfully');
        return 'success';
    } else {
        console.log(`⚠️  Capture returned: ${captureResponse.status}`);
        return 'partial';
    }
}
```

</details>


<details>
<summary><b>BlueSnap</b></summary>

**Configuration:**
```javascript
// Configuration not available
```

**Full Example:**
```javascript
async function processCheckoutCard(connectorName, credentials) {
    console.log('\n' + '='.repeat(60));
    console.log(`Processing Card Payment via ${connectorName}`);
    console.log('='.repeat(60));
    
    // Load probe data for this connector
    const probeData = loadProbeData(connectorName);
    
    // Get configuration
    const config = getConnectorConfig(connectorName, credentials);
    const client = new PaymentClient(config);
    
    // Build authorize request
    const authRequest = buildAuthorizeRequest(probeData, 'MANUAL');
    
    console.log('\nRequest:', JSON.stringify(authRequest, null, 2));
    
    // Step 1: Authorize
    console.log('\n[1/2] Authorizing...');
    const authResponse = await client.authorize(authRequest);
    
    console.log(`Status: ${authResponse.status}`);
    console.log(`Connector Transaction ID: ${authResponse.connectorTransactionId}`);
    
    // Handle status
    if (authResponse.status === 'FAILED') {
        console.log(`❌ Payment declined: ${authResponse.errorMessage || 'Unknown error'}`);
        return 'failed';
    }
    
    if (authResponse.status === 'PENDING') {
        console.log('⏳ Payment pending - awaiting async confirmation');
        console.log('   (In production: wait for webhook, then poll get())');
        return 'pending';
    }
    
    if (authResponse.status !== 'AUTHORIZED') {
        console.log(`⚠️  Unexpected status: ${authResponse.status}`);
        return 'error';
    }
    
    console.log('✅ Funds reserved successfully');
    
    // Step 2: Capture
    console.log('\n[2/2] Capturing...');
    const captureRequest = buildCaptureRequest(
        authResponse.connectorTransactionId,
        authRequest.amount
    );
    
    const captureResponse = await client.capture(captureRequest);
    console.log(`Capture Status: ${captureResponse.status}`);
    
    if (captureResponse.status === 'CAPTURED' || captureResponse.status === 'AUTHORIZED') {
        console.log('✅ Payment captured successfully');
        return 'success';
    } else {
        console.log(`⚠️  Capture returned: ${captureResponse.status}`);
        return 'partial';
    }
}
```

</details>


<details>
<summary><b>Braintree</b></summary>

**Configuration:**
```javascript
// Configuration not available
```

**Full Example:**
```javascript
async function processCheckoutCard(connectorName, credentials) {
    console.log('\n' + '='.repeat(60));
    console.log(`Processing Card Payment via ${connectorName}`);
    console.log('='.repeat(60));
    
    // Load probe data for this connector
    const probeData = loadProbeData(connectorName);
    
    // Get configuration
    const config = getConnectorConfig(connectorName, credentials);
    const client = new PaymentClient(config);
    
    // Build authorize request
    const authRequest = buildAuthorizeRequest(probeData, 'MANUAL');
    
    console.log('\nRequest:', JSON.stringify(authRequest, null, 2));
    
    // Step 1: Authorize
    console.log('\n[1/2] Authorizing...');
    const authResponse = await client.authorize(authRequest);
    
    console.log(`Status: ${authResponse.status}`);
    console.log(`Connector Transaction ID: ${authResponse.connectorTransactionId}`);
    
    // Handle status
    if (authResponse.status === 'FAILED') {
        console.log(`❌ Payment declined: ${authResponse.errorMessage || 'Unknown error'}`);
        return 'failed';
    }
    
    if (authResponse.status === 'PENDING') {
        console.log('⏳ Payment pending - awaiting async confirmation');
        console.log('   (In production: wait for webhook, then poll get())');
        return 'pending';
    }
    
    if (authResponse.status !== 'AUTHORIZED') {
        console.log(`⚠️  Unexpected status: ${authResponse.status}`);
        return 'error';
    }
    
    console.log('✅ Funds reserved successfully');
    
    // Step 2: Capture
    console.log('\n[2/2] Capturing...');
    const captureRequest = buildCaptureRequest(
        authResponse.connectorTransactionId,
        authRequest.amount
    );
    
    const captureResponse = await client.capture(captureRequest);
    console.log(`Capture Status: ${captureResponse.status}`);
    
    if (captureResponse.status === 'CAPTURED' || captureResponse.status === 'AUTHORIZED') {
        console.log('✅ Payment captured successfully');
        return 'success';
    } else {
        console.log(`⚠️  Capture returned: ${captureResponse.status}`);
        return 'partial';
    }
}
```

</details>


<details>
<summary><b>Celero</b></summary>

**Configuration:**
```javascript
// Configuration not available
```

**Full Example:**
```javascript
async function processCheckoutCard(connectorName, credentials) {
    console.log('\n' + '='.repeat(60));
    console.log(`Processing Card Payment via ${connectorName}`);
    console.log('='.repeat(60));
    
    // Load probe data for this connector
    const probeData = loadProbeData(connectorName);
    
    // Get configuration
    const config = getConnectorConfig(connectorName, credentials);
    const client = new PaymentClient(config);
    
    // Build authorize request
    const authRequest = buildAuthorizeRequest(probeData, 'MANUAL');
    
    console.log('\nRequest:', JSON.stringify(authRequest, null, 2));
    
    // Step 1: Authorize
    console.log('\n[1/2] Authorizing...');
    const authResponse = await client.authorize(authRequest);
    
    console.log(`Status: ${authResponse.status}`);
    console.log(`Connector Transaction ID: ${authResponse.connectorTransactionId}`);
    
    // Handle status
    if (authResponse.status === 'FAILED') {
        console.log(`❌ Payment declined: ${authResponse.errorMessage || 'Unknown error'}`);
        return 'failed';
    }
    
    if (authResponse.status === 'PENDING') {
        console.log('⏳ Payment pending - awaiting async confirmation');
        console.log('   (In production: wait for webhook, then poll get())');
        return 'pending';
    }
    
    if (authResponse.status !== 'AUTHORIZED') {
        console.log(`⚠️  Unexpected status: ${authResponse.status}`);
        return 'error';
    }
    
    console.log('✅ Funds reserved successfully');
    
    // Step 2: Capture
    console.log('\n[2/2] Capturing...');
    const captureRequest = buildCaptureRequest(
        authResponse.connectorTransactionId,
        authRequest.amount
    );
    
    const captureResponse = await client.capture(captureRequest);
    console.log(`Capture Status: ${captureResponse.status}`);
    
    if (captureResponse.status === 'CAPTURED' || captureResponse.status === 'AUTHORIZED') {
        console.log('✅ Payment captured successfully');
        return 'success';
    } else {
        console.log(`⚠️  Capture returned: ${captureResponse.status}`);
        return 'partial';
    }
}
```

</details>


<details>
<summary><b>Checkout.com</b></summary>

**Configuration:**
```javascript
function getCheckoutConfig(apiKey) {
    const config = ConnectorConfig.create({
        options: SdkOptions.create({ environment: Environment.SANDBOX }),
    });
    config.connectorConfig = ConnectorSpecificConfig.create({
        checkout: { apiKey: { value: apiKey } }
    });
    return config;
}
```

**Full Example:**
```javascript
async function processCheckoutCard(connectorName, credentials) {
    console.log('\n' + '='.repeat(60));
    console.log(`Processing Card Payment via ${connectorName}`);
    console.log('='.repeat(60));
    
    // Load probe data for this connector
    const probeData = loadProbeData(connectorName);
    
    // Get configuration
    const config = getConnectorConfig(connectorName, credentials);
    const client = new PaymentClient(config);
    
    // Build authorize request
    const authRequest = buildAuthorizeRequest(probeData, 'MANUAL');
    
    console.log('\nRequest:', JSON.stringify(authRequest, null, 2));
    
    // Step 1: Authorize
    console.log('\n[1/2] Authorizing...');
    const authResponse = await client.authorize(authRequest);
    
    console.log(`Status: ${authResponse.status}`);
    console.log(`Connector Transaction ID: ${authResponse.connectorTransactionId}`);
    
    // Handle status
    if (authResponse.status === 'FAILED') {
        console.log(`❌ Payment declined: ${authResponse.errorMessage || 'Unknown error'}`);
        return 'failed';
    }
    
    if (authResponse.status === 'PENDING') {
        console.log('⏳ Payment pending - awaiting async confirmation');
        console.log('   (In production: wait for webhook, then poll get())');
        return 'pending';
    }
    
    if (authResponse.status !== 'AUTHORIZED') {
        console.log(`⚠️  Unexpected status: ${authResponse.status}`);
        return 'error';
    }
    
    console.log('✅ Funds reserved successfully');
    
    // Step 2: Capture
    console.log('\n[2/2] Capturing...');
    const captureRequest = buildCaptureRequest(
        authResponse.connectorTransactionId,
        authRequest.amount
    );
    
    const captureResponse = await client.capture(captureRequest);
    console.log(`Capture Status: ${captureResponse.status}`);
    
    if (captureResponse.status === 'CAPTURED' || captureResponse.status === 'AUTHORIZED') {
        console.log('✅ Payment captured successfully');
        return 'success';
    } else {
        console.log(`⚠️  Capture returned: ${captureResponse.status}`);
        return 'partial';
    }
}
```

</details>


<details>
<summary><b>CyberSource</b></summary>

**Configuration:**
```javascript
// Configuration not available
```

**Full Example:**
```javascript
async function processCheckoutCard(connectorName, credentials) {
    console.log('\n' + '='.repeat(60));
    console.log(`Processing Card Payment via ${connectorName}`);
    console.log('='.repeat(60));
    
    // Load probe data for this connector
    const probeData = loadProbeData(connectorName);
    
    // Get configuration
    const config = getConnectorConfig(connectorName, credentials);
    const client = new PaymentClient(config);
    
    // Build authorize request
    const authRequest = buildAuthorizeRequest(probeData, 'MANUAL');
    
    console.log('\nRequest:', JSON.stringify(authRequest, null, 2));
    
    // Step 1: Authorize
    console.log('\n[1/2] Authorizing...');
    const authResponse = await client.authorize(authRequest);
    
    console.log(`Status: ${authResponse.status}`);
    console.log(`Connector Transaction ID: ${authResponse.connectorTransactionId}`);
    
    // Handle status
    if (authResponse.status === 'FAILED') {
        console.log(`❌ Payment declined: ${authResponse.errorMessage || 'Unknown error'}`);
        return 'failed';
    }
    
    if (authResponse.status === 'PENDING') {
        console.log('⏳ Payment pending - awaiting async confirmation');
        console.log('   (In production: wait for webhook, then poll get())');
        return 'pending';
    }
    
    if (authResponse.status !== 'AUTHORIZED') {
        console.log(`⚠️  Unexpected status: ${authResponse.status}`);
        return 'error';
    }
    
    console.log('✅ Funds reserved successfully');
    
    // Step 2: Capture
    console.log('\n[2/2] Capturing...');
    const captureRequest = buildCaptureRequest(
        authResponse.connectorTransactionId,
        authRequest.amount
    );
    
    const captureResponse = await client.capture(captureRequest);
    console.log(`Capture Status: ${captureResponse.status}`);
    
    if (captureResponse.status === 'CAPTURED' || captureResponse.status === 'AUTHORIZED') {
        console.log('✅ Payment captured successfully');
        return 'success';
    } else {
        console.log(`⚠️  Capture returned: ${captureResponse.status}`);
        return 'partial';
    }
}
```

</details>


<details>
<summary><b>Datatrans</b></summary>

**Configuration:**
```javascript
// Configuration not available
```

**Full Example:**
```javascript
async function processCheckoutCard(connectorName, credentials) {
    console.log('\n' + '='.repeat(60));
    console.log(`Processing Card Payment via ${connectorName}`);
    console.log('='.repeat(60));
    
    // Load probe data for this connector
    const probeData = loadProbeData(connectorName);
    
    // Get configuration
    const config = getConnectorConfig(connectorName, credentials);
    const client = new PaymentClient(config);
    
    // Build authorize request
    const authRequest = buildAuthorizeRequest(probeData, 'MANUAL');
    
    console.log('\nRequest:', JSON.stringify(authRequest, null, 2));
    
    // Step 1: Authorize
    console.log('\n[1/2] Authorizing...');
    const authResponse = await client.authorize(authRequest);
    
    console.log(`Status: ${authResponse.status}`);
    console.log(`Connector Transaction ID: ${authResponse.connectorTransactionId}`);
    
    // Handle status
    if (authResponse.status === 'FAILED') {
        console.log(`❌ Payment declined: ${authResponse.errorMessage || 'Unknown error'}`);
        return 'failed';
    }
    
    if (authResponse.status === 'PENDING') {
        console.log('⏳ Payment pending - awaiting async confirmation');
        console.log('   (In production: wait for webhook, then poll get())');
        return 'pending';
    }
    
    if (authResponse.status !== 'AUTHORIZED') {
        console.log(`⚠️  Unexpected status: ${authResponse.status}`);
        return 'error';
    }
    
    console.log('✅ Funds reserved successfully');
    
    // Step 2: Capture
    console.log('\n[2/2] Capturing...');
    const captureRequest = buildCaptureRequest(
        authResponse.connectorTransactionId,
        authRequest.amount
    );
    
    const captureResponse = await client.capture(captureRequest);
    console.log(`Capture Status: ${captureResponse.status}`);
    
    if (captureResponse.status === 'CAPTURED' || captureResponse.status === 'AUTHORIZED') {
        console.log('✅ Payment captured successfully');
        return 'success';
    } else {
        console.log(`⚠️  Capture returned: ${captureResponse.status}`);
        return 'partial';
    }
}
```

</details>


<details>
<summary><b>dLocal</b></summary>

**Configuration:**
```javascript
// Configuration not available
```

**Full Example:**
```javascript
async function processCheckoutCard(connectorName, credentials) {
    console.log('\n' + '='.repeat(60));
    console.log(`Processing Card Payment via ${connectorName}`);
    console.log('='.repeat(60));
    
    // Load probe data for this connector
    const probeData = loadProbeData(connectorName);
    
    // Get configuration
    const config = getConnectorConfig(connectorName, credentials);
    const client = new PaymentClient(config);
    
    // Build authorize request
    const authRequest = buildAuthorizeRequest(probeData, 'MANUAL');
    
    console.log('\nRequest:', JSON.stringify(authRequest, null, 2));
    
    // Step 1: Authorize
    console.log('\n[1/2] Authorizing...');
    const authResponse = await client.authorize(authRequest);
    
    console.log(`Status: ${authResponse.status}`);
    console.log(`Connector Transaction ID: ${authResponse.connectorTransactionId}`);
    
    // Handle status
    if (authResponse.status === 'FAILED') {
        console.log(`❌ Payment declined: ${authResponse.errorMessage || 'Unknown error'}`);
        return 'failed';
    }
    
    if (authResponse.status === 'PENDING') {
        console.log('⏳ Payment pending - awaiting async confirmation');
        console.log('   (In production: wait for webhook, then poll get())');
        return 'pending';
    }
    
    if (authResponse.status !== 'AUTHORIZED') {
        console.log(`⚠️  Unexpected status: ${authResponse.status}`);
        return 'error';
    }
    
    console.log('✅ Funds reserved successfully');
    
    // Step 2: Capture
    console.log('\n[2/2] Capturing...');
    const captureRequest = buildCaptureRequest(
        authResponse.connectorTransactionId,
        authRequest.amount
    );
    
    const captureResponse = await client.capture(captureRequest);
    console.log(`Capture Status: ${captureResponse.status}`);
    
    if (captureResponse.status === 'CAPTURED' || captureResponse.status === 'AUTHORIZED') {
        console.log('✅ Payment captured successfully');
        return 'success';
    } else {
        console.log(`⚠️  Capture returned: ${captureResponse.status}`);
        return 'partial';
    }
}
```

</details>


<details>
<summary><b>Elavon</b></summary>

**Configuration:**
```javascript
// Configuration not available
```

**Full Example:**
```javascript
async function processCheckoutCard(connectorName, credentials) {
    console.log('\n' + '='.repeat(60));
    console.log(`Processing Card Payment via ${connectorName}`);
    console.log('='.repeat(60));
    
    // Load probe data for this connector
    const probeData = loadProbeData(connectorName);
    
    // Get configuration
    const config = getConnectorConfig(connectorName, credentials);
    const client = new PaymentClient(config);
    
    // Build authorize request
    const authRequest = buildAuthorizeRequest(probeData, 'MANUAL');
    
    console.log('\nRequest:', JSON.stringify(authRequest, null, 2));
    
    // Step 1: Authorize
    console.log('\n[1/2] Authorizing...');
    const authResponse = await client.authorize(authRequest);
    
    console.log(`Status: ${authResponse.status}`);
    console.log(`Connector Transaction ID: ${authResponse.connectorTransactionId}`);
    
    // Handle status
    if (authResponse.status === 'FAILED') {
        console.log(`❌ Payment declined: ${authResponse.errorMessage || 'Unknown error'}`);
        return 'failed';
    }
    
    if (authResponse.status === 'PENDING') {
        console.log('⏳ Payment pending - awaiting async confirmation');
        console.log('   (In production: wait for webhook, then poll get())');
        return 'pending';
    }
    
    if (authResponse.status !== 'AUTHORIZED') {
        console.log(`⚠️  Unexpected status: ${authResponse.status}`);
        return 'error';
    }
    
    console.log('✅ Funds reserved successfully');
    
    // Step 2: Capture
    console.log('\n[2/2] Capturing...');
    const captureRequest = buildCaptureRequest(
        authResponse.connectorTransactionId,
        authRequest.amount
    );
    
    const captureResponse = await client.capture(captureRequest);
    console.log(`Capture Status: ${captureResponse.status}`);
    
    if (captureResponse.status === 'CAPTURED' || captureResponse.status === 'AUTHORIZED') {
        console.log('✅ Payment captured successfully');
        return 'success';
    } else {
        console.log(`⚠️  Capture returned: ${captureResponse.status}`);
        return 'partial';
    }
}
```

</details>


<details>
<summary><b>Finix</b></summary>

**Configuration:**
```javascript
// Configuration not available
```

**Full Example:**
```javascript
async function processCheckoutCard(connectorName, credentials) {
    console.log('\n' + '='.repeat(60));
    console.log(`Processing Card Payment via ${connectorName}`);
    console.log('='.repeat(60));
    
    // Load probe data for this connector
    const probeData = loadProbeData(connectorName);
    
    // Get configuration
    const config = getConnectorConfig(connectorName, credentials);
    const client = new PaymentClient(config);
    
    // Build authorize request
    const authRequest = buildAuthorizeRequest(probeData, 'MANUAL');
    
    console.log('\nRequest:', JSON.stringify(authRequest, null, 2));
    
    // Step 1: Authorize
    console.log('\n[1/2] Authorizing...');
    const authResponse = await client.authorize(authRequest);
    
    console.log(`Status: ${authResponse.status}`);
    console.log(`Connector Transaction ID: ${authResponse.connectorTransactionId}`);
    
    // Handle status
    if (authResponse.status === 'FAILED') {
        console.log(`❌ Payment declined: ${authResponse.errorMessage || 'Unknown error'}`);
        return 'failed';
    }
    
    if (authResponse.status === 'PENDING') {
        console.log('⏳ Payment pending - awaiting async confirmation');
        console.log('   (In production: wait for webhook, then poll get())');
        return 'pending';
    }
    
    if (authResponse.status !== 'AUTHORIZED') {
        console.log(`⚠️  Unexpected status: ${authResponse.status}`);
        return 'error';
    }
    
    console.log('✅ Funds reserved successfully');
    
    // Step 2: Capture
    console.log('\n[2/2] Capturing...');
    const captureRequest = buildCaptureRequest(
        authResponse.connectorTransactionId,
        authRequest.amount
    );
    
    const captureResponse = await client.capture(captureRequest);
    console.log(`Capture Status: ${captureResponse.status}`);
    
    if (captureResponse.status === 'CAPTURED' || captureResponse.status === 'AUTHORIZED') {
        console.log('✅ Payment captured successfully');
        return 'success';
    } else {
        console.log(`⚠️  Capture returned: ${captureResponse.status}`);
        return 'partial';
    }
}
```

</details>


<details>
<summary><b>Fiserv</b></summary>

**Configuration:**
```javascript
// Configuration not available
```

**Full Example:**
```javascript
async function processCheckoutCard(connectorName, credentials) {
    console.log('\n' + '='.repeat(60));
    console.log(`Processing Card Payment via ${connectorName}`);
    console.log('='.repeat(60));
    
    // Load probe data for this connector
    const probeData = loadProbeData(connectorName);
    
    // Get configuration
    const config = getConnectorConfig(connectorName, credentials);
    const client = new PaymentClient(config);
    
    // Build authorize request
    const authRequest = buildAuthorizeRequest(probeData, 'MANUAL');
    
    console.log('\nRequest:', JSON.stringify(authRequest, null, 2));
    
    // Step 1: Authorize
    console.log('\n[1/2] Authorizing...');
    const authResponse = await client.authorize(authRequest);
    
    console.log(`Status: ${authResponse.status}`);
    console.log(`Connector Transaction ID: ${authResponse.connectorTransactionId}`);
    
    // Handle status
    if (authResponse.status === 'FAILED') {
        console.log(`❌ Payment declined: ${authResponse.errorMessage || 'Unknown error'}`);
        return 'failed';
    }
    
    if (authResponse.status === 'PENDING') {
        console.log('⏳ Payment pending - awaiting async confirmation');
        console.log('   (In production: wait for webhook, then poll get())');
        return 'pending';
    }
    
    if (authResponse.status !== 'AUTHORIZED') {
        console.log(`⚠️  Unexpected status: ${authResponse.status}`);
        return 'error';
    }
    
    console.log('✅ Funds reserved successfully');
    
    // Step 2: Capture
    console.log('\n[2/2] Capturing...');
    const captureRequest = buildCaptureRequest(
        authResponse.connectorTransactionId,
        authRequest.amount
    );
    
    const captureResponse = await client.capture(captureRequest);
    console.log(`Capture Status: ${captureResponse.status}`);
    
    if (captureResponse.status === 'CAPTURED' || captureResponse.status === 'AUTHORIZED') {
        console.log('✅ Payment captured successfully');
        return 'success';
    } else {
        console.log(`⚠️  Capture returned: ${captureResponse.status}`);
        return 'partial';
    }
}
```

</details>


<details>
<summary><b>Fiserv EMEA</b></summary>

**Configuration:**
```javascript
// Configuration not available
```

**Full Example:**
```javascript
async function processCheckoutCard(connectorName, credentials) {
    console.log('\n' + '='.repeat(60));
    console.log(`Processing Card Payment via ${connectorName}`);
    console.log('='.repeat(60));
    
    // Load probe data for this connector
    const probeData = loadProbeData(connectorName);
    
    // Get configuration
    const config = getConnectorConfig(connectorName, credentials);
    const client = new PaymentClient(config);
    
    // Build authorize request
    const authRequest = buildAuthorizeRequest(probeData, 'MANUAL');
    
    console.log('\nRequest:', JSON.stringify(authRequest, null, 2));
    
    // Step 1: Authorize
    console.log('\n[1/2] Authorizing...');
    const authResponse = await client.authorize(authRequest);
    
    console.log(`Status: ${authResponse.status}`);
    console.log(`Connector Transaction ID: ${authResponse.connectorTransactionId}`);
    
    // Handle status
    if (authResponse.status === 'FAILED') {
        console.log(`❌ Payment declined: ${authResponse.errorMessage || 'Unknown error'}`);
        return 'failed';
    }
    
    if (authResponse.status === 'PENDING') {
        console.log('⏳ Payment pending - awaiting async confirmation');
        console.log('   (In production: wait for webhook, then poll get())');
        return 'pending';
    }
    
    if (authResponse.status !== 'AUTHORIZED') {
        console.log(`⚠️  Unexpected status: ${authResponse.status}`);
        return 'error';
    }
    
    console.log('✅ Funds reserved successfully');
    
    // Step 2: Capture
    console.log('\n[2/2] Capturing...');
    const captureRequest = buildCaptureRequest(
        authResponse.connectorTransactionId,
        authRequest.amount
    );
    
    const captureResponse = await client.capture(captureRequest);
    console.log(`Capture Status: ${captureResponse.status}`);
    
    if (captureResponse.status === 'CAPTURED' || captureResponse.status === 'AUTHORIZED') {
        console.log('✅ Payment captured successfully');
        return 'success';
    } else {
        console.log(`⚠️  Capture returned: ${captureResponse.status}`);
        return 'partial';
    }
}
```

</details>


<details>
<summary><b>Fiuu</b></summary>

**Configuration:**
```javascript
// Configuration not available
```

**Full Example:**
```javascript
async function processCheckoutCard(connectorName, credentials) {
    console.log('\n' + '='.repeat(60));
    console.log(`Processing Card Payment via ${connectorName}`);
    console.log('='.repeat(60));
    
    // Load probe data for this connector
    const probeData = loadProbeData(connectorName);
    
    // Get configuration
    const config = getConnectorConfig(connectorName, credentials);
    const client = new PaymentClient(config);
    
    // Build authorize request
    const authRequest = buildAuthorizeRequest(probeData, 'MANUAL');
    
    console.log('\nRequest:', JSON.stringify(authRequest, null, 2));
    
    // Step 1: Authorize
    console.log('\n[1/2] Authorizing...');
    const authResponse = await client.authorize(authRequest);
    
    console.log(`Status: ${authResponse.status}`);
    console.log(`Connector Transaction ID: ${authResponse.connectorTransactionId}`);
    
    // Handle status
    if (authResponse.status === 'FAILED') {
        console.log(`❌ Payment declined: ${authResponse.errorMessage || 'Unknown error'}`);
        return 'failed';
    }
    
    if (authResponse.status === 'PENDING') {
        console.log('⏳ Payment pending - awaiting async confirmation');
        console.log('   (In production: wait for webhook, then poll get())');
        return 'pending';
    }
    
    if (authResponse.status !== 'AUTHORIZED') {
        console.log(`⚠️  Unexpected status: ${authResponse.status}`);
        return 'error';
    }
    
    console.log('✅ Funds reserved successfully');
    
    // Step 2: Capture
    console.log('\n[2/2] Capturing...');
    const captureRequest = buildCaptureRequest(
        authResponse.connectorTransactionId,
        authRequest.amount
    );
    
    const captureResponse = await client.capture(captureRequest);
    console.log(`Capture Status: ${captureResponse.status}`);
    
    if (captureResponse.status === 'CAPTURED' || captureResponse.status === 'AUTHORIZED') {
        console.log('✅ Payment captured successfully');
        return 'success';
    } else {
        console.log(`⚠️  Capture returned: ${captureResponse.status}`);
        return 'partial';
    }
}
```

</details>


<details>
<summary><b>Getnet</b></summary>

**Configuration:**
```javascript
// Configuration not available
```

**Full Example:**
```javascript
async function processCheckoutCard(connectorName, credentials) {
    console.log('\n' + '='.repeat(60));
    console.log(`Processing Card Payment via ${connectorName}`);
    console.log('='.repeat(60));
    
    // Load probe data for this connector
    const probeData = loadProbeData(connectorName);
    
    // Get configuration
    const config = getConnectorConfig(connectorName, credentials);
    const client = new PaymentClient(config);
    
    // Build authorize request
    const authRequest = buildAuthorizeRequest(probeData, 'MANUAL');
    
    console.log('\nRequest:', JSON.stringify(authRequest, null, 2));
    
    // Step 1: Authorize
    console.log('\n[1/2] Authorizing...');
    const authResponse = await client.authorize(authRequest);
    
    console.log(`Status: ${authResponse.status}`);
    console.log(`Connector Transaction ID: ${authResponse.connectorTransactionId}`);
    
    // Handle status
    if (authResponse.status === 'FAILED') {
        console.log(`❌ Payment declined: ${authResponse.errorMessage || 'Unknown error'}`);
        return 'failed';
    }
    
    if (authResponse.status === 'PENDING') {
        console.log('⏳ Payment pending - awaiting async confirmation');
        console.log('   (In production: wait for webhook, then poll get())');
        return 'pending';
    }
    
    if (authResponse.status !== 'AUTHORIZED') {
        console.log(`⚠️  Unexpected status: ${authResponse.status}`);
        return 'error';
    }
    
    console.log('✅ Funds reserved successfully');
    
    // Step 2: Capture
    console.log('\n[2/2] Capturing...');
    const captureRequest = buildCaptureRequest(
        authResponse.connectorTransactionId,
        authRequest.amount
    );
    
    const captureResponse = await client.capture(captureRequest);
    console.log(`Capture Status: ${captureResponse.status}`);
    
    if (captureResponse.status === 'CAPTURED' || captureResponse.status === 'AUTHORIZED') {
        console.log('✅ Payment captured successfully');
        return 'success';
    } else {
        console.log(`⚠️  Capture returned: ${captureResponse.status}`);
        return 'partial';
    }
}
```

</details>


<details>
<summary><b>Global Payments</b></summary>

**Configuration:**
```javascript
// Configuration not available
```

**Full Example:**
```javascript
async function processCheckoutCard(connectorName, credentials) {
    console.log('\n' + '='.repeat(60));
    console.log(`Processing Card Payment via ${connectorName}`);
    console.log('='.repeat(60));
    
    // Load probe data for this connector
    const probeData = loadProbeData(connectorName);
    
    // Get configuration
    const config = getConnectorConfig(connectorName, credentials);
    const client = new PaymentClient(config);
    
    // Build authorize request
    const authRequest = buildAuthorizeRequest(probeData, 'MANUAL');
    
    console.log('\nRequest:', JSON.stringify(authRequest, null, 2));
    
    // Step 1: Authorize
    console.log('\n[1/2] Authorizing...');
    const authResponse = await client.authorize(authRequest);
    
    console.log(`Status: ${authResponse.status}`);
    console.log(`Connector Transaction ID: ${authResponse.connectorTransactionId}`);
    
    // Handle status
    if (authResponse.status === 'FAILED') {
        console.log(`❌ Payment declined: ${authResponse.errorMessage || 'Unknown error'}`);
        return 'failed';
    }
    
    if (authResponse.status === 'PENDING') {
        console.log('⏳ Payment pending - awaiting async confirmation');
        console.log('   (In production: wait for webhook, then poll get())');
        return 'pending';
    }
    
    if (authResponse.status !== 'AUTHORIZED') {
        console.log(`⚠️  Unexpected status: ${authResponse.status}`);
        return 'error';
    }
    
    console.log('✅ Funds reserved successfully');
    
    // Step 2: Capture
    console.log('\n[2/2] Capturing...');
    const captureRequest = buildCaptureRequest(
        authResponse.connectorTransactionId,
        authRequest.amount
    );
    
    const captureResponse = await client.capture(captureRequest);
    console.log(`Capture Status: ${captureResponse.status}`);
    
    if (captureResponse.status === 'CAPTURED' || captureResponse.status === 'AUTHORIZED') {
        console.log('✅ Payment captured successfully');
        return 'success';
    } else {
        console.log(`⚠️  Capture returned: ${captureResponse.status}`);
        return 'partial';
    }
}
```

</details>


<details>
<summary><b>HiPay</b></summary>

**Configuration:**
```javascript
// Configuration not available
```

**Full Example:**
```javascript
async function processCheckoutCard(connectorName, credentials) {
    console.log('\n' + '='.repeat(60));
    console.log(`Processing Card Payment via ${connectorName}`);
    console.log('='.repeat(60));
    
    // Load probe data for this connector
    const probeData = loadProbeData(connectorName);
    
    // Get configuration
    const config = getConnectorConfig(connectorName, credentials);
    const client = new PaymentClient(config);
    
    // Build authorize request
    const authRequest = buildAuthorizeRequest(probeData, 'MANUAL');
    
    console.log('\nRequest:', JSON.stringify(authRequest, null, 2));
    
    // Step 1: Authorize
    console.log('\n[1/2] Authorizing...');
    const authResponse = await client.authorize(authRequest);
    
    console.log(`Status: ${authResponse.status}`);
    console.log(`Connector Transaction ID: ${authResponse.connectorTransactionId}`);
    
    // Handle status
    if (authResponse.status === 'FAILED') {
        console.log(`❌ Payment declined: ${authResponse.errorMessage || 'Unknown error'}`);
        return 'failed';
    }
    
    if (authResponse.status === 'PENDING') {
        console.log('⏳ Payment pending - awaiting async confirmation');
        console.log('   (In production: wait for webhook, then poll get())');
        return 'pending';
    }
    
    if (authResponse.status !== 'AUTHORIZED') {
        console.log(`⚠️  Unexpected status: ${authResponse.status}`);
        return 'error';
    }
    
    console.log('✅ Funds reserved successfully');
    
    // Step 2: Capture
    console.log('\n[2/2] Capturing...');
    const captureRequest = buildCaptureRequest(
        authResponse.connectorTransactionId,
        authRequest.amount
    );
    
    const captureResponse = await client.capture(captureRequest);
    console.log(`Capture Status: ${captureResponse.status}`);
    
    if (captureResponse.status === 'CAPTURED' || captureResponse.status === 'AUTHORIZED') {
        console.log('✅ Payment captured successfully');
        return 'success';
    } else {
        console.log(`⚠️  Capture returned: ${captureResponse.status}`);
        return 'partial';
    }
}
```

</details>


<details>
<summary><b>J.P. Morgan</b></summary>

**Configuration:**
```javascript
// Configuration not available
```

**Full Example:**
```javascript
async function processCheckoutCard(connectorName, credentials) {
    console.log('\n' + '='.repeat(60));
    console.log(`Processing Card Payment via ${connectorName}`);
    console.log('='.repeat(60));
    
    // Load probe data for this connector
    const probeData = loadProbeData(connectorName);
    
    // Get configuration
    const config = getConnectorConfig(connectorName, credentials);
    const client = new PaymentClient(config);
    
    // Build authorize request
    const authRequest = buildAuthorizeRequest(probeData, 'MANUAL');
    
    console.log('\nRequest:', JSON.stringify(authRequest, null, 2));
    
    // Step 1: Authorize
    console.log('\n[1/2] Authorizing...');
    const authResponse = await client.authorize(authRequest);
    
    console.log(`Status: ${authResponse.status}`);
    console.log(`Connector Transaction ID: ${authResponse.connectorTransactionId}`);
    
    // Handle status
    if (authResponse.status === 'FAILED') {
        console.log(`❌ Payment declined: ${authResponse.errorMessage || 'Unknown error'}`);
        return 'failed';
    }
    
    if (authResponse.status === 'PENDING') {
        console.log('⏳ Payment pending - awaiting async confirmation');
        console.log('   (In production: wait for webhook, then poll get())');
        return 'pending';
    }
    
    if (authResponse.status !== 'AUTHORIZED') {
        console.log(`⚠️  Unexpected status: ${authResponse.status}`);
        return 'error';
    }
    
    console.log('✅ Funds reserved successfully');
    
    // Step 2: Capture
    console.log('\n[2/2] Capturing...');
    const captureRequest = buildCaptureRequest(
        authResponse.connectorTransactionId,
        authRequest.amount
    );
    
    const captureResponse = await client.capture(captureRequest);
    console.log(`Capture Status: ${captureResponse.status}`);
    
    if (captureResponse.status === 'CAPTURED' || captureResponse.status === 'AUTHORIZED') {
        console.log('✅ Payment captured successfully');
        return 'success';
    } else {
        console.log(`⚠️  Capture returned: ${captureResponse.status}`);
        return 'partial';
    }
}
```

</details>


<details>
<summary><b>NMI</b></summary>

**Configuration:**
```javascript
// Configuration not available
```

**Full Example:**
```javascript
async function processCheckoutCard(connectorName, credentials) {
    console.log('\n' + '='.repeat(60));
    console.log(`Processing Card Payment via ${connectorName}`);
    console.log('='.repeat(60));
    
    // Load probe data for this connector
    const probeData = loadProbeData(connectorName);
    
    // Get configuration
    const config = getConnectorConfig(connectorName, credentials);
    const client = new PaymentClient(config);
    
    // Build authorize request
    const authRequest = buildAuthorizeRequest(probeData, 'MANUAL');
    
    console.log('\nRequest:', JSON.stringify(authRequest, null, 2));
    
    // Step 1: Authorize
    console.log('\n[1/2] Authorizing...');
    const authResponse = await client.authorize(authRequest);
    
    console.log(`Status: ${authResponse.status}`);
    console.log(`Connector Transaction ID: ${authResponse.connectorTransactionId}`);
    
    // Handle status
    if (authResponse.status === 'FAILED') {
        console.log(`❌ Payment declined: ${authResponse.errorMessage || 'Unknown error'}`);
        return 'failed';
    }
    
    if (authResponse.status === 'PENDING') {
        console.log('⏳ Payment pending - awaiting async confirmation');
        console.log('   (In production: wait for webhook, then poll get())');
        return 'pending';
    }
    
    if (authResponse.status !== 'AUTHORIZED') {
        console.log(`⚠️  Unexpected status: ${authResponse.status}`);
        return 'error';
    }
    
    console.log('✅ Funds reserved successfully');
    
    // Step 2: Capture
    console.log('\n[2/2] Capturing...');
    const captureRequest = buildCaptureRequest(
        authResponse.connectorTransactionId,
        authRequest.amount
    );
    
    const captureResponse = await client.capture(captureRequest);
    console.log(`Capture Status: ${captureResponse.status}`);
    
    if (captureResponse.status === 'CAPTURED' || captureResponse.status === 'AUTHORIZED') {
        console.log('✅ Payment captured successfully');
        return 'success';
    } else {
        console.log(`⚠️  Capture returned: ${captureResponse.status}`);
        return 'partial';
    }
}
```

</details>


<details>
<summary><b>Noon</b></summary>

**Configuration:**
```javascript
// Configuration not available
```

**Full Example:**
```javascript
async function processCheckoutCard(connectorName, credentials) {
    console.log('\n' + '='.repeat(60));
    console.log(`Processing Card Payment via ${connectorName}`);
    console.log('='.repeat(60));
    
    // Load probe data for this connector
    const probeData = loadProbeData(connectorName);
    
    // Get configuration
    const config = getConnectorConfig(connectorName, credentials);
    const client = new PaymentClient(config);
    
    // Build authorize request
    const authRequest = buildAuthorizeRequest(probeData, 'MANUAL');
    
    console.log('\nRequest:', JSON.stringify(authRequest, null, 2));
    
    // Step 1: Authorize
    console.log('\n[1/2] Authorizing...');
    const authResponse = await client.authorize(authRequest);
    
    console.log(`Status: ${authResponse.status}`);
    console.log(`Connector Transaction ID: ${authResponse.connectorTransactionId}`);
    
    // Handle status
    if (authResponse.status === 'FAILED') {
        console.log(`❌ Payment declined: ${authResponse.errorMessage || 'Unknown error'}`);
        return 'failed';
    }
    
    if (authResponse.status === 'PENDING') {
        console.log('⏳ Payment pending - awaiting async confirmation');
        console.log('   (In production: wait for webhook, then poll get())');
        return 'pending';
    }
    
    if (authResponse.status !== 'AUTHORIZED') {
        console.log(`⚠️  Unexpected status: ${authResponse.status}`);
        return 'error';
    }
    
    console.log('✅ Funds reserved successfully');
    
    // Step 2: Capture
    console.log('\n[2/2] Capturing...');
    const captureRequest = buildCaptureRequest(
        authResponse.connectorTransactionId,
        authRequest.amount
    );
    
    const captureResponse = await client.capture(captureRequest);
    console.log(`Capture Status: ${captureResponse.status}`);
    
    if (captureResponse.status === 'CAPTURED' || captureResponse.status === 'AUTHORIZED') {
        console.log('✅ Payment captured successfully');
        return 'success';
    } else {
        console.log(`⚠️  Capture returned: ${captureResponse.status}`);
        return 'partial';
    }
}
```

</details>


<details>
<summary><b>Novalnet</b></summary>

**Configuration:**
```javascript
// Configuration not available
```

**Full Example:**
```javascript
async function processCheckoutCard(connectorName, credentials) {
    console.log('\n' + '='.repeat(60));
    console.log(`Processing Card Payment via ${connectorName}`);
    console.log('='.repeat(60));
    
    // Load probe data for this connector
    const probeData = loadProbeData(connectorName);
    
    // Get configuration
    const config = getConnectorConfig(connectorName, credentials);
    const client = new PaymentClient(config);
    
    // Build authorize request
    const authRequest = buildAuthorizeRequest(probeData, 'MANUAL');
    
    console.log('\nRequest:', JSON.stringify(authRequest, null, 2));
    
    // Step 1: Authorize
    console.log('\n[1/2] Authorizing...');
    const authResponse = await client.authorize(authRequest);
    
    console.log(`Status: ${authResponse.status}`);
    console.log(`Connector Transaction ID: ${authResponse.connectorTransactionId}`);
    
    // Handle status
    if (authResponse.status === 'FAILED') {
        console.log(`❌ Payment declined: ${authResponse.errorMessage || 'Unknown error'}`);
        return 'failed';
    }
    
    if (authResponse.status === 'PENDING') {
        console.log('⏳ Payment pending - awaiting async confirmation');
        console.log('   (In production: wait for webhook, then poll get())');
        return 'pending';
    }
    
    if (authResponse.status !== 'AUTHORIZED') {
        console.log(`⚠️  Unexpected status: ${authResponse.status}`);
        return 'error';
    }
    
    console.log('✅ Funds reserved successfully');
    
    // Step 2: Capture
    console.log('\n[2/2] Capturing...');
    const captureRequest = buildCaptureRequest(
        authResponse.connectorTransactionId,
        authRequest.amount
    );
    
    const captureResponse = await client.capture(captureRequest);
    console.log(`Capture Status: ${captureResponse.status}`);
    
    if (captureResponse.status === 'CAPTURED' || captureResponse.status === 'AUTHORIZED') {
        console.log('✅ Payment captured successfully');
        return 'success';
    } else {
        console.log(`⚠️  Capture returned: ${captureResponse.status}`);
        return 'partial';
    }
}
```

</details>


<details>
<summary><b>Nuvei</b></summary>

**Configuration:**
```javascript
// Configuration not available
```

**Full Example:**
```javascript
async function processCheckoutCard(connectorName, credentials) {
    console.log('\n' + '='.repeat(60));
    console.log(`Processing Card Payment via ${connectorName}`);
    console.log('='.repeat(60));
    
    // Load probe data for this connector
    const probeData = loadProbeData(connectorName);
    
    // Get configuration
    const config = getConnectorConfig(connectorName, credentials);
    const client = new PaymentClient(config);
    
    // Build authorize request
    const authRequest = buildAuthorizeRequest(probeData, 'MANUAL');
    
    console.log('\nRequest:', JSON.stringify(authRequest, null, 2));
    
    // Step 1: Authorize
    console.log('\n[1/2] Authorizing...');
    const authResponse = await client.authorize(authRequest);
    
    console.log(`Status: ${authResponse.status}`);
    console.log(`Connector Transaction ID: ${authResponse.connectorTransactionId}`);
    
    // Handle status
    if (authResponse.status === 'FAILED') {
        console.log(`❌ Payment declined: ${authResponse.errorMessage || 'Unknown error'}`);
        return 'failed';
    }
    
    if (authResponse.status === 'PENDING') {
        console.log('⏳ Payment pending - awaiting async confirmation');
        console.log('   (In production: wait for webhook, then poll get())');
        return 'pending';
    }
    
    if (authResponse.status !== 'AUTHORIZED') {
        console.log(`⚠️  Unexpected status: ${authResponse.status}`);
        return 'error';
    }
    
    console.log('✅ Funds reserved successfully');
    
    // Step 2: Capture
    console.log('\n[2/2] Capturing...');
    const captureRequest = buildCaptureRequest(
        authResponse.connectorTransactionId,
        authRequest.amount
    );
    
    const captureResponse = await client.capture(captureRequest);
    console.log(`Capture Status: ${captureResponse.status}`);
    
    if (captureResponse.status === 'CAPTURED' || captureResponse.status === 'AUTHORIZED') {
        console.log('✅ Payment captured successfully');
        return 'success';
    } else {
        console.log(`⚠️  Capture returned: ${captureResponse.status}`);
        return 'partial';
    }
}
```

</details>


<details>
<summary><b>Paybox</b></summary>

**Configuration:**
```javascript
// Configuration not available
```

**Full Example:**
```javascript
async function processCheckoutCard(connectorName, credentials) {
    console.log('\n' + '='.repeat(60));
    console.log(`Processing Card Payment via ${connectorName}`);
    console.log('='.repeat(60));
    
    // Load probe data for this connector
    const probeData = loadProbeData(connectorName);
    
    // Get configuration
    const config = getConnectorConfig(connectorName, credentials);
    const client = new PaymentClient(config);
    
    // Build authorize request
    const authRequest = buildAuthorizeRequest(probeData, 'MANUAL');
    
    console.log('\nRequest:', JSON.stringify(authRequest, null, 2));
    
    // Step 1: Authorize
    console.log('\n[1/2] Authorizing...');
    const authResponse = await client.authorize(authRequest);
    
    console.log(`Status: ${authResponse.status}`);
    console.log(`Connector Transaction ID: ${authResponse.connectorTransactionId}`);
    
    // Handle status
    if (authResponse.status === 'FAILED') {
        console.log(`❌ Payment declined: ${authResponse.errorMessage || 'Unknown error'}`);
        return 'failed';
    }
    
    if (authResponse.status === 'PENDING') {
        console.log('⏳ Payment pending - awaiting async confirmation');
        console.log('   (In production: wait for webhook, then poll get())');
        return 'pending';
    }
    
    if (authResponse.status !== 'AUTHORIZED') {
        console.log(`⚠️  Unexpected status: ${authResponse.status}`);
        return 'error';
    }
    
    console.log('✅ Funds reserved successfully');
    
    // Step 2: Capture
    console.log('\n[2/2] Capturing...');
    const captureRequest = buildCaptureRequest(
        authResponse.connectorTransactionId,
        authRequest.amount
    );
    
    const captureResponse = await client.capture(captureRequest);
    console.log(`Capture Status: ${captureResponse.status}`);
    
    if (captureResponse.status === 'CAPTURED' || captureResponse.status === 'AUTHORIZED') {
        console.log('✅ Payment captured successfully');
        return 'success';
    } else {
        console.log(`⚠️  Capture returned: ${captureResponse.status}`);
        return 'partial';
    }
}
```

</details>


<details>
<summary><b>PayPal</b></summary>

**Configuration:**
```javascript
// Configuration not available
```

**Full Example:**
```javascript
async function processCheckoutCard(connectorName, credentials) {
    console.log('\n' + '='.repeat(60));
    console.log(`Processing Card Payment via ${connectorName}`);
    console.log('='.repeat(60));
    
    // Load probe data for this connector
    const probeData = loadProbeData(connectorName);
    
    // Get configuration
    const config = getConnectorConfig(connectorName, credentials);
    const client = new PaymentClient(config);
    
    // Build authorize request
    const authRequest = buildAuthorizeRequest(probeData, 'MANUAL');
    
    console.log('\nRequest:', JSON.stringify(authRequest, null, 2));
    
    // Step 1: Authorize
    console.log('\n[1/2] Authorizing...');
    const authResponse = await client.authorize(authRequest);
    
    console.log(`Status: ${authResponse.status}`);
    console.log(`Connector Transaction ID: ${authResponse.connectorTransactionId}`);
    
    // Handle status
    if (authResponse.status === 'FAILED') {
        console.log(`❌ Payment declined: ${authResponse.errorMessage || 'Unknown error'}`);
        return 'failed';
    }
    
    if (authResponse.status === 'PENDING') {
        console.log('⏳ Payment pending - awaiting async confirmation');
        console.log('   (In production: wait for webhook, then poll get())');
        return 'pending';
    }
    
    if (authResponse.status !== 'AUTHORIZED') {
        console.log(`⚠️  Unexpected status: ${authResponse.status}`);
        return 'error';
    }
    
    console.log('✅ Funds reserved successfully');
    
    // Step 2: Capture
    console.log('\n[2/2] Capturing...');
    const captureRequest = buildCaptureRequest(
        authResponse.connectorTransactionId,
        authRequest.amount
    );
    
    const captureResponse = await client.capture(captureRequest);
    console.log(`Capture Status: ${captureResponse.status}`);
    
    if (captureResponse.status === 'CAPTURED' || captureResponse.status === 'AUTHORIZED') {
        console.log('✅ Payment captured successfully');
        return 'success';
    } else {
        console.log(`⚠️  Capture returned: ${captureResponse.status}`);
        return 'partial';
    }
}
```

</details>


<details>
<summary><b>Paysafe</b></summary>

**Configuration:**
```javascript
// Configuration not available
```

**Full Example:**
```javascript
async function processCheckoutCard(connectorName, credentials) {
    console.log('\n' + '='.repeat(60));
    console.log(`Processing Card Payment via ${connectorName}`);
    console.log('='.repeat(60));
    
    // Load probe data for this connector
    const probeData = loadProbeData(connectorName);
    
    // Get configuration
    const config = getConnectorConfig(connectorName, credentials);
    const client = new PaymentClient(config);
    
    // Build authorize request
    const authRequest = buildAuthorizeRequest(probeData, 'MANUAL');
    
    console.log('\nRequest:', JSON.stringify(authRequest, null, 2));
    
    // Step 1: Authorize
    console.log('\n[1/2] Authorizing...');
    const authResponse = await client.authorize(authRequest);
    
    console.log(`Status: ${authResponse.status}`);
    console.log(`Connector Transaction ID: ${authResponse.connectorTransactionId}`);
    
    // Handle status
    if (authResponse.status === 'FAILED') {
        console.log(`❌ Payment declined: ${authResponse.errorMessage || 'Unknown error'}`);
        return 'failed';
    }
    
    if (authResponse.status === 'PENDING') {
        console.log('⏳ Payment pending - awaiting async confirmation');
        console.log('   (In production: wait for webhook, then poll get())');
        return 'pending';
    }
    
    if (authResponse.status !== 'AUTHORIZED') {
        console.log(`⚠️  Unexpected status: ${authResponse.status}`);
        return 'error';
    }
    
    console.log('✅ Funds reserved successfully');
    
    // Step 2: Capture
    console.log('\n[2/2] Capturing...');
    const captureRequest = buildCaptureRequest(
        authResponse.connectorTransactionId,
        authRequest.amount
    );
    
    const captureResponse = await client.capture(captureRequest);
    console.log(`Capture Status: ${captureResponse.status}`);
    
    if (captureResponse.status === 'CAPTURED' || captureResponse.status === 'AUTHORIZED') {
        console.log('✅ Payment captured successfully');
        return 'success';
    } else {
        console.log(`⚠️  Capture returned: ${captureResponse.status}`);
        return 'partial';
    }
}
```

</details>


<details>
<summary><b>PlacetoPay</b></summary>

**Configuration:**
```javascript
// Configuration not available
```

**Full Example:**
```javascript
async function processCheckoutCard(connectorName, credentials) {
    console.log('\n' + '='.repeat(60));
    console.log(`Processing Card Payment via ${connectorName}`);
    console.log('='.repeat(60));
    
    // Load probe data for this connector
    const probeData = loadProbeData(connectorName);
    
    // Get configuration
    const config = getConnectorConfig(connectorName, credentials);
    const client = new PaymentClient(config);
    
    // Build authorize request
    const authRequest = buildAuthorizeRequest(probeData, 'MANUAL');
    
    console.log('\nRequest:', JSON.stringify(authRequest, null, 2));
    
    // Step 1: Authorize
    console.log('\n[1/2] Authorizing...');
    const authResponse = await client.authorize(authRequest);
    
    console.log(`Status: ${authResponse.status}`);
    console.log(`Connector Transaction ID: ${authResponse.connectorTransactionId}`);
    
    // Handle status
    if (authResponse.status === 'FAILED') {
        console.log(`❌ Payment declined: ${authResponse.errorMessage || 'Unknown error'}`);
        return 'failed';
    }
    
    if (authResponse.status === 'PENDING') {
        console.log('⏳ Payment pending - awaiting async confirmation');
        console.log('   (In production: wait for webhook, then poll get())');
        return 'pending';
    }
    
    if (authResponse.status !== 'AUTHORIZED') {
        console.log(`⚠️  Unexpected status: ${authResponse.status}`);
        return 'error';
    }
    
    console.log('✅ Funds reserved successfully');
    
    // Step 2: Capture
    console.log('\n[2/2] Capturing...');
    const captureRequest = buildCaptureRequest(
        authResponse.connectorTransactionId,
        authRequest.amount
    );
    
    const captureResponse = await client.capture(captureRequest);
    console.log(`Capture Status: ${captureResponse.status}`);
    
    if (captureResponse.status === 'CAPTURED' || captureResponse.status === 'AUTHORIZED') {
        console.log('✅ Payment captured successfully');
        return 'success';
    } else {
        console.log(`⚠️  Capture returned: ${captureResponse.status}`);
        return 'partial';
    }
}
```

</details>


<details>
<summary><b>PowerTranz</b></summary>

**Configuration:**
```javascript
// Configuration not available
```

**Full Example:**
```javascript
async function processCheckoutCard(connectorName, credentials) {
    console.log('\n' + '='.repeat(60));
    console.log(`Processing Card Payment via ${connectorName}`);
    console.log('='.repeat(60));
    
    // Load probe data for this connector
    const probeData = loadProbeData(connectorName);
    
    // Get configuration
    const config = getConnectorConfig(connectorName, credentials);
    const client = new PaymentClient(config);
    
    // Build authorize request
    const authRequest = buildAuthorizeRequest(probeData, 'MANUAL');
    
    console.log('\nRequest:', JSON.stringify(authRequest, null, 2));
    
    // Step 1: Authorize
    console.log('\n[1/2] Authorizing...');
    const authResponse = await client.authorize(authRequest);
    
    console.log(`Status: ${authResponse.status}`);
    console.log(`Connector Transaction ID: ${authResponse.connectorTransactionId}`);
    
    // Handle status
    if (authResponse.status === 'FAILED') {
        console.log(`❌ Payment declined: ${authResponse.errorMessage || 'Unknown error'}`);
        return 'failed';
    }
    
    if (authResponse.status === 'PENDING') {
        console.log('⏳ Payment pending - awaiting async confirmation');
        console.log('   (In production: wait for webhook, then poll get())');
        return 'pending';
    }
    
    if (authResponse.status !== 'AUTHORIZED') {
        console.log(`⚠️  Unexpected status: ${authResponse.status}`);
        return 'error';
    }
    
    console.log('✅ Funds reserved successfully');
    
    // Step 2: Capture
    console.log('\n[2/2] Capturing...');
    const captureRequest = buildCaptureRequest(
        authResponse.connectorTransactionId,
        authRequest.amount
    );
    
    const captureResponse = await client.capture(captureRequest);
    console.log(`Capture Status: ${captureResponse.status}`);
    
    if (captureResponse.status === 'CAPTURED' || captureResponse.status === 'AUTHORIZED') {
        console.log('✅ Payment captured successfully');
        return 'success';
    } else {
        console.log(`⚠️  Capture returned: ${captureResponse.status}`);
        return 'partial';
    }
}
```

</details>


<details>
<summary><b>Rapyd</b></summary>

**Configuration:**
```javascript
// Configuration not available
```

**Full Example:**
```javascript
async function processCheckoutCard(connectorName, credentials) {
    console.log('\n' + '='.repeat(60));
    console.log(`Processing Card Payment via ${connectorName}`);
    console.log('='.repeat(60));
    
    // Load probe data for this connector
    const probeData = loadProbeData(connectorName);
    
    // Get configuration
    const config = getConnectorConfig(connectorName, credentials);
    const client = new PaymentClient(config);
    
    // Build authorize request
    const authRequest = buildAuthorizeRequest(probeData, 'MANUAL');
    
    console.log('\nRequest:', JSON.stringify(authRequest, null, 2));
    
    // Step 1: Authorize
    console.log('\n[1/2] Authorizing...');
    const authResponse = await client.authorize(authRequest);
    
    console.log(`Status: ${authResponse.status}`);
    console.log(`Connector Transaction ID: ${authResponse.connectorTransactionId}`);
    
    // Handle status
    if (authResponse.status === 'FAILED') {
        console.log(`❌ Payment declined: ${authResponse.errorMessage || 'Unknown error'}`);
        return 'failed';
    }
    
    if (authResponse.status === 'PENDING') {
        console.log('⏳ Payment pending - awaiting async confirmation');
        console.log('   (In production: wait for webhook, then poll get())');
        return 'pending';
    }
    
    if (authResponse.status !== 'AUTHORIZED') {
        console.log(`⚠️  Unexpected status: ${authResponse.status}`);
        return 'error';
    }
    
    console.log('✅ Funds reserved successfully');
    
    // Step 2: Capture
    console.log('\n[2/2] Capturing...');
    const captureRequest = buildCaptureRequest(
        authResponse.connectorTransactionId,
        authRequest.amount
    );
    
    const captureResponse = await client.capture(captureRequest);
    console.log(`Capture Status: ${captureResponse.status}`);
    
    if (captureResponse.status === 'CAPTURED' || captureResponse.status === 'AUTHORIZED') {
        console.log('✅ Payment captured successfully');
        return 'success';
    } else {
        console.log(`⚠️  Capture returned: ${captureResponse.status}`);
        return 'partial';
    }
}
```

</details>


<details>
<summary><b>Razorpay</b></summary>

**Configuration:**
```javascript
// Configuration not available
```

**Full Example:**
```javascript
async function processCheckoutCard(connectorName, credentials) {
    console.log('\n' + '='.repeat(60));
    console.log(`Processing Card Payment via ${connectorName}`);
    console.log('='.repeat(60));
    
    // Load probe data for this connector
    const probeData = loadProbeData(connectorName);
    
    // Get configuration
    const config = getConnectorConfig(connectorName, credentials);
    const client = new PaymentClient(config);
    
    // Build authorize request
    const authRequest = buildAuthorizeRequest(probeData, 'MANUAL');
    
    console.log('\nRequest:', JSON.stringify(authRequest, null, 2));
    
    // Step 1: Authorize
    console.log('\n[1/2] Authorizing...');
    const authResponse = await client.authorize(authRequest);
    
    console.log(`Status: ${authResponse.status}`);
    console.log(`Connector Transaction ID: ${authResponse.connectorTransactionId}`);
    
    // Handle status
    if (authResponse.status === 'FAILED') {
        console.log(`❌ Payment declined: ${authResponse.errorMessage || 'Unknown error'}`);
        return 'failed';
    }
    
    if (authResponse.status === 'PENDING') {
        console.log('⏳ Payment pending - awaiting async confirmation');
        console.log('   (In production: wait for webhook, then poll get())');
        return 'pending';
    }
    
    if (authResponse.status !== 'AUTHORIZED') {
        console.log(`⚠️  Unexpected status: ${authResponse.status}`);
        return 'error';
    }
    
    console.log('✅ Funds reserved successfully');
    
    // Step 2: Capture
    console.log('\n[2/2] Capturing...');
    const captureRequest = buildCaptureRequest(
        authResponse.connectorTransactionId,
        authRequest.amount
    );
    
    const captureResponse = await client.capture(captureRequest);
    console.log(`Capture Status: ${captureResponse.status}`);
    
    if (captureResponse.status === 'CAPTURED' || captureResponse.status === 'AUTHORIZED') {
        console.log('✅ Payment captured successfully');
        return 'success';
    } else {
        console.log(`⚠️  Capture returned: ${captureResponse.status}`);
        return 'partial';
    }
}
```

</details>


<details>
<summary><b>Revolut</b></summary>

**Configuration:**
```javascript
// Configuration not available
```

**Full Example:**
```javascript
async function processCheckoutCard(connectorName, credentials) {
    console.log('\n' + '='.repeat(60));
    console.log(`Processing Card Payment via ${connectorName}`);
    console.log('='.repeat(60));
    
    // Load probe data for this connector
    const probeData = loadProbeData(connectorName);
    
    // Get configuration
    const config = getConnectorConfig(connectorName, credentials);
    const client = new PaymentClient(config);
    
    // Build authorize request
    const authRequest = buildAuthorizeRequest(probeData, 'MANUAL');
    
    console.log('\nRequest:', JSON.stringify(authRequest, null, 2));
    
    // Step 1: Authorize
    console.log('\n[1/2] Authorizing...');
    const authResponse = await client.authorize(authRequest);
    
    console.log(`Status: ${authResponse.status}`);
    console.log(`Connector Transaction ID: ${authResponse.connectorTransactionId}`);
    
    // Handle status
    if (authResponse.status === 'FAILED') {
        console.log(`❌ Payment declined: ${authResponse.errorMessage || 'Unknown error'}`);
        return 'failed';
    }
    
    if (authResponse.status === 'PENDING') {
        console.log('⏳ Payment pending - awaiting async confirmation');
        console.log('   (In production: wait for webhook, then poll get())');
        return 'pending';
    }
    
    if (authResponse.status !== 'AUTHORIZED') {
        console.log(`⚠️  Unexpected status: ${authResponse.status}`);
        return 'error';
    }
    
    console.log('✅ Funds reserved successfully');
    
    // Step 2: Capture
    console.log('\n[2/2] Capturing...');
    const captureRequest = buildCaptureRequest(
        authResponse.connectorTransactionId,
        authRequest.amount
    );
    
    const captureResponse = await client.capture(captureRequest);
    console.log(`Capture Status: ${captureResponse.status}`);
    
    if (captureResponse.status === 'CAPTURED' || captureResponse.status === 'AUTHORIZED') {
        console.log('✅ Payment captured successfully');
        return 'success';
    } else {
        console.log(`⚠️  Capture returned: ${captureResponse.status}`);
        return 'partial';
    }
}
```

</details>


<details>
<summary><b>Revolv3</b></summary>

**Configuration:**
```javascript
// Configuration not available
```

**Full Example:**
```javascript
async function processCheckoutCard(connectorName, credentials) {
    console.log('\n' + '='.repeat(60));
    console.log(`Processing Card Payment via ${connectorName}`);
    console.log('='.repeat(60));
    
    // Load probe data for this connector
    const probeData = loadProbeData(connectorName);
    
    // Get configuration
    const config = getConnectorConfig(connectorName, credentials);
    const client = new PaymentClient(config);
    
    // Build authorize request
    const authRequest = buildAuthorizeRequest(probeData, 'MANUAL');
    
    console.log('\nRequest:', JSON.stringify(authRequest, null, 2));
    
    // Step 1: Authorize
    console.log('\n[1/2] Authorizing...');
    const authResponse = await client.authorize(authRequest);
    
    console.log(`Status: ${authResponse.status}`);
    console.log(`Connector Transaction ID: ${authResponse.connectorTransactionId}`);
    
    // Handle status
    if (authResponse.status === 'FAILED') {
        console.log(`❌ Payment declined: ${authResponse.errorMessage || 'Unknown error'}`);
        return 'failed';
    }
    
    if (authResponse.status === 'PENDING') {
        console.log('⏳ Payment pending - awaiting async confirmation');
        console.log('   (In production: wait for webhook, then poll get())');
        return 'pending';
    }
    
    if (authResponse.status !== 'AUTHORIZED') {
        console.log(`⚠️  Unexpected status: ${authResponse.status}`);
        return 'error';
    }
    
    console.log('✅ Funds reserved successfully');
    
    // Step 2: Capture
    console.log('\n[2/2] Capturing...');
    const captureRequest = buildCaptureRequest(
        authResponse.connectorTransactionId,
        authRequest.amount
    );
    
    const captureResponse = await client.capture(captureRequest);
    console.log(`Capture Status: ${captureResponse.status}`);
    
    if (captureResponse.status === 'CAPTURED' || captureResponse.status === 'AUTHORIZED') {
        console.log('✅ Payment captured successfully');
        return 'success';
    } else {
        console.log(`⚠️  Capture returned: ${captureResponse.status}`);
        return 'partial';
    }
}
```

</details>


<details>
<summary><b>Shift4</b></summary>

**Configuration:**
```javascript
// Configuration not available
```

**Full Example:**
```javascript
async function processCheckoutCard(connectorName, credentials) {
    console.log('\n' + '='.repeat(60));
    console.log(`Processing Card Payment via ${connectorName}`);
    console.log('='.repeat(60));
    
    // Load probe data for this connector
    const probeData = loadProbeData(connectorName);
    
    // Get configuration
    const config = getConnectorConfig(connectorName, credentials);
    const client = new PaymentClient(config);
    
    // Build authorize request
    const authRequest = buildAuthorizeRequest(probeData, 'MANUAL');
    
    console.log('\nRequest:', JSON.stringify(authRequest, null, 2));
    
    // Step 1: Authorize
    console.log('\n[1/2] Authorizing...');
    const authResponse = await client.authorize(authRequest);
    
    console.log(`Status: ${authResponse.status}`);
    console.log(`Connector Transaction ID: ${authResponse.connectorTransactionId}`);
    
    // Handle status
    if (authResponse.status === 'FAILED') {
        console.log(`❌ Payment declined: ${authResponse.errorMessage || 'Unknown error'}`);
        return 'failed';
    }
    
    if (authResponse.status === 'PENDING') {
        console.log('⏳ Payment pending - awaiting async confirmation');
        console.log('   (In production: wait for webhook, then poll get())');
        return 'pending';
    }
    
    if (authResponse.status !== 'AUTHORIZED') {
        console.log(`⚠️  Unexpected status: ${authResponse.status}`);
        return 'error';
    }
    
    console.log('✅ Funds reserved successfully');
    
    // Step 2: Capture
    console.log('\n[2/2] Capturing...');
    const captureRequest = buildCaptureRequest(
        authResponse.connectorTransactionId,
        authRequest.amount
    );
    
    const captureResponse = await client.capture(captureRequest);
    console.log(`Capture Status: ${captureResponse.status}`);
    
    if (captureResponse.status === 'CAPTURED' || captureResponse.status === 'AUTHORIZED') {
        console.log('✅ Payment captured successfully');
        return 'success';
    } else {
        console.log(`⚠️  Capture returned: ${captureResponse.status}`);
        return 'partial';
    }
}
```

</details>


<details>
<summary><b>Silverflow</b></summary>

**Configuration:**
```javascript
// Configuration not available
```

**Full Example:**
```javascript
async function processCheckoutCard(connectorName, credentials) {
    console.log('\n' + '='.repeat(60));
    console.log(`Processing Card Payment via ${connectorName}`);
    console.log('='.repeat(60));
    
    // Load probe data for this connector
    const probeData = loadProbeData(connectorName);
    
    // Get configuration
    const config = getConnectorConfig(connectorName, credentials);
    const client = new PaymentClient(config);
    
    // Build authorize request
    const authRequest = buildAuthorizeRequest(probeData, 'MANUAL');
    
    console.log('\nRequest:', JSON.stringify(authRequest, null, 2));
    
    // Step 1: Authorize
    console.log('\n[1/2] Authorizing...');
    const authResponse = await client.authorize(authRequest);
    
    console.log(`Status: ${authResponse.status}`);
    console.log(`Connector Transaction ID: ${authResponse.connectorTransactionId}`);
    
    // Handle status
    if (authResponse.status === 'FAILED') {
        console.log(`❌ Payment declined: ${authResponse.errorMessage || 'Unknown error'}`);
        return 'failed';
    }
    
    if (authResponse.status === 'PENDING') {
        console.log('⏳ Payment pending - awaiting async confirmation');
        console.log('   (In production: wait for webhook, then poll get())');
        return 'pending';
    }
    
    if (authResponse.status !== 'AUTHORIZED') {
        console.log(`⚠️  Unexpected status: ${authResponse.status}`);
        return 'error';
    }
    
    console.log('✅ Funds reserved successfully');
    
    // Step 2: Capture
    console.log('\n[2/2] Capturing...');
    const captureRequest = buildCaptureRequest(
        authResponse.connectorTransactionId,
        authRequest.amount
    );
    
    const captureResponse = await client.capture(captureRequest);
    console.log(`Capture Status: ${captureResponse.status}`);
    
    if (captureResponse.status === 'CAPTURED' || captureResponse.status === 'AUTHORIZED') {
        console.log('✅ Payment captured successfully');
        return 'success';
    } else {
        console.log(`⚠️  Capture returned: ${captureResponse.status}`);
        return 'partial';
    }
}
```

</details>


<details>
<summary><b>Stax</b></summary>

**Configuration:**
```javascript
// Configuration not available
```

**Full Example:**
```javascript
async function processCheckoutCard(connectorName, credentials) {
    console.log('\n' + '='.repeat(60));
    console.log(`Processing Card Payment via ${connectorName}`);
    console.log('='.repeat(60));
    
    // Load probe data for this connector
    const probeData = loadProbeData(connectorName);
    
    // Get configuration
    const config = getConnectorConfig(connectorName, credentials);
    const client = new PaymentClient(config);
    
    // Build authorize request
    const authRequest = buildAuthorizeRequest(probeData, 'MANUAL');
    
    console.log('\nRequest:', JSON.stringify(authRequest, null, 2));
    
    // Step 1: Authorize
    console.log('\n[1/2] Authorizing...');
    const authResponse = await client.authorize(authRequest);
    
    console.log(`Status: ${authResponse.status}`);
    console.log(`Connector Transaction ID: ${authResponse.connectorTransactionId}`);
    
    // Handle status
    if (authResponse.status === 'FAILED') {
        console.log(`❌ Payment declined: ${authResponse.errorMessage || 'Unknown error'}`);
        return 'failed';
    }
    
    if (authResponse.status === 'PENDING') {
        console.log('⏳ Payment pending - awaiting async confirmation');
        console.log('   (In production: wait for webhook, then poll get())');
        return 'pending';
    }
    
    if (authResponse.status !== 'AUTHORIZED') {
        console.log(`⚠️  Unexpected status: ${authResponse.status}`);
        return 'error';
    }
    
    console.log('✅ Funds reserved successfully');
    
    // Step 2: Capture
    console.log('\n[2/2] Capturing...');
    const captureRequest = buildCaptureRequest(
        authResponse.connectorTransactionId,
        authRequest.amount
    );
    
    const captureResponse = await client.capture(captureRequest);
    console.log(`Capture Status: ${captureResponse.status}`);
    
    if (captureResponse.status === 'CAPTURED' || captureResponse.status === 'AUTHORIZED') {
        console.log('✅ Payment captured successfully');
        return 'success';
    } else {
        console.log(`⚠️  Capture returned: ${captureResponse.status}`);
        return 'partial';
    }
}
```

</details>


<details>
<summary><b>Stripe</b></summary>

**Configuration:**
```javascript
function getStripeConfig(apiKey) {
    const config = ConnectorConfig.create({
        options: SdkOptions.create({ environment: Environment.SANDBOX }),
    });
    config.connectorConfig = ConnectorSpecificConfig.create({
        stripe: { apiKey: { value: apiKey } }
    });
    return config;
}
```

**Full Example:**
```javascript
async function processCheckoutCard(connectorName, credentials) {
    console.log('\n' + '='.repeat(60));
    console.log(`Processing Card Payment via ${connectorName}`);
    console.log('='.repeat(60));
    
    // Load probe data for this connector
    const probeData = loadProbeData(connectorName);
    
    // Get configuration
    const config = getConnectorConfig(connectorName, credentials);
    const client = new PaymentClient(config);
    
    // Build authorize request
    const authRequest = buildAuthorizeRequest(probeData, 'MANUAL');
    
    console.log('\nRequest:', JSON.stringify(authRequest, null, 2));
    
    // Step 1: Authorize
    console.log('\n[1/2] Authorizing...');
    const authResponse = await client.authorize(authRequest);
    
    console.log(`Status: ${authResponse.status}`);
    console.log(`Connector Transaction ID: ${authResponse.connectorTransactionId}`);
    
    // Handle status
    if (authResponse.status === 'FAILED') {
        console.log(`❌ Payment declined: ${authResponse.errorMessage || 'Unknown error'}`);
        return 'failed';
    }
    
    if (authResponse.status === 'PENDING') {
        console.log('⏳ Payment pending - awaiting async confirmation');
        console.log('   (In production: wait for webhook, then poll get())');
        return 'pending';
    }
    
    if (authResponse.status !== 'AUTHORIZED') {
        console.log(`⚠️  Unexpected status: ${authResponse.status}`);
        return 'error';
    }
    
    console.log('✅ Funds reserved successfully');
    
    // Step 2: Capture
    console.log('\n[2/2] Capturing...');
    const captureRequest = buildCaptureRequest(
        authResponse.connectorTransactionId,
        authRequest.amount
    );
    
    const captureResponse = await client.capture(captureRequest);
    console.log(`Capture Status: ${captureResponse.status}`);
    
    if (captureResponse.status === 'CAPTURED' || captureResponse.status === 'AUTHORIZED') {
        console.log('✅ Payment captured successfully');
        return 'success';
    } else {
        console.log(`⚠️  Capture returned: ${captureResponse.status}`);
        return 'partial';
    }
}
```

</details>


<details>
<summary><b>Trust Payments</b></summary>

**Configuration:**
```javascript
// Configuration not available
```

**Full Example:**
```javascript
async function processCheckoutCard(connectorName, credentials) {
    console.log('\n' + '='.repeat(60));
    console.log(`Processing Card Payment via ${connectorName}`);
    console.log('='.repeat(60));
    
    // Load probe data for this connector
    const probeData = loadProbeData(connectorName);
    
    // Get configuration
    const config = getConnectorConfig(connectorName, credentials);
    const client = new PaymentClient(config);
    
    // Build authorize request
    const authRequest = buildAuthorizeRequest(probeData, 'MANUAL');
    
    console.log('\nRequest:', JSON.stringify(authRequest, null, 2));
    
    // Step 1: Authorize
    console.log('\n[1/2] Authorizing...');
    const authResponse = await client.authorize(authRequest);
    
    console.log(`Status: ${authResponse.status}`);
    console.log(`Connector Transaction ID: ${authResponse.connectorTransactionId}`);
    
    // Handle status
    if (authResponse.status === 'FAILED') {
        console.log(`❌ Payment declined: ${authResponse.errorMessage || 'Unknown error'}`);
        return 'failed';
    }
    
    if (authResponse.status === 'PENDING') {
        console.log('⏳ Payment pending - awaiting async confirmation');
        console.log('   (In production: wait for webhook, then poll get())');
        return 'pending';
    }
    
    if (authResponse.status !== 'AUTHORIZED') {
        console.log(`⚠️  Unexpected status: ${authResponse.status}`);
        return 'error';
    }
    
    console.log('✅ Funds reserved successfully');
    
    // Step 2: Capture
    console.log('\n[2/2] Capturing...');
    const captureRequest = buildCaptureRequest(
        authResponse.connectorTransactionId,
        authRequest.amount
    );
    
    const captureResponse = await client.capture(captureRequest);
    console.log(`Capture Status: ${captureResponse.status}`);
    
    if (captureResponse.status === 'CAPTURED' || captureResponse.status === 'AUTHORIZED') {
        console.log('✅ Payment captured successfully');
        return 'success';
    } else {
        console.log(`⚠️  Capture returned: ${captureResponse.status}`);
        return 'partial';
    }
}
```

</details>


<details>
<summary><b>TSYS</b></summary>

**Configuration:**
```javascript
// Configuration not available
```

**Full Example:**
```javascript
async function processCheckoutCard(connectorName, credentials) {
    console.log('\n' + '='.repeat(60));
    console.log(`Processing Card Payment via ${connectorName}`);
    console.log('='.repeat(60));
    
    // Load probe data for this connector
    const probeData = loadProbeData(connectorName);
    
    // Get configuration
    const config = getConnectorConfig(connectorName, credentials);
    const client = new PaymentClient(config);
    
    // Build authorize request
    const authRequest = buildAuthorizeRequest(probeData, 'MANUAL');
    
    console.log('\nRequest:', JSON.stringify(authRequest, null, 2));
    
    // Step 1: Authorize
    console.log('\n[1/2] Authorizing...');
    const authResponse = await client.authorize(authRequest);
    
    console.log(`Status: ${authResponse.status}`);
    console.log(`Connector Transaction ID: ${authResponse.connectorTransactionId}`);
    
    // Handle status
    if (authResponse.status === 'FAILED') {
        console.log(`❌ Payment declined: ${authResponse.errorMessage || 'Unknown error'}`);
        return 'failed';
    }
    
    if (authResponse.status === 'PENDING') {
        console.log('⏳ Payment pending - awaiting async confirmation');
        console.log('   (In production: wait for webhook, then poll get())');
        return 'pending';
    }
    
    if (authResponse.status !== 'AUTHORIZED') {
        console.log(`⚠️  Unexpected status: ${authResponse.status}`);
        return 'error';
    }
    
    console.log('✅ Funds reserved successfully');
    
    // Step 2: Capture
    console.log('\n[2/2] Capturing...');
    const captureRequest = buildCaptureRequest(
        authResponse.connectorTransactionId,
        authRequest.amount
    );
    
    const captureResponse = await client.capture(captureRequest);
    console.log(`Capture Status: ${captureResponse.status}`);
    
    if (captureResponse.status === 'CAPTURED' || captureResponse.status === 'AUTHORIZED') {
        console.log('✅ Payment captured successfully');
        return 'success';
    } else {
        console.log(`⚠️  Capture returned: ${captureResponse.status}`);
        return 'partial';
    }
}
```

</details>


<details>
<summary><b>Wells Fargo</b></summary>

**Configuration:**
```javascript
// Configuration not available
```

**Full Example:**
```javascript
async function processCheckoutCard(connectorName, credentials) {
    console.log('\n' + '='.repeat(60));
    console.log(`Processing Card Payment via ${connectorName}`);
    console.log('='.repeat(60));
    
    // Load probe data for this connector
    const probeData = loadProbeData(connectorName);
    
    // Get configuration
    const config = getConnectorConfig(connectorName, credentials);
    const client = new PaymentClient(config);
    
    // Build authorize request
    const authRequest = buildAuthorizeRequest(probeData, 'MANUAL');
    
    console.log('\nRequest:', JSON.stringify(authRequest, null, 2));
    
    // Step 1: Authorize
    console.log('\n[1/2] Authorizing...');
    const authResponse = await client.authorize(authRequest);
    
    console.log(`Status: ${authResponse.status}`);
    console.log(`Connector Transaction ID: ${authResponse.connectorTransactionId}`);
    
    // Handle status
    if (authResponse.status === 'FAILED') {
        console.log(`❌ Payment declined: ${authResponse.errorMessage || 'Unknown error'}`);
        return 'failed';
    }
    
    if (authResponse.status === 'PENDING') {
        console.log('⏳ Payment pending - awaiting async confirmation');
        console.log('   (In production: wait for webhook, then poll get())');
        return 'pending';
    }
    
    if (authResponse.status !== 'AUTHORIZED') {
        console.log(`⚠️  Unexpected status: ${authResponse.status}`);
        return 'error';
    }
    
    console.log('✅ Funds reserved successfully');
    
    // Step 2: Capture
    console.log('\n[2/2] Capturing...');
    const captureRequest = buildCaptureRequest(
        authResponse.connectorTransactionId,
        authRequest.amount
    );
    
    const captureResponse = await client.capture(captureRequest);
    console.log(`Capture Status: ${captureResponse.status}`);
    
    if (captureResponse.status === 'CAPTURED' || captureResponse.status === 'AUTHORIZED') {
        console.log('✅ Payment captured successfully');
        return 'success';
    } else {
        console.log(`⚠️  Capture returned: ${captureResponse.status}`);
        return 'partial';
    }
}
```

</details>


<details>
<summary><b>Worldpay</b></summary>

**Configuration:**
```javascript
// Configuration not available
```

**Full Example:**
```javascript
async function processCheckoutCard(connectorName, credentials) {
    console.log('\n' + '='.repeat(60));
    console.log(`Processing Card Payment via ${connectorName}`);
    console.log('='.repeat(60));
    
    // Load probe data for this connector
    const probeData = loadProbeData(connectorName);
    
    // Get configuration
    const config = getConnectorConfig(connectorName, credentials);
    const client = new PaymentClient(config);
    
    // Build authorize request
    const authRequest = buildAuthorizeRequest(probeData, 'MANUAL');
    
    console.log('\nRequest:', JSON.stringify(authRequest, null, 2));
    
    // Step 1: Authorize
    console.log('\n[1/2] Authorizing...');
    const authResponse = await client.authorize(authRequest);
    
    console.log(`Status: ${authResponse.status}`);
    console.log(`Connector Transaction ID: ${authResponse.connectorTransactionId}`);
    
    // Handle status
    if (authResponse.status === 'FAILED') {
        console.log(`❌ Payment declined: ${authResponse.errorMessage || 'Unknown error'}`);
        return 'failed';
    }
    
    if (authResponse.status === 'PENDING') {
        console.log('⏳ Payment pending - awaiting async confirmation');
        console.log('   (In production: wait for webhook, then poll get())');
        return 'pending';
    }
    
    if (authResponse.status !== 'AUTHORIZED') {
        console.log(`⚠️  Unexpected status: ${authResponse.status}`);
        return 'error';
    }
    
    console.log('✅ Funds reserved successfully');
    
    // Step 2: Capture
    console.log('\n[2/2] Capturing...');
    const captureRequest = buildCaptureRequest(
        authResponse.connectorTransactionId,
        authRequest.amount
    );
    
    const captureResponse = await client.capture(captureRequest);
    console.log(`Capture Status: ${captureResponse.status}`);
    
    if (captureResponse.status === 'CAPTURED' || captureResponse.status === 'AUTHORIZED') {
        console.log('✅ Payment captured successfully');
        return 'success';
    } else {
        console.log(`⚠️  Capture returned: ${captureResponse.status}`);
        return 'partial';
    }
}
```

</details>


<details>
<summary><b>Worldpay Vantiv</b></summary>

**Configuration:**
```javascript
// Configuration not available
```

**Full Example:**
```javascript
async function processCheckoutCard(connectorName, credentials) {
    console.log('\n' + '='.repeat(60));
    console.log(`Processing Card Payment via ${connectorName}`);
    console.log('='.repeat(60));
    
    // Load probe data for this connector
    const probeData = loadProbeData(connectorName);
    
    // Get configuration
    const config = getConnectorConfig(connectorName, credentials);
    const client = new PaymentClient(config);
    
    // Build authorize request
    const authRequest = buildAuthorizeRequest(probeData, 'MANUAL');
    
    console.log('\nRequest:', JSON.stringify(authRequest, null, 2));
    
    // Step 1: Authorize
    console.log('\n[1/2] Authorizing...');
    const authResponse = await client.authorize(authRequest);
    
    console.log(`Status: ${authResponse.status}`);
    console.log(`Connector Transaction ID: ${authResponse.connectorTransactionId}`);
    
    // Handle status
    if (authResponse.status === 'FAILED') {
        console.log(`❌ Payment declined: ${authResponse.errorMessage || 'Unknown error'}`);
        return 'failed';
    }
    
    if (authResponse.status === 'PENDING') {
        console.log('⏳ Payment pending - awaiting async confirmation');
        console.log('   (In production: wait for webhook, then poll get())');
        return 'pending';
    }
    
    if (authResponse.status !== 'AUTHORIZED') {
        console.log(`⚠️  Unexpected status: ${authResponse.status}`);
        return 'error';
    }
    
    console.log('✅ Funds reserved successfully');
    
    // Step 2: Capture
    console.log('\n[2/2] Capturing...');
    const captureRequest = buildCaptureRequest(
        authResponse.connectorTransactionId,
        authRequest.amount
    );
    
    const captureResponse = await client.capture(captureRequest);
    console.log(`Capture Status: ${captureResponse.status}`);
    
    if (captureResponse.status === 'CAPTURED' || captureResponse.status === 'AUTHORIZED') {
        console.log('✅ Payment captured successfully');
        return 'success';
    } else {
        console.log(`⚠️  Capture returned: ${captureResponse.status}`);
        return 'partial';
    }
}
```

</details>


<details>
<summary><b>Worldpay XML</b></summary>

**Configuration:**
```javascript
// Configuration not available
```

**Full Example:**
```javascript
async function processCheckoutCard(connectorName, credentials) {
    console.log('\n' + '='.repeat(60));
    console.log(`Processing Card Payment via ${connectorName}`);
    console.log('='.repeat(60));
    
    // Load probe data for this connector
    const probeData = loadProbeData(connectorName);
    
    // Get configuration
    const config = getConnectorConfig(connectorName, credentials);
    const client = new PaymentClient(config);
    
    // Build authorize request
    const authRequest = buildAuthorizeRequest(probeData, 'MANUAL');
    
    console.log('\nRequest:', JSON.stringify(authRequest, null, 2));
    
    // Step 1: Authorize
    console.log('\n[1/2] Authorizing...');
    const authResponse = await client.authorize(authRequest);
    
    console.log(`Status: ${authResponse.status}`);
    console.log(`Connector Transaction ID: ${authResponse.connectorTransactionId}`);
    
    // Handle status
    if (authResponse.status === 'FAILED') {
        console.log(`❌ Payment declined: ${authResponse.errorMessage || 'Unknown error'}`);
        return 'failed';
    }
    
    if (authResponse.status === 'PENDING') {
        console.log('⏳ Payment pending - awaiting async confirmation');
        console.log('   (In production: wait for webhook, then poll get())');
        return 'pending';
    }
    
    if (authResponse.status !== 'AUTHORIZED') {
        console.log(`⚠️  Unexpected status: ${authResponse.status}`);
        return 'error';
    }
    
    console.log('✅ Funds reserved successfully');
    
    // Step 2: Capture
    console.log('\n[2/2] Capturing...');
    const captureRequest = buildCaptureRequest(
        authResponse.connectorTransactionId,
        authRequest.amount
    );
    
    const captureResponse = await client.capture(captureRequest);
    console.log(`Capture Status: ${captureResponse.status}`);
    
    if (captureResponse.status === 'CAPTURED' || captureResponse.status === 'AUTHORIZED') {
        console.log('✅ Payment captured successfully');
        return 'success';
    } else {
        console.log(`⚠️  Capture returned: ${captureResponse.status}`);
        return 'partial';
    }
}
```

</details>


<details>
<summary><b>Xendit</b></summary>

**Configuration:**
```javascript
// Configuration not available
```

**Full Example:**
```javascript
async function processCheckoutCard(connectorName, credentials) {
    console.log('\n' + '='.repeat(60));
    console.log(`Processing Card Payment via ${connectorName}`);
    console.log('='.repeat(60));
    
    // Load probe data for this connector
    const probeData = loadProbeData(connectorName);
    
    // Get configuration
    const config = getConnectorConfig(connectorName, credentials);
    const client = new PaymentClient(config);
    
    // Build authorize request
    const authRequest = buildAuthorizeRequest(probeData, 'MANUAL');
    
    console.log('\nRequest:', JSON.stringify(authRequest, null, 2));
    
    // Step 1: Authorize
    console.log('\n[1/2] Authorizing...');
    const authResponse = await client.authorize(authRequest);
    
    console.log(`Status: ${authResponse.status}`);
    console.log(`Connector Transaction ID: ${authResponse.connectorTransactionId}`);
    
    // Handle status
    if (authResponse.status === 'FAILED') {
        console.log(`❌ Payment declined: ${authResponse.errorMessage || 'Unknown error'}`);
        return 'failed';
    }
    
    if (authResponse.status === 'PENDING') {
        console.log('⏳ Payment pending - awaiting async confirmation');
        console.log('   (In production: wait for webhook, then poll get())');
        return 'pending';
    }
    
    if (authResponse.status !== 'AUTHORIZED') {
        console.log(`⚠️  Unexpected status: ${authResponse.status}`);
        return 'error';
    }
    
    console.log('✅ Funds reserved successfully');
    
    // Step 2: Capture
    console.log('\n[2/2] Capturing...');
    const captureRequest = buildCaptureRequest(
        authResponse.connectorTransactionId,
        authRequest.amount
    );
    
    const captureResponse = await client.capture(captureRequest);
    console.log(`Capture Status: ${captureResponse.status}`);
    
    if (captureResponse.status === 'CAPTURED' || captureResponse.status === 'AUTHORIZED') {
        console.log('✅ Payment captured successfully');
        return 'success';
    } else {
        console.log(`⚠️  Capture returned: ${captureResponse.status}`);
        return 'partial';
    }
}
```

</details>


<details>
<summary><b>Zift</b></summary>

**Configuration:**
```javascript
// Configuration not available
```

**Full Example:**
```javascript
async function processCheckoutCard(connectorName, credentials) {
    console.log('\n' + '='.repeat(60));
    console.log(`Processing Card Payment via ${connectorName}`);
    console.log('='.repeat(60));
    
    // Load probe data for this connector
    const probeData = loadProbeData(connectorName);
    
    // Get configuration
    const config = getConnectorConfig(connectorName, credentials);
    const client = new PaymentClient(config);
    
    // Build authorize request
    const authRequest = buildAuthorizeRequest(probeData, 'MANUAL');
    
    console.log('\nRequest:', JSON.stringify(authRequest, null, 2));
    
    // Step 1: Authorize
    console.log('\n[1/2] Authorizing...');
    const authResponse = await client.authorize(authRequest);
    
    console.log(`Status: ${authResponse.status}`);
    console.log(`Connector Transaction ID: ${authResponse.connectorTransactionId}`);
    
    // Handle status
    if (authResponse.status === 'FAILED') {
        console.log(`❌ Payment declined: ${authResponse.errorMessage || 'Unknown error'}`);
        return 'failed';
    }
    
    if (authResponse.status === 'PENDING') {
        console.log('⏳ Payment pending - awaiting async confirmation');
        console.log('   (In production: wait for webhook, then poll get())');
        return 'pending';
    }
    
    if (authResponse.status !== 'AUTHORIZED') {
        console.log(`⚠️  Unexpected status: ${authResponse.status}`);
        return 'error';
    }
    
    console.log('✅ Funds reserved successfully');
    
    // Step 2: Capture
    console.log('\n[2/2] Capturing...');
    const captureRequest = buildCaptureRequest(
        authResponse.connectorTransactionId,
        authRequest.amount
    );
    
    const captureResponse = await client.capture(captureRequest);
    console.log(`Capture Status: ${captureResponse.status}`);
    
    if (captureResponse.status === 'CAPTURED' || captureResponse.status === 'AUTHORIZED') {
        console.log('✅ Payment captured successfully');
        return 'success';
    } else {
        console.log(`⚠️  Capture returned: ${captureResponse.status}`);
        return 'partial';
    }
}
```

</details>



### Kotlin


<details>
<summary><b>ACI</b></summary>

**Configuration:**
```kotlin
// Configuration not available
```

**Full Example:**
```kotlin
suspend fun processCheckoutCard(
    connectorName: String,
    credentials: Map<String, String>
): String {
    println("\n${"=".repeat(60)}")
    println("Processing Card Payment via $connectorName")
    println("${"=".repeat(60)}")
    
    // Load probe data for this connector
    val probeData = loadProbeData(connectorName)
    
    // Build config
    val config = getConnectorConfig(connectorName, credentials)
    val client = PaymentClient(config)
    
    // Build request from probe data
    val authRequest = buildAuthorizeRequest(probeData, CaptureMethod.MANUAL)
    
    println("\nRequest: ${Gson().toJson(authRequest)}")
    
    // Step 1: Authorize
    println("\n[1/2] Authorizing...")
    val authResponse = client.authorize(authRequest)
    
    println("Status: ${authResponse.status}")
    println("Connector Transaction ID: ${authResponse.connectorTransactionId}")
    
    // Handle status
    return when (authResponse.status) {
        payments.PaymentStatus.FAILURE -> {
            println("❌ Payment declined: ${authResponse.error?.message ?: "Unknown error"}")
            "failed"
        }
        payments.PaymentStatus.PENDING -> {
            println("⏳ Payment pending - awaiting async confirmation")
            println("   (In production: wait for webhook, then poll get())")
            "pending"
        }
        payments.PaymentStatus.AUTHORIZED -> {
            println("✅ Funds reserved successfully")
            
            // Step 2: Capture
            println("\n[2/2] Capturing...")
            val captureRequest = buildCaptureRequest(
                authResponse.connectorTransactionId,
                authResponse.amount
            )
            
            val captureResponse = client.capture(captureRequest)
            println("Capture Status: ${captureResponse.status}")
            
            when (captureResponse.status) {
                payments.PaymentStatus.CAPTURED, payments.PaymentStatus.AUTHORIZED -> {
                    println("✅ Payment captured successfully")
                    "success"
                }
                else -> {
                    println("⚠️  Capture returned: ${captureResponse.status}")
                    "partial"
                }
            }
        }
        else -> {
            println("⚠️  Unexpected status: ${authResponse.status}")
            "error"
        }
    }
}
```

</details>


<details>
<summary><b>Adyen</b></summary>

**Configuration:**
```kotlin
fun getAdyenConfig(apiKey: String, merchantAccount: String): ConnectorConfig {
    return ConnectorConfig.newBuilder().apply {
        options = SdkOptions.newBuilder().apply {
            environment = Environment.SANDBOX
        }.build()
        connectorConfig = ConnectorSpecificConfig.newBuilder().apply {
            adyenBuilder.apply {
                this.apiKey = apiKey
                this.merchantAccount = merchantAccount
            }
        }.build()
    }.build()
}
```

**Full Example:**
```kotlin
suspend fun processCheckoutCard(
    connectorName: String,
    credentials: Map<String, String>
): String {
    println("\n${"=".repeat(60)}")
    println("Processing Card Payment via $connectorName")
    println("${"=".repeat(60)}")
    
    // Load probe data for this connector
    val probeData = loadProbeData(connectorName)
    
    // Build config
    val config = getConnectorConfig(connectorName, credentials)
    val client = PaymentClient(config)
    
    // Build request from probe data
    val authRequest = buildAuthorizeRequest(probeData, CaptureMethod.MANUAL)
    
    println("\nRequest: ${Gson().toJson(authRequest)}")
    
    // Step 1: Authorize
    println("\n[1/2] Authorizing...")
    val authResponse = client.authorize(authRequest)
    
    println("Status: ${authResponse.status}")
    println("Connector Transaction ID: ${authResponse.connectorTransactionId}")
    
    // Handle status
    return when (authResponse.status) {
        payments.PaymentStatus.FAILURE -> {
            println("❌ Payment declined: ${authResponse.error?.message ?: "Unknown error"}")
            "failed"
        }
        payments.PaymentStatus.PENDING -> {
            println("⏳ Payment pending - awaiting async confirmation")
            println("   (In production: wait for webhook, then poll get())")
            "pending"
        }
        payments.PaymentStatus.AUTHORIZED -> {
            println("✅ Funds reserved successfully")
            
            // Step 2: Capture
            println("\n[2/2] Capturing...")
            val captureRequest = buildCaptureRequest(
                authResponse.connectorTransactionId,
                authResponse.amount
            )
            
            val captureResponse = client.capture(captureRequest)
            println("Capture Status: ${captureResponse.status}")
            
            when (captureResponse.status) {
                payments.PaymentStatus.CAPTURED, payments.PaymentStatus.AUTHORIZED -> {
                    println("✅ Payment captured successfully")
                    "success"
                }
                else -> {
                    println("⚠️  Capture returned: ${captureResponse.status}")
                    "partial"
                }
            }
        }
        else -> {
            println("⚠️  Unexpected status: ${authResponse.status}")
            "error"
        }
    }
}
```

</details>


<details>
<summary><b>Airwallex</b></summary>

**Configuration:**
```kotlin
// Configuration not available
```

**Full Example:**
```kotlin
suspend fun processCheckoutCard(
    connectorName: String,
    credentials: Map<String, String>
): String {
    println("\n${"=".repeat(60)}")
    println("Processing Card Payment via $connectorName")
    println("${"=".repeat(60)}")
    
    // Load probe data for this connector
    val probeData = loadProbeData(connectorName)
    
    // Build config
    val config = getConnectorConfig(connectorName, credentials)
    val client = PaymentClient(config)
    
    // Build request from probe data
    val authRequest = buildAuthorizeRequest(probeData, CaptureMethod.MANUAL)
    
    println("\nRequest: ${Gson().toJson(authRequest)}")
    
    // Step 1: Authorize
    println("\n[1/2] Authorizing...")
    val authResponse = client.authorize(authRequest)
    
    println("Status: ${authResponse.status}")
    println("Connector Transaction ID: ${authResponse.connectorTransactionId}")
    
    // Handle status
    return when (authResponse.status) {
        payments.PaymentStatus.FAILURE -> {
            println("❌ Payment declined: ${authResponse.error?.message ?: "Unknown error"}")
            "failed"
        }
        payments.PaymentStatus.PENDING -> {
            println("⏳ Payment pending - awaiting async confirmation")
            println("   (In production: wait for webhook, then poll get())")
            "pending"
        }
        payments.PaymentStatus.AUTHORIZED -> {
            println("✅ Funds reserved successfully")
            
            // Step 2: Capture
            println("\n[2/2] Capturing...")
            val captureRequest = buildCaptureRequest(
                authResponse.connectorTransactionId,
                authResponse.amount
            )
            
            val captureResponse = client.capture(captureRequest)
            println("Capture Status: ${captureResponse.status}")
            
            when (captureResponse.status) {
                payments.PaymentStatus.CAPTURED, payments.PaymentStatus.AUTHORIZED -> {
                    println("✅ Payment captured successfully")
                    "success"
                }
                else -> {
                    println("⚠️  Capture returned: ${captureResponse.status}")
                    "partial"
                }
            }
        }
        else -> {
            println("⚠️  Unexpected status: ${authResponse.status}")
            "error"
        }
    }
}
```

</details>


<details>
<summary><b>authipay</b></summary>

**Configuration:**
```kotlin
// Configuration not available
```

**Full Example:**
```kotlin
suspend fun processCheckoutCard(
    connectorName: String,
    credentials: Map<String, String>
): String {
    println("\n${"=".repeat(60)}")
    println("Processing Card Payment via $connectorName")
    println("${"=".repeat(60)}")
    
    // Load probe data for this connector
    val probeData = loadProbeData(connectorName)
    
    // Build config
    val config = getConnectorConfig(connectorName, credentials)
    val client = PaymentClient(config)
    
    // Build request from probe data
    val authRequest = buildAuthorizeRequest(probeData, CaptureMethod.MANUAL)
    
    println("\nRequest: ${Gson().toJson(authRequest)}")
    
    // Step 1: Authorize
    println("\n[1/2] Authorizing...")
    val authResponse = client.authorize(authRequest)
    
    println("Status: ${authResponse.status}")
    println("Connector Transaction ID: ${authResponse.connectorTransactionId}")
    
    // Handle status
    return when (authResponse.status) {
        payments.PaymentStatus.FAILURE -> {
            println("❌ Payment declined: ${authResponse.error?.message ?: "Unknown error"}")
            "failed"
        }
        payments.PaymentStatus.PENDING -> {
            println("⏳ Payment pending - awaiting async confirmation")
            println("   (In production: wait for webhook, then poll get())")
            "pending"
        }
        payments.PaymentStatus.AUTHORIZED -> {
            println("✅ Funds reserved successfully")
            
            // Step 2: Capture
            println("\n[2/2] Capturing...")
            val captureRequest = buildCaptureRequest(
                authResponse.connectorTransactionId,
                authResponse.amount
            )
            
            val captureResponse = client.capture(captureRequest)
            println("Capture Status: ${captureResponse.status}")
            
            when (captureResponse.status) {
                payments.PaymentStatus.CAPTURED, payments.PaymentStatus.AUTHORIZED -> {
                    println("✅ Payment captured successfully")
                    "success"
                }
                else -> {
                    println("⚠️  Capture returned: ${captureResponse.status}")
                    "partial"
                }
            }
        }
        else -> {
            println("⚠️  Unexpected status: ${authResponse.status}")
            "error"
        }
    }
}
```

</details>


<details>
<summary><b>Authorize.net</b></summary>

**Configuration:**
```kotlin
// Configuration not available
```

**Full Example:**
```kotlin
suspend fun processCheckoutCard(
    connectorName: String,
    credentials: Map<String, String>
): String {
    println("\n${"=".repeat(60)}")
    println("Processing Card Payment via $connectorName")
    println("${"=".repeat(60)}")
    
    // Load probe data for this connector
    val probeData = loadProbeData(connectorName)
    
    // Build config
    val config = getConnectorConfig(connectorName, credentials)
    val client = PaymentClient(config)
    
    // Build request from probe data
    val authRequest = buildAuthorizeRequest(probeData, CaptureMethod.MANUAL)
    
    println("\nRequest: ${Gson().toJson(authRequest)}")
    
    // Step 1: Authorize
    println("\n[1/2] Authorizing...")
    val authResponse = client.authorize(authRequest)
    
    println("Status: ${authResponse.status}")
    println("Connector Transaction ID: ${authResponse.connectorTransactionId}")
    
    // Handle status
    return when (authResponse.status) {
        payments.PaymentStatus.FAILURE -> {
            println("❌ Payment declined: ${authResponse.error?.message ?: "Unknown error"}")
            "failed"
        }
        payments.PaymentStatus.PENDING -> {
            println("⏳ Payment pending - awaiting async confirmation")
            println("   (In production: wait for webhook, then poll get())")
            "pending"
        }
        payments.PaymentStatus.AUTHORIZED -> {
            println("✅ Funds reserved successfully")
            
            // Step 2: Capture
            println("\n[2/2] Capturing...")
            val captureRequest = buildCaptureRequest(
                authResponse.connectorTransactionId,
                authResponse.amount
            )
            
            val captureResponse = client.capture(captureRequest)
            println("Capture Status: ${captureResponse.status}")
            
            when (captureResponse.status) {
                payments.PaymentStatus.CAPTURED, payments.PaymentStatus.AUTHORIZED -> {
                    println("✅ Payment captured successfully")
                    "success"
                }
                else -> {
                    println("⚠️  Capture returned: ${captureResponse.status}")
                    "partial"
                }
            }
        }
        else -> {
            println("⚠️  Unexpected status: ${authResponse.status}")
            "error"
        }
    }
}
```

</details>


<details>
<summary><b>Bambora</b></summary>

**Configuration:**
```kotlin
// Configuration not available
```

**Full Example:**
```kotlin
suspend fun processCheckoutCard(
    connectorName: String,
    credentials: Map<String, String>
): String {
    println("\n${"=".repeat(60)}")
    println("Processing Card Payment via $connectorName")
    println("${"=".repeat(60)}")
    
    // Load probe data for this connector
    val probeData = loadProbeData(connectorName)
    
    // Build config
    val config = getConnectorConfig(connectorName, credentials)
    val client = PaymentClient(config)
    
    // Build request from probe data
    val authRequest = buildAuthorizeRequest(probeData, CaptureMethod.MANUAL)
    
    println("\nRequest: ${Gson().toJson(authRequest)}")
    
    // Step 1: Authorize
    println("\n[1/2] Authorizing...")
    val authResponse = client.authorize(authRequest)
    
    println("Status: ${authResponse.status}")
    println("Connector Transaction ID: ${authResponse.connectorTransactionId}")
    
    // Handle status
    return when (authResponse.status) {
        payments.PaymentStatus.FAILURE -> {
            println("❌ Payment declined: ${authResponse.error?.message ?: "Unknown error"}")
            "failed"
        }
        payments.PaymentStatus.PENDING -> {
            println("⏳ Payment pending - awaiting async confirmation")
            println("   (In production: wait for webhook, then poll get())")
            "pending"
        }
        payments.PaymentStatus.AUTHORIZED -> {
            println("✅ Funds reserved successfully")
            
            // Step 2: Capture
            println("\n[2/2] Capturing...")
            val captureRequest = buildCaptureRequest(
                authResponse.connectorTransactionId,
                authResponse.amount
            )
            
            val captureResponse = client.capture(captureRequest)
            println("Capture Status: ${captureResponse.status}")
            
            when (captureResponse.status) {
                payments.PaymentStatus.CAPTURED, payments.PaymentStatus.AUTHORIZED -> {
                    println("✅ Payment captured successfully")
                    "success"
                }
                else -> {
                    println("⚠️  Capture returned: ${captureResponse.status}")
                    "partial"
                }
            }
        }
        else -> {
            println("⚠️  Unexpected status: ${authResponse.status}")
            "error"
        }
    }
}
```

</details>


<details>
<summary><b>Bambora APAC</b></summary>

**Configuration:**
```kotlin
// Configuration not available
```

**Full Example:**
```kotlin
suspend fun processCheckoutCard(
    connectorName: String,
    credentials: Map<String, String>
): String {
    println("\n${"=".repeat(60)}")
    println("Processing Card Payment via $connectorName")
    println("${"=".repeat(60)}")
    
    // Load probe data for this connector
    val probeData = loadProbeData(connectorName)
    
    // Build config
    val config = getConnectorConfig(connectorName, credentials)
    val client = PaymentClient(config)
    
    // Build request from probe data
    val authRequest = buildAuthorizeRequest(probeData, CaptureMethod.MANUAL)
    
    println("\nRequest: ${Gson().toJson(authRequest)}")
    
    // Step 1: Authorize
    println("\n[1/2] Authorizing...")
    val authResponse = client.authorize(authRequest)
    
    println("Status: ${authResponse.status}")
    println("Connector Transaction ID: ${authResponse.connectorTransactionId}")
    
    // Handle status
    return when (authResponse.status) {
        payments.PaymentStatus.FAILURE -> {
            println("❌ Payment declined: ${authResponse.error?.message ?: "Unknown error"}")
            "failed"
        }
        payments.PaymentStatus.PENDING -> {
            println("⏳ Payment pending - awaiting async confirmation")
            println("   (In production: wait for webhook, then poll get())")
            "pending"
        }
        payments.PaymentStatus.AUTHORIZED -> {
            println("✅ Funds reserved successfully")
            
            // Step 2: Capture
            println("\n[2/2] Capturing...")
            val captureRequest = buildCaptureRequest(
                authResponse.connectorTransactionId,
                authResponse.amount
            )
            
            val captureResponse = client.capture(captureRequest)
            println("Capture Status: ${captureResponse.status}")
            
            when (captureResponse.status) {
                payments.PaymentStatus.CAPTURED, payments.PaymentStatus.AUTHORIZED -> {
                    println("✅ Payment captured successfully")
                    "success"
                }
                else -> {
                    println("⚠️  Capture returned: ${captureResponse.status}")
                    "partial"
                }
            }
        }
        else -> {
            println("⚠️  Unexpected status: ${authResponse.status}")
            "error"
        }
    }
}
```

</details>


<details>
<summary><b>Bank of America</b></summary>

**Configuration:**
```kotlin
// Configuration not available
```

**Full Example:**
```kotlin
suspend fun processCheckoutCard(
    connectorName: String,
    credentials: Map<String, String>
): String {
    println("\n${"=".repeat(60)}")
    println("Processing Card Payment via $connectorName")
    println("${"=".repeat(60)}")
    
    // Load probe data for this connector
    val probeData = loadProbeData(connectorName)
    
    // Build config
    val config = getConnectorConfig(connectorName, credentials)
    val client = PaymentClient(config)
    
    // Build request from probe data
    val authRequest = buildAuthorizeRequest(probeData, CaptureMethod.MANUAL)
    
    println("\nRequest: ${Gson().toJson(authRequest)}")
    
    // Step 1: Authorize
    println("\n[1/2] Authorizing...")
    val authResponse = client.authorize(authRequest)
    
    println("Status: ${authResponse.status}")
    println("Connector Transaction ID: ${authResponse.connectorTransactionId}")
    
    // Handle status
    return when (authResponse.status) {
        payments.PaymentStatus.FAILURE -> {
            println("❌ Payment declined: ${authResponse.error?.message ?: "Unknown error"}")
            "failed"
        }
        payments.PaymentStatus.PENDING -> {
            println("⏳ Payment pending - awaiting async confirmation")
            println("   (In production: wait for webhook, then poll get())")
            "pending"
        }
        payments.PaymentStatus.AUTHORIZED -> {
            println("✅ Funds reserved successfully")
            
            // Step 2: Capture
            println("\n[2/2] Capturing...")
            val captureRequest = buildCaptureRequest(
                authResponse.connectorTransactionId,
                authResponse.amount
            )
            
            val captureResponse = client.capture(captureRequest)
            println("Capture Status: ${captureResponse.status}")
            
            when (captureResponse.status) {
                payments.PaymentStatus.CAPTURED, payments.PaymentStatus.AUTHORIZED -> {
                    println("✅ Payment captured successfully")
                    "success"
                }
                else -> {
                    println("⚠️  Capture returned: ${captureResponse.status}")
                    "partial"
                }
            }
        }
        else -> {
            println("⚠️  Unexpected status: ${authResponse.status}")
            "error"
        }
    }
}
```

</details>


<details>
<summary><b>Barclaycard</b></summary>

**Configuration:**
```kotlin
// Configuration not available
```

**Full Example:**
```kotlin
suspend fun processCheckoutCard(
    connectorName: String,
    credentials: Map<String, String>
): String {
    println("\n${"=".repeat(60)}")
    println("Processing Card Payment via $connectorName")
    println("${"=".repeat(60)}")
    
    // Load probe data for this connector
    val probeData = loadProbeData(connectorName)
    
    // Build config
    val config = getConnectorConfig(connectorName, credentials)
    val client = PaymentClient(config)
    
    // Build request from probe data
    val authRequest = buildAuthorizeRequest(probeData, CaptureMethod.MANUAL)
    
    println("\nRequest: ${Gson().toJson(authRequest)}")
    
    // Step 1: Authorize
    println("\n[1/2] Authorizing...")
    val authResponse = client.authorize(authRequest)
    
    println("Status: ${authResponse.status}")
    println("Connector Transaction ID: ${authResponse.connectorTransactionId}")
    
    // Handle status
    return when (authResponse.status) {
        payments.PaymentStatus.FAILURE -> {
            println("❌ Payment declined: ${authResponse.error?.message ?: "Unknown error"}")
            "failed"
        }
        payments.PaymentStatus.PENDING -> {
            println("⏳ Payment pending - awaiting async confirmation")
            println("   (In production: wait for webhook, then poll get())")
            "pending"
        }
        payments.PaymentStatus.AUTHORIZED -> {
            println("✅ Funds reserved successfully")
            
            // Step 2: Capture
            println("\n[2/2] Capturing...")
            val captureRequest = buildCaptureRequest(
                authResponse.connectorTransactionId,
                authResponse.amount
            )
            
            val captureResponse = client.capture(captureRequest)
            println("Capture Status: ${captureResponse.status}")
            
            when (captureResponse.status) {
                payments.PaymentStatus.CAPTURED, payments.PaymentStatus.AUTHORIZED -> {
                    println("✅ Payment captured successfully")
                    "success"
                }
                else -> {
                    println("⚠️  Capture returned: ${captureResponse.status}")
                    "partial"
                }
            }
        }
        else -> {
            println("⚠️  Unexpected status: ${authResponse.status}")
            "error"
        }
    }
}
```

</details>


<details>
<summary><b>Billwerk</b></summary>

**Configuration:**
```kotlin
// Configuration not available
```

**Full Example:**
```kotlin
suspend fun processCheckoutCard(
    connectorName: String,
    credentials: Map<String, String>
): String {
    println("\n${"=".repeat(60)}")
    println("Processing Card Payment via $connectorName")
    println("${"=".repeat(60)}")
    
    // Load probe data for this connector
    val probeData = loadProbeData(connectorName)
    
    // Build config
    val config = getConnectorConfig(connectorName, credentials)
    val client = PaymentClient(config)
    
    // Build request from probe data
    val authRequest = buildAuthorizeRequest(probeData, CaptureMethod.MANUAL)
    
    println("\nRequest: ${Gson().toJson(authRequest)}")
    
    // Step 1: Authorize
    println("\n[1/2] Authorizing...")
    val authResponse = client.authorize(authRequest)
    
    println("Status: ${authResponse.status}")
    println("Connector Transaction ID: ${authResponse.connectorTransactionId}")
    
    // Handle status
    return when (authResponse.status) {
        payments.PaymentStatus.FAILURE -> {
            println("❌ Payment declined: ${authResponse.error?.message ?: "Unknown error"}")
            "failed"
        }
        payments.PaymentStatus.PENDING -> {
            println("⏳ Payment pending - awaiting async confirmation")
            println("   (In production: wait for webhook, then poll get())")
            "pending"
        }
        payments.PaymentStatus.AUTHORIZED -> {
            println("✅ Funds reserved successfully")
            
            // Step 2: Capture
            println("\n[2/2] Capturing...")
            val captureRequest = buildCaptureRequest(
                authResponse.connectorTransactionId,
                authResponse.amount
            )
            
            val captureResponse = client.capture(captureRequest)
            println("Capture Status: ${captureResponse.status}")
            
            when (captureResponse.status) {
                payments.PaymentStatus.CAPTURED, payments.PaymentStatus.AUTHORIZED -> {
                    println("✅ Payment captured successfully")
                    "success"
                }
                else -> {
                    println("⚠️  Capture returned: ${captureResponse.status}")
                    "partial"
                }
            }
        }
        else -> {
            println("⚠️  Unexpected status: ${authResponse.status}")
            "error"
        }
    }
}
```

</details>


<details>
<summary><b>BlueSnap</b></summary>

**Configuration:**
```kotlin
// Configuration not available
```

**Full Example:**
```kotlin
suspend fun processCheckoutCard(
    connectorName: String,
    credentials: Map<String, String>
): String {
    println("\n${"=".repeat(60)}")
    println("Processing Card Payment via $connectorName")
    println("${"=".repeat(60)}")
    
    // Load probe data for this connector
    val probeData = loadProbeData(connectorName)
    
    // Build config
    val config = getConnectorConfig(connectorName, credentials)
    val client = PaymentClient(config)
    
    // Build request from probe data
    val authRequest = buildAuthorizeRequest(probeData, CaptureMethod.MANUAL)
    
    println("\nRequest: ${Gson().toJson(authRequest)}")
    
    // Step 1: Authorize
    println("\n[1/2] Authorizing...")
    val authResponse = client.authorize(authRequest)
    
    println("Status: ${authResponse.status}")
    println("Connector Transaction ID: ${authResponse.connectorTransactionId}")
    
    // Handle status
    return when (authResponse.status) {
        payments.PaymentStatus.FAILURE -> {
            println("❌ Payment declined: ${authResponse.error?.message ?: "Unknown error"}")
            "failed"
        }
        payments.PaymentStatus.PENDING -> {
            println("⏳ Payment pending - awaiting async confirmation")
            println("   (In production: wait for webhook, then poll get())")
            "pending"
        }
        payments.PaymentStatus.AUTHORIZED -> {
            println("✅ Funds reserved successfully")
            
            // Step 2: Capture
            println("\n[2/2] Capturing...")
            val captureRequest = buildCaptureRequest(
                authResponse.connectorTransactionId,
                authResponse.amount
            )
            
            val captureResponse = client.capture(captureRequest)
            println("Capture Status: ${captureResponse.status}")
            
            when (captureResponse.status) {
                payments.PaymentStatus.CAPTURED, payments.PaymentStatus.AUTHORIZED -> {
                    println("✅ Payment captured successfully")
                    "success"
                }
                else -> {
                    println("⚠️  Capture returned: ${captureResponse.status}")
                    "partial"
                }
            }
        }
        else -> {
            println("⚠️  Unexpected status: ${authResponse.status}")
            "error"
        }
    }
}
```

</details>


<details>
<summary><b>Braintree</b></summary>

**Configuration:**
```kotlin
// Configuration not available
```

**Full Example:**
```kotlin
suspend fun processCheckoutCard(
    connectorName: String,
    credentials: Map<String, String>
): String {
    println("\n${"=".repeat(60)}")
    println("Processing Card Payment via $connectorName")
    println("${"=".repeat(60)}")
    
    // Load probe data for this connector
    val probeData = loadProbeData(connectorName)
    
    // Build config
    val config = getConnectorConfig(connectorName, credentials)
    val client = PaymentClient(config)
    
    // Build request from probe data
    val authRequest = buildAuthorizeRequest(probeData, CaptureMethod.MANUAL)
    
    println("\nRequest: ${Gson().toJson(authRequest)}")
    
    // Step 1: Authorize
    println("\n[1/2] Authorizing...")
    val authResponse = client.authorize(authRequest)
    
    println("Status: ${authResponse.status}")
    println("Connector Transaction ID: ${authResponse.connectorTransactionId}")
    
    // Handle status
    return when (authResponse.status) {
        payments.PaymentStatus.FAILURE -> {
            println("❌ Payment declined: ${authResponse.error?.message ?: "Unknown error"}")
            "failed"
        }
        payments.PaymentStatus.PENDING -> {
            println("⏳ Payment pending - awaiting async confirmation")
            println("   (In production: wait for webhook, then poll get())")
            "pending"
        }
        payments.PaymentStatus.AUTHORIZED -> {
            println("✅ Funds reserved successfully")
            
            // Step 2: Capture
            println("\n[2/2] Capturing...")
            val captureRequest = buildCaptureRequest(
                authResponse.connectorTransactionId,
                authResponse.amount
            )
            
            val captureResponse = client.capture(captureRequest)
            println("Capture Status: ${captureResponse.status}")
            
            when (captureResponse.status) {
                payments.PaymentStatus.CAPTURED, payments.PaymentStatus.AUTHORIZED -> {
                    println("✅ Payment captured successfully")
                    "success"
                }
                else -> {
                    println("⚠️  Capture returned: ${captureResponse.status}")
                    "partial"
                }
            }
        }
        else -> {
            println("⚠️  Unexpected status: ${authResponse.status}")
            "error"
        }
    }
}
```

</details>


<details>
<summary><b>Celero</b></summary>

**Configuration:**
```kotlin
// Configuration not available
```

**Full Example:**
```kotlin
suspend fun processCheckoutCard(
    connectorName: String,
    credentials: Map<String, String>
): String {
    println("\n${"=".repeat(60)}")
    println("Processing Card Payment via $connectorName")
    println("${"=".repeat(60)}")
    
    // Load probe data for this connector
    val probeData = loadProbeData(connectorName)
    
    // Build config
    val config = getConnectorConfig(connectorName, credentials)
    val client = PaymentClient(config)
    
    // Build request from probe data
    val authRequest = buildAuthorizeRequest(probeData, CaptureMethod.MANUAL)
    
    println("\nRequest: ${Gson().toJson(authRequest)}")
    
    // Step 1: Authorize
    println("\n[1/2] Authorizing...")
    val authResponse = client.authorize(authRequest)
    
    println("Status: ${authResponse.status}")
    println("Connector Transaction ID: ${authResponse.connectorTransactionId}")
    
    // Handle status
    return when (authResponse.status) {
        payments.PaymentStatus.FAILURE -> {
            println("❌ Payment declined: ${authResponse.error?.message ?: "Unknown error"}")
            "failed"
        }
        payments.PaymentStatus.PENDING -> {
            println("⏳ Payment pending - awaiting async confirmation")
            println("   (In production: wait for webhook, then poll get())")
            "pending"
        }
        payments.PaymentStatus.AUTHORIZED -> {
            println("✅ Funds reserved successfully")
            
            // Step 2: Capture
            println("\n[2/2] Capturing...")
            val captureRequest = buildCaptureRequest(
                authResponse.connectorTransactionId,
                authResponse.amount
            )
            
            val captureResponse = client.capture(captureRequest)
            println("Capture Status: ${captureResponse.status}")
            
            when (captureResponse.status) {
                payments.PaymentStatus.CAPTURED, payments.PaymentStatus.AUTHORIZED -> {
                    println("✅ Payment captured successfully")
                    "success"
                }
                else -> {
                    println("⚠️  Capture returned: ${captureResponse.status}")
                    "partial"
                }
            }
        }
        else -> {
            println("⚠️  Unexpected status: ${authResponse.status}")
            "error"
        }
    }
}
```

</details>


<details>
<summary><b>Checkout.com</b></summary>

**Configuration:**
```kotlin
fun getCheckoutConfig(apiKey: String): ConnectorConfig {
    return ConnectorConfig.newBuilder().apply {
        options = SdkOptions.newBuilder().apply {
            environment = Environment.SANDBOX
        }.build()
        connectorConfig = ConnectorSpecificConfig.newBuilder().apply {
            checkoutBuilder.apply {
                this.apiKey = apiKey
            }
        }.build()
    }.build()
}
```

**Full Example:**
```kotlin
suspend fun processCheckoutCard(
    connectorName: String,
    credentials: Map<String, String>
): String {
    println("\n${"=".repeat(60)}")
    println("Processing Card Payment via $connectorName")
    println("${"=".repeat(60)}")
    
    // Load probe data for this connector
    val probeData = loadProbeData(connectorName)
    
    // Build config
    val config = getConnectorConfig(connectorName, credentials)
    val client = PaymentClient(config)
    
    // Build request from probe data
    val authRequest = buildAuthorizeRequest(probeData, CaptureMethod.MANUAL)
    
    println("\nRequest: ${Gson().toJson(authRequest)}")
    
    // Step 1: Authorize
    println("\n[1/2] Authorizing...")
    val authResponse = client.authorize(authRequest)
    
    println("Status: ${authResponse.status}")
    println("Connector Transaction ID: ${authResponse.connectorTransactionId}")
    
    // Handle status
    return when (authResponse.status) {
        payments.PaymentStatus.FAILURE -> {
            println("❌ Payment declined: ${authResponse.error?.message ?: "Unknown error"}")
            "failed"
        }
        payments.PaymentStatus.PENDING -> {
            println("⏳ Payment pending - awaiting async confirmation")
            println("   (In production: wait for webhook, then poll get())")
            "pending"
        }
        payments.PaymentStatus.AUTHORIZED -> {
            println("✅ Funds reserved successfully")
            
            // Step 2: Capture
            println("\n[2/2] Capturing...")
            val captureRequest = buildCaptureRequest(
                authResponse.connectorTransactionId,
                authResponse.amount
            )
            
            val captureResponse = client.capture(captureRequest)
            println("Capture Status: ${captureResponse.status}")
            
            when (captureResponse.status) {
                payments.PaymentStatus.CAPTURED, payments.PaymentStatus.AUTHORIZED -> {
                    println("✅ Payment captured successfully")
                    "success"
                }
                else -> {
                    println("⚠️  Capture returned: ${captureResponse.status}")
                    "partial"
                }
            }
        }
        else -> {
            println("⚠️  Unexpected status: ${authResponse.status}")
            "error"
        }
    }
}
```

</details>


<details>
<summary><b>CyberSource</b></summary>

**Configuration:**
```kotlin
// Configuration not available
```

**Full Example:**
```kotlin
suspend fun processCheckoutCard(
    connectorName: String,
    credentials: Map<String, String>
): String {
    println("\n${"=".repeat(60)}")
    println("Processing Card Payment via $connectorName")
    println("${"=".repeat(60)}")
    
    // Load probe data for this connector
    val probeData = loadProbeData(connectorName)
    
    // Build config
    val config = getConnectorConfig(connectorName, credentials)
    val client = PaymentClient(config)
    
    // Build request from probe data
    val authRequest = buildAuthorizeRequest(probeData, CaptureMethod.MANUAL)
    
    println("\nRequest: ${Gson().toJson(authRequest)}")
    
    // Step 1: Authorize
    println("\n[1/2] Authorizing...")
    val authResponse = client.authorize(authRequest)
    
    println("Status: ${authResponse.status}")
    println("Connector Transaction ID: ${authResponse.connectorTransactionId}")
    
    // Handle status
    return when (authResponse.status) {
        payments.PaymentStatus.FAILURE -> {
            println("❌ Payment declined: ${authResponse.error?.message ?: "Unknown error"}")
            "failed"
        }
        payments.PaymentStatus.PENDING -> {
            println("⏳ Payment pending - awaiting async confirmation")
            println("   (In production: wait for webhook, then poll get())")
            "pending"
        }
        payments.PaymentStatus.AUTHORIZED -> {
            println("✅ Funds reserved successfully")
            
            // Step 2: Capture
            println("\n[2/2] Capturing...")
            val captureRequest = buildCaptureRequest(
                authResponse.connectorTransactionId,
                authResponse.amount
            )
            
            val captureResponse = client.capture(captureRequest)
            println("Capture Status: ${captureResponse.status}")
            
            when (captureResponse.status) {
                payments.PaymentStatus.CAPTURED, payments.PaymentStatus.AUTHORIZED -> {
                    println("✅ Payment captured successfully")
                    "success"
                }
                else -> {
                    println("⚠️  Capture returned: ${captureResponse.status}")
                    "partial"
                }
            }
        }
        else -> {
            println("⚠️  Unexpected status: ${authResponse.status}")
            "error"
        }
    }
}
```

</details>


<details>
<summary><b>Datatrans</b></summary>

**Configuration:**
```kotlin
// Configuration not available
```

**Full Example:**
```kotlin
suspend fun processCheckoutCard(
    connectorName: String,
    credentials: Map<String, String>
): String {
    println("\n${"=".repeat(60)}")
    println("Processing Card Payment via $connectorName")
    println("${"=".repeat(60)}")
    
    // Load probe data for this connector
    val probeData = loadProbeData(connectorName)
    
    // Build config
    val config = getConnectorConfig(connectorName, credentials)
    val client = PaymentClient(config)
    
    // Build request from probe data
    val authRequest = buildAuthorizeRequest(probeData, CaptureMethod.MANUAL)
    
    println("\nRequest: ${Gson().toJson(authRequest)}")
    
    // Step 1: Authorize
    println("\n[1/2] Authorizing...")
    val authResponse = client.authorize(authRequest)
    
    println("Status: ${authResponse.status}")
    println("Connector Transaction ID: ${authResponse.connectorTransactionId}")
    
    // Handle status
    return when (authResponse.status) {
        payments.PaymentStatus.FAILURE -> {
            println("❌ Payment declined: ${authResponse.error?.message ?: "Unknown error"}")
            "failed"
        }
        payments.PaymentStatus.PENDING -> {
            println("⏳ Payment pending - awaiting async confirmation")
            println("   (In production: wait for webhook, then poll get())")
            "pending"
        }
        payments.PaymentStatus.AUTHORIZED -> {
            println("✅ Funds reserved successfully")
            
            // Step 2: Capture
            println("\n[2/2] Capturing...")
            val captureRequest = buildCaptureRequest(
                authResponse.connectorTransactionId,
                authResponse.amount
            )
            
            val captureResponse = client.capture(captureRequest)
            println("Capture Status: ${captureResponse.status}")
            
            when (captureResponse.status) {
                payments.PaymentStatus.CAPTURED, payments.PaymentStatus.AUTHORIZED -> {
                    println("✅ Payment captured successfully")
                    "success"
                }
                else -> {
                    println("⚠️  Capture returned: ${captureResponse.status}")
                    "partial"
                }
            }
        }
        else -> {
            println("⚠️  Unexpected status: ${authResponse.status}")
            "error"
        }
    }
}
```

</details>


<details>
<summary><b>dLocal</b></summary>

**Configuration:**
```kotlin
// Configuration not available
```

**Full Example:**
```kotlin
suspend fun processCheckoutCard(
    connectorName: String,
    credentials: Map<String, String>
): String {
    println("\n${"=".repeat(60)}")
    println("Processing Card Payment via $connectorName")
    println("${"=".repeat(60)}")
    
    // Load probe data for this connector
    val probeData = loadProbeData(connectorName)
    
    // Build config
    val config = getConnectorConfig(connectorName, credentials)
    val client = PaymentClient(config)
    
    // Build request from probe data
    val authRequest = buildAuthorizeRequest(probeData, CaptureMethod.MANUAL)
    
    println("\nRequest: ${Gson().toJson(authRequest)}")
    
    // Step 1: Authorize
    println("\n[1/2] Authorizing...")
    val authResponse = client.authorize(authRequest)
    
    println("Status: ${authResponse.status}")
    println("Connector Transaction ID: ${authResponse.connectorTransactionId}")
    
    // Handle status
    return when (authResponse.status) {
        payments.PaymentStatus.FAILURE -> {
            println("❌ Payment declined: ${authResponse.error?.message ?: "Unknown error"}")
            "failed"
        }
        payments.PaymentStatus.PENDING -> {
            println("⏳ Payment pending - awaiting async confirmation")
            println("   (In production: wait for webhook, then poll get())")
            "pending"
        }
        payments.PaymentStatus.AUTHORIZED -> {
            println("✅ Funds reserved successfully")
            
            // Step 2: Capture
            println("\n[2/2] Capturing...")
            val captureRequest = buildCaptureRequest(
                authResponse.connectorTransactionId,
                authResponse.amount
            )
            
            val captureResponse = client.capture(captureRequest)
            println("Capture Status: ${captureResponse.status}")
            
            when (captureResponse.status) {
                payments.PaymentStatus.CAPTURED, payments.PaymentStatus.AUTHORIZED -> {
                    println("✅ Payment captured successfully")
                    "success"
                }
                else -> {
                    println("⚠️  Capture returned: ${captureResponse.status}")
                    "partial"
                }
            }
        }
        else -> {
            println("⚠️  Unexpected status: ${authResponse.status}")
            "error"
        }
    }
}
```

</details>


<details>
<summary><b>Elavon</b></summary>

**Configuration:**
```kotlin
// Configuration not available
```

**Full Example:**
```kotlin
suspend fun processCheckoutCard(
    connectorName: String,
    credentials: Map<String, String>
): String {
    println("\n${"=".repeat(60)}")
    println("Processing Card Payment via $connectorName")
    println("${"=".repeat(60)}")
    
    // Load probe data for this connector
    val probeData = loadProbeData(connectorName)
    
    // Build config
    val config = getConnectorConfig(connectorName, credentials)
    val client = PaymentClient(config)
    
    // Build request from probe data
    val authRequest = buildAuthorizeRequest(probeData, CaptureMethod.MANUAL)
    
    println("\nRequest: ${Gson().toJson(authRequest)}")
    
    // Step 1: Authorize
    println("\n[1/2] Authorizing...")
    val authResponse = client.authorize(authRequest)
    
    println("Status: ${authResponse.status}")
    println("Connector Transaction ID: ${authResponse.connectorTransactionId}")
    
    // Handle status
    return when (authResponse.status) {
        payments.PaymentStatus.FAILURE -> {
            println("❌ Payment declined: ${authResponse.error?.message ?: "Unknown error"}")
            "failed"
        }
        payments.PaymentStatus.PENDING -> {
            println("⏳ Payment pending - awaiting async confirmation")
            println("   (In production: wait for webhook, then poll get())")
            "pending"
        }
        payments.PaymentStatus.AUTHORIZED -> {
            println("✅ Funds reserved successfully")
            
            // Step 2: Capture
            println("\n[2/2] Capturing...")
            val captureRequest = buildCaptureRequest(
                authResponse.connectorTransactionId,
                authResponse.amount
            )
            
            val captureResponse = client.capture(captureRequest)
            println("Capture Status: ${captureResponse.status}")
            
            when (captureResponse.status) {
                payments.PaymentStatus.CAPTURED, payments.PaymentStatus.AUTHORIZED -> {
                    println("✅ Payment captured successfully")
                    "success"
                }
                else -> {
                    println("⚠️  Capture returned: ${captureResponse.status}")
                    "partial"
                }
            }
        }
        else -> {
            println("⚠️  Unexpected status: ${authResponse.status}")
            "error"
        }
    }
}
```

</details>


<details>
<summary><b>Finix</b></summary>

**Configuration:**
```kotlin
// Configuration not available
```

**Full Example:**
```kotlin
suspend fun processCheckoutCard(
    connectorName: String,
    credentials: Map<String, String>
): String {
    println("\n${"=".repeat(60)}")
    println("Processing Card Payment via $connectorName")
    println("${"=".repeat(60)}")
    
    // Load probe data for this connector
    val probeData = loadProbeData(connectorName)
    
    // Build config
    val config = getConnectorConfig(connectorName, credentials)
    val client = PaymentClient(config)
    
    // Build request from probe data
    val authRequest = buildAuthorizeRequest(probeData, CaptureMethod.MANUAL)
    
    println("\nRequest: ${Gson().toJson(authRequest)}")
    
    // Step 1: Authorize
    println("\n[1/2] Authorizing...")
    val authResponse = client.authorize(authRequest)
    
    println("Status: ${authResponse.status}")
    println("Connector Transaction ID: ${authResponse.connectorTransactionId}")
    
    // Handle status
    return when (authResponse.status) {
        payments.PaymentStatus.FAILURE -> {
            println("❌ Payment declined: ${authResponse.error?.message ?: "Unknown error"}")
            "failed"
        }
        payments.PaymentStatus.PENDING -> {
            println("⏳ Payment pending - awaiting async confirmation")
            println("   (In production: wait for webhook, then poll get())")
            "pending"
        }
        payments.PaymentStatus.AUTHORIZED -> {
            println("✅ Funds reserved successfully")
            
            // Step 2: Capture
            println("\n[2/2] Capturing...")
            val captureRequest = buildCaptureRequest(
                authResponse.connectorTransactionId,
                authResponse.amount
            )
            
            val captureResponse = client.capture(captureRequest)
            println("Capture Status: ${captureResponse.status}")
            
            when (captureResponse.status) {
                payments.PaymentStatus.CAPTURED, payments.PaymentStatus.AUTHORIZED -> {
                    println("✅ Payment captured successfully")
                    "success"
                }
                else -> {
                    println("⚠️  Capture returned: ${captureResponse.status}")
                    "partial"
                }
            }
        }
        else -> {
            println("⚠️  Unexpected status: ${authResponse.status}")
            "error"
        }
    }
}
```

</details>


<details>
<summary><b>Fiserv</b></summary>

**Configuration:**
```kotlin
// Configuration not available
```

**Full Example:**
```kotlin
suspend fun processCheckoutCard(
    connectorName: String,
    credentials: Map<String, String>
): String {
    println("\n${"=".repeat(60)}")
    println("Processing Card Payment via $connectorName")
    println("${"=".repeat(60)}")
    
    // Load probe data for this connector
    val probeData = loadProbeData(connectorName)
    
    // Build config
    val config = getConnectorConfig(connectorName, credentials)
    val client = PaymentClient(config)
    
    // Build request from probe data
    val authRequest = buildAuthorizeRequest(probeData, CaptureMethod.MANUAL)
    
    println("\nRequest: ${Gson().toJson(authRequest)}")
    
    // Step 1: Authorize
    println("\n[1/2] Authorizing...")
    val authResponse = client.authorize(authRequest)
    
    println("Status: ${authResponse.status}")
    println("Connector Transaction ID: ${authResponse.connectorTransactionId}")
    
    // Handle status
    return when (authResponse.status) {
        payments.PaymentStatus.FAILURE -> {
            println("❌ Payment declined: ${authResponse.error?.message ?: "Unknown error"}")
            "failed"
        }
        payments.PaymentStatus.PENDING -> {
            println("⏳ Payment pending - awaiting async confirmation")
            println("   (In production: wait for webhook, then poll get())")
            "pending"
        }
        payments.PaymentStatus.AUTHORIZED -> {
            println("✅ Funds reserved successfully")
            
            // Step 2: Capture
            println("\n[2/2] Capturing...")
            val captureRequest = buildCaptureRequest(
                authResponse.connectorTransactionId,
                authResponse.amount
            )
            
            val captureResponse = client.capture(captureRequest)
            println("Capture Status: ${captureResponse.status}")
            
            when (captureResponse.status) {
                payments.PaymentStatus.CAPTURED, payments.PaymentStatus.AUTHORIZED -> {
                    println("✅ Payment captured successfully")
                    "success"
                }
                else -> {
                    println("⚠️  Capture returned: ${captureResponse.status}")
                    "partial"
                }
            }
        }
        else -> {
            println("⚠️  Unexpected status: ${authResponse.status}")
            "error"
        }
    }
}
```

</details>


<details>
<summary><b>Fiserv EMEA</b></summary>

**Configuration:**
```kotlin
// Configuration not available
```

**Full Example:**
```kotlin
suspend fun processCheckoutCard(
    connectorName: String,
    credentials: Map<String, String>
): String {
    println("\n${"=".repeat(60)}")
    println("Processing Card Payment via $connectorName")
    println("${"=".repeat(60)}")
    
    // Load probe data for this connector
    val probeData = loadProbeData(connectorName)
    
    // Build config
    val config = getConnectorConfig(connectorName, credentials)
    val client = PaymentClient(config)
    
    // Build request from probe data
    val authRequest = buildAuthorizeRequest(probeData, CaptureMethod.MANUAL)
    
    println("\nRequest: ${Gson().toJson(authRequest)}")
    
    // Step 1: Authorize
    println("\n[1/2] Authorizing...")
    val authResponse = client.authorize(authRequest)
    
    println("Status: ${authResponse.status}")
    println("Connector Transaction ID: ${authResponse.connectorTransactionId}")
    
    // Handle status
    return when (authResponse.status) {
        payments.PaymentStatus.FAILURE -> {
            println("❌ Payment declined: ${authResponse.error?.message ?: "Unknown error"}")
            "failed"
        }
        payments.PaymentStatus.PENDING -> {
            println("⏳ Payment pending - awaiting async confirmation")
            println("   (In production: wait for webhook, then poll get())")
            "pending"
        }
        payments.PaymentStatus.AUTHORIZED -> {
            println("✅ Funds reserved successfully")
            
            // Step 2: Capture
            println("\n[2/2] Capturing...")
            val captureRequest = buildCaptureRequest(
                authResponse.connectorTransactionId,
                authResponse.amount
            )
            
            val captureResponse = client.capture(captureRequest)
            println("Capture Status: ${captureResponse.status}")
            
            when (captureResponse.status) {
                payments.PaymentStatus.CAPTURED, payments.PaymentStatus.AUTHORIZED -> {
                    println("✅ Payment captured successfully")
                    "success"
                }
                else -> {
                    println("⚠️  Capture returned: ${captureResponse.status}")
                    "partial"
                }
            }
        }
        else -> {
            println("⚠️  Unexpected status: ${authResponse.status}")
            "error"
        }
    }
}
```

</details>


<details>
<summary><b>Fiuu</b></summary>

**Configuration:**
```kotlin
// Configuration not available
```

**Full Example:**
```kotlin
suspend fun processCheckoutCard(
    connectorName: String,
    credentials: Map<String, String>
): String {
    println("\n${"=".repeat(60)}")
    println("Processing Card Payment via $connectorName")
    println("${"=".repeat(60)}")
    
    // Load probe data for this connector
    val probeData = loadProbeData(connectorName)
    
    // Build config
    val config = getConnectorConfig(connectorName, credentials)
    val client = PaymentClient(config)
    
    // Build request from probe data
    val authRequest = buildAuthorizeRequest(probeData, CaptureMethod.MANUAL)
    
    println("\nRequest: ${Gson().toJson(authRequest)}")
    
    // Step 1: Authorize
    println("\n[1/2] Authorizing...")
    val authResponse = client.authorize(authRequest)
    
    println("Status: ${authResponse.status}")
    println("Connector Transaction ID: ${authResponse.connectorTransactionId}")
    
    // Handle status
    return when (authResponse.status) {
        payments.PaymentStatus.FAILURE -> {
            println("❌ Payment declined: ${authResponse.error?.message ?: "Unknown error"}")
            "failed"
        }
        payments.PaymentStatus.PENDING -> {
            println("⏳ Payment pending - awaiting async confirmation")
            println("   (In production: wait for webhook, then poll get())")
            "pending"
        }
        payments.PaymentStatus.AUTHORIZED -> {
            println("✅ Funds reserved successfully")
            
            // Step 2: Capture
            println("\n[2/2] Capturing...")
            val captureRequest = buildCaptureRequest(
                authResponse.connectorTransactionId,
                authResponse.amount
            )
            
            val captureResponse = client.capture(captureRequest)
            println("Capture Status: ${captureResponse.status}")
            
            when (captureResponse.status) {
                payments.PaymentStatus.CAPTURED, payments.PaymentStatus.AUTHORIZED -> {
                    println("✅ Payment captured successfully")
                    "success"
                }
                else -> {
                    println("⚠️  Capture returned: ${captureResponse.status}")
                    "partial"
                }
            }
        }
        else -> {
            println("⚠️  Unexpected status: ${authResponse.status}")
            "error"
        }
    }
}
```

</details>


<details>
<summary><b>Getnet</b></summary>

**Configuration:**
```kotlin
// Configuration not available
```

**Full Example:**
```kotlin
suspend fun processCheckoutCard(
    connectorName: String,
    credentials: Map<String, String>
): String {
    println("\n${"=".repeat(60)}")
    println("Processing Card Payment via $connectorName")
    println("${"=".repeat(60)}")
    
    // Load probe data for this connector
    val probeData = loadProbeData(connectorName)
    
    // Build config
    val config = getConnectorConfig(connectorName, credentials)
    val client = PaymentClient(config)
    
    // Build request from probe data
    val authRequest = buildAuthorizeRequest(probeData, CaptureMethod.MANUAL)
    
    println("\nRequest: ${Gson().toJson(authRequest)}")
    
    // Step 1: Authorize
    println("\n[1/2] Authorizing...")
    val authResponse = client.authorize(authRequest)
    
    println("Status: ${authResponse.status}")
    println("Connector Transaction ID: ${authResponse.connectorTransactionId}")
    
    // Handle status
    return when (authResponse.status) {
        payments.PaymentStatus.FAILURE -> {
            println("❌ Payment declined: ${authResponse.error?.message ?: "Unknown error"}")
            "failed"
        }
        payments.PaymentStatus.PENDING -> {
            println("⏳ Payment pending - awaiting async confirmation")
            println("   (In production: wait for webhook, then poll get())")
            "pending"
        }
        payments.PaymentStatus.AUTHORIZED -> {
            println("✅ Funds reserved successfully")
            
            // Step 2: Capture
            println("\n[2/2] Capturing...")
            val captureRequest = buildCaptureRequest(
                authResponse.connectorTransactionId,
                authResponse.amount
            )
            
            val captureResponse = client.capture(captureRequest)
            println("Capture Status: ${captureResponse.status}")
            
            when (captureResponse.status) {
                payments.PaymentStatus.CAPTURED, payments.PaymentStatus.AUTHORIZED -> {
                    println("✅ Payment captured successfully")
                    "success"
                }
                else -> {
                    println("⚠️  Capture returned: ${captureResponse.status}")
                    "partial"
                }
            }
        }
        else -> {
            println("⚠️  Unexpected status: ${authResponse.status}")
            "error"
        }
    }
}
```

</details>


<details>
<summary><b>Global Payments</b></summary>

**Configuration:**
```kotlin
// Configuration not available
```

**Full Example:**
```kotlin
suspend fun processCheckoutCard(
    connectorName: String,
    credentials: Map<String, String>
): String {
    println("\n${"=".repeat(60)}")
    println("Processing Card Payment via $connectorName")
    println("${"=".repeat(60)}")
    
    // Load probe data for this connector
    val probeData = loadProbeData(connectorName)
    
    // Build config
    val config = getConnectorConfig(connectorName, credentials)
    val client = PaymentClient(config)
    
    // Build request from probe data
    val authRequest = buildAuthorizeRequest(probeData, CaptureMethod.MANUAL)
    
    println("\nRequest: ${Gson().toJson(authRequest)}")
    
    // Step 1: Authorize
    println("\n[1/2] Authorizing...")
    val authResponse = client.authorize(authRequest)
    
    println("Status: ${authResponse.status}")
    println("Connector Transaction ID: ${authResponse.connectorTransactionId}")
    
    // Handle status
    return when (authResponse.status) {
        payments.PaymentStatus.FAILURE -> {
            println("❌ Payment declined: ${authResponse.error?.message ?: "Unknown error"}")
            "failed"
        }
        payments.PaymentStatus.PENDING -> {
            println("⏳ Payment pending - awaiting async confirmation")
            println("   (In production: wait for webhook, then poll get())")
            "pending"
        }
        payments.PaymentStatus.AUTHORIZED -> {
            println("✅ Funds reserved successfully")
            
            // Step 2: Capture
            println("\n[2/2] Capturing...")
            val captureRequest = buildCaptureRequest(
                authResponse.connectorTransactionId,
                authResponse.amount
            )
            
            val captureResponse = client.capture(captureRequest)
            println("Capture Status: ${captureResponse.status}")
            
            when (captureResponse.status) {
                payments.PaymentStatus.CAPTURED, payments.PaymentStatus.AUTHORIZED -> {
                    println("✅ Payment captured successfully")
                    "success"
                }
                else -> {
                    println("⚠️  Capture returned: ${captureResponse.status}")
                    "partial"
                }
            }
        }
        else -> {
            println("⚠️  Unexpected status: ${authResponse.status}")
            "error"
        }
    }
}
```

</details>


<details>
<summary><b>HiPay</b></summary>

**Configuration:**
```kotlin
// Configuration not available
```

**Full Example:**
```kotlin
suspend fun processCheckoutCard(
    connectorName: String,
    credentials: Map<String, String>
): String {
    println("\n${"=".repeat(60)}")
    println("Processing Card Payment via $connectorName")
    println("${"=".repeat(60)}")
    
    // Load probe data for this connector
    val probeData = loadProbeData(connectorName)
    
    // Build config
    val config = getConnectorConfig(connectorName, credentials)
    val client = PaymentClient(config)
    
    // Build request from probe data
    val authRequest = buildAuthorizeRequest(probeData, CaptureMethod.MANUAL)
    
    println("\nRequest: ${Gson().toJson(authRequest)}")
    
    // Step 1: Authorize
    println("\n[1/2] Authorizing...")
    val authResponse = client.authorize(authRequest)
    
    println("Status: ${authResponse.status}")
    println("Connector Transaction ID: ${authResponse.connectorTransactionId}")
    
    // Handle status
    return when (authResponse.status) {
        payments.PaymentStatus.FAILURE -> {
            println("❌ Payment declined: ${authResponse.error?.message ?: "Unknown error"}")
            "failed"
        }
        payments.PaymentStatus.PENDING -> {
            println("⏳ Payment pending - awaiting async confirmation")
            println("   (In production: wait for webhook, then poll get())")
            "pending"
        }
        payments.PaymentStatus.AUTHORIZED -> {
            println("✅ Funds reserved successfully")
            
            // Step 2: Capture
            println("\n[2/2] Capturing...")
            val captureRequest = buildCaptureRequest(
                authResponse.connectorTransactionId,
                authResponse.amount
            )
            
            val captureResponse = client.capture(captureRequest)
            println("Capture Status: ${captureResponse.status}")
            
            when (captureResponse.status) {
                payments.PaymentStatus.CAPTURED, payments.PaymentStatus.AUTHORIZED -> {
                    println("✅ Payment captured successfully")
                    "success"
                }
                else -> {
                    println("⚠️  Capture returned: ${captureResponse.status}")
                    "partial"
                }
            }
        }
        else -> {
            println("⚠️  Unexpected status: ${authResponse.status}")
            "error"
        }
    }
}
```

</details>


<details>
<summary><b>J.P. Morgan</b></summary>

**Configuration:**
```kotlin
// Configuration not available
```

**Full Example:**
```kotlin
suspend fun processCheckoutCard(
    connectorName: String,
    credentials: Map<String, String>
): String {
    println("\n${"=".repeat(60)}")
    println("Processing Card Payment via $connectorName")
    println("${"=".repeat(60)}")
    
    // Load probe data for this connector
    val probeData = loadProbeData(connectorName)
    
    // Build config
    val config = getConnectorConfig(connectorName, credentials)
    val client = PaymentClient(config)
    
    // Build request from probe data
    val authRequest = buildAuthorizeRequest(probeData, CaptureMethod.MANUAL)
    
    println("\nRequest: ${Gson().toJson(authRequest)}")
    
    // Step 1: Authorize
    println("\n[1/2] Authorizing...")
    val authResponse = client.authorize(authRequest)
    
    println("Status: ${authResponse.status}")
    println("Connector Transaction ID: ${authResponse.connectorTransactionId}")
    
    // Handle status
    return when (authResponse.status) {
        payments.PaymentStatus.FAILURE -> {
            println("❌ Payment declined: ${authResponse.error?.message ?: "Unknown error"}")
            "failed"
        }
        payments.PaymentStatus.PENDING -> {
            println("⏳ Payment pending - awaiting async confirmation")
            println("   (In production: wait for webhook, then poll get())")
            "pending"
        }
        payments.PaymentStatus.AUTHORIZED -> {
            println("✅ Funds reserved successfully")
            
            // Step 2: Capture
            println("\n[2/2] Capturing...")
            val captureRequest = buildCaptureRequest(
                authResponse.connectorTransactionId,
                authResponse.amount
            )
            
            val captureResponse = client.capture(captureRequest)
            println("Capture Status: ${captureResponse.status}")
            
            when (captureResponse.status) {
                payments.PaymentStatus.CAPTURED, payments.PaymentStatus.AUTHORIZED -> {
                    println("✅ Payment captured successfully")
                    "success"
                }
                else -> {
                    println("⚠️  Capture returned: ${captureResponse.status}")
                    "partial"
                }
            }
        }
        else -> {
            println("⚠️  Unexpected status: ${authResponse.status}")
            "error"
        }
    }
}
```

</details>


<details>
<summary><b>NMI</b></summary>

**Configuration:**
```kotlin
// Configuration not available
```

**Full Example:**
```kotlin
suspend fun processCheckoutCard(
    connectorName: String,
    credentials: Map<String, String>
): String {
    println("\n${"=".repeat(60)}")
    println("Processing Card Payment via $connectorName")
    println("${"=".repeat(60)}")
    
    // Load probe data for this connector
    val probeData = loadProbeData(connectorName)
    
    // Build config
    val config = getConnectorConfig(connectorName, credentials)
    val client = PaymentClient(config)
    
    // Build request from probe data
    val authRequest = buildAuthorizeRequest(probeData, CaptureMethod.MANUAL)
    
    println("\nRequest: ${Gson().toJson(authRequest)}")
    
    // Step 1: Authorize
    println("\n[1/2] Authorizing...")
    val authResponse = client.authorize(authRequest)
    
    println("Status: ${authResponse.status}")
    println("Connector Transaction ID: ${authResponse.connectorTransactionId}")
    
    // Handle status
    return when (authResponse.status) {
        payments.PaymentStatus.FAILURE -> {
            println("❌ Payment declined: ${authResponse.error?.message ?: "Unknown error"}")
            "failed"
        }
        payments.PaymentStatus.PENDING -> {
            println("⏳ Payment pending - awaiting async confirmation")
            println("   (In production: wait for webhook, then poll get())")
            "pending"
        }
        payments.PaymentStatus.AUTHORIZED -> {
            println("✅ Funds reserved successfully")
            
            // Step 2: Capture
            println("\n[2/2] Capturing...")
            val captureRequest = buildCaptureRequest(
                authResponse.connectorTransactionId,
                authResponse.amount
            )
            
            val captureResponse = client.capture(captureRequest)
            println("Capture Status: ${captureResponse.status}")
            
            when (captureResponse.status) {
                payments.PaymentStatus.CAPTURED, payments.PaymentStatus.AUTHORIZED -> {
                    println("✅ Payment captured successfully")
                    "success"
                }
                else -> {
                    println("⚠️  Capture returned: ${captureResponse.status}")
                    "partial"
                }
            }
        }
        else -> {
            println("⚠️  Unexpected status: ${authResponse.status}")
            "error"
        }
    }
}
```

</details>


<details>
<summary><b>Noon</b></summary>

**Configuration:**
```kotlin
// Configuration not available
```

**Full Example:**
```kotlin
suspend fun processCheckoutCard(
    connectorName: String,
    credentials: Map<String, String>
): String {
    println("\n${"=".repeat(60)}")
    println("Processing Card Payment via $connectorName")
    println("${"=".repeat(60)}")
    
    // Load probe data for this connector
    val probeData = loadProbeData(connectorName)
    
    // Build config
    val config = getConnectorConfig(connectorName, credentials)
    val client = PaymentClient(config)
    
    // Build request from probe data
    val authRequest = buildAuthorizeRequest(probeData, CaptureMethod.MANUAL)
    
    println("\nRequest: ${Gson().toJson(authRequest)}")
    
    // Step 1: Authorize
    println("\n[1/2] Authorizing...")
    val authResponse = client.authorize(authRequest)
    
    println("Status: ${authResponse.status}")
    println("Connector Transaction ID: ${authResponse.connectorTransactionId}")
    
    // Handle status
    return when (authResponse.status) {
        payments.PaymentStatus.FAILURE -> {
            println("❌ Payment declined: ${authResponse.error?.message ?: "Unknown error"}")
            "failed"
        }
        payments.PaymentStatus.PENDING -> {
            println("⏳ Payment pending - awaiting async confirmation")
            println("   (In production: wait for webhook, then poll get())")
            "pending"
        }
        payments.PaymentStatus.AUTHORIZED -> {
            println("✅ Funds reserved successfully")
            
            // Step 2: Capture
            println("\n[2/2] Capturing...")
            val captureRequest = buildCaptureRequest(
                authResponse.connectorTransactionId,
                authResponse.amount
            )
            
            val captureResponse = client.capture(captureRequest)
            println("Capture Status: ${captureResponse.status}")
            
            when (captureResponse.status) {
                payments.PaymentStatus.CAPTURED, payments.PaymentStatus.AUTHORIZED -> {
                    println("✅ Payment captured successfully")
                    "success"
                }
                else -> {
                    println("⚠️  Capture returned: ${captureResponse.status}")
                    "partial"
                }
            }
        }
        else -> {
            println("⚠️  Unexpected status: ${authResponse.status}")
            "error"
        }
    }
}
```

</details>


<details>
<summary><b>Novalnet</b></summary>

**Configuration:**
```kotlin
// Configuration not available
```

**Full Example:**
```kotlin
suspend fun processCheckoutCard(
    connectorName: String,
    credentials: Map<String, String>
): String {
    println("\n${"=".repeat(60)}")
    println("Processing Card Payment via $connectorName")
    println("${"=".repeat(60)}")
    
    // Load probe data for this connector
    val probeData = loadProbeData(connectorName)
    
    // Build config
    val config = getConnectorConfig(connectorName, credentials)
    val client = PaymentClient(config)
    
    // Build request from probe data
    val authRequest = buildAuthorizeRequest(probeData, CaptureMethod.MANUAL)
    
    println("\nRequest: ${Gson().toJson(authRequest)}")
    
    // Step 1: Authorize
    println("\n[1/2] Authorizing...")
    val authResponse = client.authorize(authRequest)
    
    println("Status: ${authResponse.status}")
    println("Connector Transaction ID: ${authResponse.connectorTransactionId}")
    
    // Handle status
    return when (authResponse.status) {
        payments.PaymentStatus.FAILURE -> {
            println("❌ Payment declined: ${authResponse.error?.message ?: "Unknown error"}")
            "failed"
        }
        payments.PaymentStatus.PENDING -> {
            println("⏳ Payment pending - awaiting async confirmation")
            println("   (In production: wait for webhook, then poll get())")
            "pending"
        }
        payments.PaymentStatus.AUTHORIZED -> {
            println("✅ Funds reserved successfully")
            
            // Step 2: Capture
            println("\n[2/2] Capturing...")
            val captureRequest = buildCaptureRequest(
                authResponse.connectorTransactionId,
                authResponse.amount
            )
            
            val captureResponse = client.capture(captureRequest)
            println("Capture Status: ${captureResponse.status}")
            
            when (captureResponse.status) {
                payments.PaymentStatus.CAPTURED, payments.PaymentStatus.AUTHORIZED -> {
                    println("✅ Payment captured successfully")
                    "success"
                }
                else -> {
                    println("⚠️  Capture returned: ${captureResponse.status}")
                    "partial"
                }
            }
        }
        else -> {
            println("⚠️  Unexpected status: ${authResponse.status}")
            "error"
        }
    }
}
```

</details>


<details>
<summary><b>Nuvei</b></summary>

**Configuration:**
```kotlin
// Configuration not available
```

**Full Example:**
```kotlin
suspend fun processCheckoutCard(
    connectorName: String,
    credentials: Map<String, String>
): String {
    println("\n${"=".repeat(60)}")
    println("Processing Card Payment via $connectorName")
    println("${"=".repeat(60)}")
    
    // Load probe data for this connector
    val probeData = loadProbeData(connectorName)
    
    // Build config
    val config = getConnectorConfig(connectorName, credentials)
    val client = PaymentClient(config)
    
    // Build request from probe data
    val authRequest = buildAuthorizeRequest(probeData, CaptureMethod.MANUAL)
    
    println("\nRequest: ${Gson().toJson(authRequest)}")
    
    // Step 1: Authorize
    println("\n[1/2] Authorizing...")
    val authResponse = client.authorize(authRequest)
    
    println("Status: ${authResponse.status}")
    println("Connector Transaction ID: ${authResponse.connectorTransactionId}")
    
    // Handle status
    return when (authResponse.status) {
        payments.PaymentStatus.FAILURE -> {
            println("❌ Payment declined: ${authResponse.error?.message ?: "Unknown error"}")
            "failed"
        }
        payments.PaymentStatus.PENDING -> {
            println("⏳ Payment pending - awaiting async confirmation")
            println("   (In production: wait for webhook, then poll get())")
            "pending"
        }
        payments.PaymentStatus.AUTHORIZED -> {
            println("✅ Funds reserved successfully")
            
            // Step 2: Capture
            println("\n[2/2] Capturing...")
            val captureRequest = buildCaptureRequest(
                authResponse.connectorTransactionId,
                authResponse.amount
            )
            
            val captureResponse = client.capture(captureRequest)
            println("Capture Status: ${captureResponse.status}")
            
            when (captureResponse.status) {
                payments.PaymentStatus.CAPTURED, payments.PaymentStatus.AUTHORIZED -> {
                    println("✅ Payment captured successfully")
                    "success"
                }
                else -> {
                    println("⚠️  Capture returned: ${captureResponse.status}")
                    "partial"
                }
            }
        }
        else -> {
            println("⚠️  Unexpected status: ${authResponse.status}")
            "error"
        }
    }
}
```

</details>


<details>
<summary><b>Paybox</b></summary>

**Configuration:**
```kotlin
// Configuration not available
```

**Full Example:**
```kotlin
suspend fun processCheckoutCard(
    connectorName: String,
    credentials: Map<String, String>
): String {
    println("\n${"=".repeat(60)}")
    println("Processing Card Payment via $connectorName")
    println("${"=".repeat(60)}")
    
    // Load probe data for this connector
    val probeData = loadProbeData(connectorName)
    
    // Build config
    val config = getConnectorConfig(connectorName, credentials)
    val client = PaymentClient(config)
    
    // Build request from probe data
    val authRequest = buildAuthorizeRequest(probeData, CaptureMethod.MANUAL)
    
    println("\nRequest: ${Gson().toJson(authRequest)}")
    
    // Step 1: Authorize
    println("\n[1/2] Authorizing...")
    val authResponse = client.authorize(authRequest)
    
    println("Status: ${authResponse.status}")
    println("Connector Transaction ID: ${authResponse.connectorTransactionId}")
    
    // Handle status
    return when (authResponse.status) {
        payments.PaymentStatus.FAILURE -> {
            println("❌ Payment declined: ${authResponse.error?.message ?: "Unknown error"}")
            "failed"
        }
        payments.PaymentStatus.PENDING -> {
            println("⏳ Payment pending - awaiting async confirmation")
            println("   (In production: wait for webhook, then poll get())")
            "pending"
        }
        payments.PaymentStatus.AUTHORIZED -> {
            println("✅ Funds reserved successfully")
            
            // Step 2: Capture
            println("\n[2/2] Capturing...")
            val captureRequest = buildCaptureRequest(
                authResponse.connectorTransactionId,
                authResponse.amount
            )
            
            val captureResponse = client.capture(captureRequest)
            println("Capture Status: ${captureResponse.status}")
            
            when (captureResponse.status) {
                payments.PaymentStatus.CAPTURED, payments.PaymentStatus.AUTHORIZED -> {
                    println("✅ Payment captured successfully")
                    "success"
                }
                else -> {
                    println("⚠️  Capture returned: ${captureResponse.status}")
                    "partial"
                }
            }
        }
        else -> {
            println("⚠️  Unexpected status: ${authResponse.status}")
            "error"
        }
    }
}
```

</details>


<details>
<summary><b>PayPal</b></summary>

**Configuration:**
```kotlin
// Configuration not available
```

**Full Example:**
```kotlin
suspend fun processCheckoutCard(
    connectorName: String,
    credentials: Map<String, String>
): String {
    println("\n${"=".repeat(60)}")
    println("Processing Card Payment via $connectorName")
    println("${"=".repeat(60)}")
    
    // Load probe data for this connector
    val probeData = loadProbeData(connectorName)
    
    // Build config
    val config = getConnectorConfig(connectorName, credentials)
    val client = PaymentClient(config)
    
    // Build request from probe data
    val authRequest = buildAuthorizeRequest(probeData, CaptureMethod.MANUAL)
    
    println("\nRequest: ${Gson().toJson(authRequest)}")
    
    // Step 1: Authorize
    println("\n[1/2] Authorizing...")
    val authResponse = client.authorize(authRequest)
    
    println("Status: ${authResponse.status}")
    println("Connector Transaction ID: ${authResponse.connectorTransactionId}")
    
    // Handle status
    return when (authResponse.status) {
        payments.PaymentStatus.FAILURE -> {
            println("❌ Payment declined: ${authResponse.error?.message ?: "Unknown error"}")
            "failed"
        }
        payments.PaymentStatus.PENDING -> {
            println("⏳ Payment pending - awaiting async confirmation")
            println("   (In production: wait for webhook, then poll get())")
            "pending"
        }
        payments.PaymentStatus.AUTHORIZED -> {
            println("✅ Funds reserved successfully")
            
            // Step 2: Capture
            println("\n[2/2] Capturing...")
            val captureRequest = buildCaptureRequest(
                authResponse.connectorTransactionId,
                authResponse.amount
            )
            
            val captureResponse = client.capture(captureRequest)
            println("Capture Status: ${captureResponse.status}")
            
            when (captureResponse.status) {
                payments.PaymentStatus.CAPTURED, payments.PaymentStatus.AUTHORIZED -> {
                    println("✅ Payment captured successfully")
                    "success"
                }
                else -> {
                    println("⚠️  Capture returned: ${captureResponse.status}")
                    "partial"
                }
            }
        }
        else -> {
            println("⚠️  Unexpected status: ${authResponse.status}")
            "error"
        }
    }
}
```

</details>


<details>
<summary><b>Paysafe</b></summary>

**Configuration:**
```kotlin
// Configuration not available
```

**Full Example:**
```kotlin
suspend fun processCheckoutCard(
    connectorName: String,
    credentials: Map<String, String>
): String {
    println("\n${"=".repeat(60)}")
    println("Processing Card Payment via $connectorName")
    println("${"=".repeat(60)}")
    
    // Load probe data for this connector
    val probeData = loadProbeData(connectorName)
    
    // Build config
    val config = getConnectorConfig(connectorName, credentials)
    val client = PaymentClient(config)
    
    // Build request from probe data
    val authRequest = buildAuthorizeRequest(probeData, CaptureMethod.MANUAL)
    
    println("\nRequest: ${Gson().toJson(authRequest)}")
    
    // Step 1: Authorize
    println("\n[1/2] Authorizing...")
    val authResponse = client.authorize(authRequest)
    
    println("Status: ${authResponse.status}")
    println("Connector Transaction ID: ${authResponse.connectorTransactionId}")
    
    // Handle status
    return when (authResponse.status) {
        payments.PaymentStatus.FAILURE -> {
            println("❌ Payment declined: ${authResponse.error?.message ?: "Unknown error"}")
            "failed"
        }
        payments.PaymentStatus.PENDING -> {
            println("⏳ Payment pending - awaiting async confirmation")
            println("   (In production: wait for webhook, then poll get())")
            "pending"
        }
        payments.PaymentStatus.AUTHORIZED -> {
            println("✅ Funds reserved successfully")
            
            // Step 2: Capture
            println("\n[2/2] Capturing...")
            val captureRequest = buildCaptureRequest(
                authResponse.connectorTransactionId,
                authResponse.amount
            )
            
            val captureResponse = client.capture(captureRequest)
            println("Capture Status: ${captureResponse.status}")
            
            when (captureResponse.status) {
                payments.PaymentStatus.CAPTURED, payments.PaymentStatus.AUTHORIZED -> {
                    println("✅ Payment captured successfully")
                    "success"
                }
                else -> {
                    println("⚠️  Capture returned: ${captureResponse.status}")
                    "partial"
                }
            }
        }
        else -> {
            println("⚠️  Unexpected status: ${authResponse.status}")
            "error"
        }
    }
}
```

</details>


<details>
<summary><b>PlacetoPay</b></summary>

**Configuration:**
```kotlin
// Configuration not available
```

**Full Example:**
```kotlin
suspend fun processCheckoutCard(
    connectorName: String,
    credentials: Map<String, String>
): String {
    println("\n${"=".repeat(60)}")
    println("Processing Card Payment via $connectorName")
    println("${"=".repeat(60)}")
    
    // Load probe data for this connector
    val probeData = loadProbeData(connectorName)
    
    // Build config
    val config = getConnectorConfig(connectorName, credentials)
    val client = PaymentClient(config)
    
    // Build request from probe data
    val authRequest = buildAuthorizeRequest(probeData, CaptureMethod.MANUAL)
    
    println("\nRequest: ${Gson().toJson(authRequest)}")
    
    // Step 1: Authorize
    println("\n[1/2] Authorizing...")
    val authResponse = client.authorize(authRequest)
    
    println("Status: ${authResponse.status}")
    println("Connector Transaction ID: ${authResponse.connectorTransactionId}")
    
    // Handle status
    return when (authResponse.status) {
        payments.PaymentStatus.FAILURE -> {
            println("❌ Payment declined: ${authResponse.error?.message ?: "Unknown error"}")
            "failed"
        }
        payments.PaymentStatus.PENDING -> {
            println("⏳ Payment pending - awaiting async confirmation")
            println("   (In production: wait for webhook, then poll get())")
            "pending"
        }
        payments.PaymentStatus.AUTHORIZED -> {
            println("✅ Funds reserved successfully")
            
            // Step 2: Capture
            println("\n[2/2] Capturing...")
            val captureRequest = buildCaptureRequest(
                authResponse.connectorTransactionId,
                authResponse.amount
            )
            
            val captureResponse = client.capture(captureRequest)
            println("Capture Status: ${captureResponse.status}")
            
            when (captureResponse.status) {
                payments.PaymentStatus.CAPTURED, payments.PaymentStatus.AUTHORIZED -> {
                    println("✅ Payment captured successfully")
                    "success"
                }
                else -> {
                    println("⚠️  Capture returned: ${captureResponse.status}")
                    "partial"
                }
            }
        }
        else -> {
            println("⚠️  Unexpected status: ${authResponse.status}")
            "error"
        }
    }
}
```

</details>


<details>
<summary><b>PowerTranz</b></summary>

**Configuration:**
```kotlin
// Configuration not available
```

**Full Example:**
```kotlin
suspend fun processCheckoutCard(
    connectorName: String,
    credentials: Map<String, String>
): String {
    println("\n${"=".repeat(60)}")
    println("Processing Card Payment via $connectorName")
    println("${"=".repeat(60)}")
    
    // Load probe data for this connector
    val probeData = loadProbeData(connectorName)
    
    // Build config
    val config = getConnectorConfig(connectorName, credentials)
    val client = PaymentClient(config)
    
    // Build request from probe data
    val authRequest = buildAuthorizeRequest(probeData, CaptureMethod.MANUAL)
    
    println("\nRequest: ${Gson().toJson(authRequest)}")
    
    // Step 1: Authorize
    println("\n[1/2] Authorizing...")
    val authResponse = client.authorize(authRequest)
    
    println("Status: ${authResponse.status}")
    println("Connector Transaction ID: ${authResponse.connectorTransactionId}")
    
    // Handle status
    return when (authResponse.status) {
        payments.PaymentStatus.FAILURE -> {
            println("❌ Payment declined: ${authResponse.error?.message ?: "Unknown error"}")
            "failed"
        }
        payments.PaymentStatus.PENDING -> {
            println("⏳ Payment pending - awaiting async confirmation")
            println("   (In production: wait for webhook, then poll get())")
            "pending"
        }
        payments.PaymentStatus.AUTHORIZED -> {
            println("✅ Funds reserved successfully")
            
            // Step 2: Capture
            println("\n[2/2] Capturing...")
            val captureRequest = buildCaptureRequest(
                authResponse.connectorTransactionId,
                authResponse.amount
            )
            
            val captureResponse = client.capture(captureRequest)
            println("Capture Status: ${captureResponse.status}")
            
            when (captureResponse.status) {
                payments.PaymentStatus.CAPTURED, payments.PaymentStatus.AUTHORIZED -> {
                    println("✅ Payment captured successfully")
                    "success"
                }
                else -> {
                    println("⚠️  Capture returned: ${captureResponse.status}")
                    "partial"
                }
            }
        }
        else -> {
            println("⚠️  Unexpected status: ${authResponse.status}")
            "error"
        }
    }
}
```

</details>


<details>
<summary><b>Rapyd</b></summary>

**Configuration:**
```kotlin
// Configuration not available
```

**Full Example:**
```kotlin
suspend fun processCheckoutCard(
    connectorName: String,
    credentials: Map<String, String>
): String {
    println("\n${"=".repeat(60)}")
    println("Processing Card Payment via $connectorName")
    println("${"=".repeat(60)}")
    
    // Load probe data for this connector
    val probeData = loadProbeData(connectorName)
    
    // Build config
    val config = getConnectorConfig(connectorName, credentials)
    val client = PaymentClient(config)
    
    // Build request from probe data
    val authRequest = buildAuthorizeRequest(probeData, CaptureMethod.MANUAL)
    
    println("\nRequest: ${Gson().toJson(authRequest)}")
    
    // Step 1: Authorize
    println("\n[1/2] Authorizing...")
    val authResponse = client.authorize(authRequest)
    
    println("Status: ${authResponse.status}")
    println("Connector Transaction ID: ${authResponse.connectorTransactionId}")
    
    // Handle status
    return when (authResponse.status) {
        payments.PaymentStatus.FAILURE -> {
            println("❌ Payment declined: ${authResponse.error?.message ?: "Unknown error"}")
            "failed"
        }
        payments.PaymentStatus.PENDING -> {
            println("⏳ Payment pending - awaiting async confirmation")
            println("   (In production: wait for webhook, then poll get())")
            "pending"
        }
        payments.PaymentStatus.AUTHORIZED -> {
            println("✅ Funds reserved successfully")
            
            // Step 2: Capture
            println("\n[2/2] Capturing...")
            val captureRequest = buildCaptureRequest(
                authResponse.connectorTransactionId,
                authResponse.amount
            )
            
            val captureResponse = client.capture(captureRequest)
            println("Capture Status: ${captureResponse.status}")
            
            when (captureResponse.status) {
                payments.PaymentStatus.CAPTURED, payments.PaymentStatus.AUTHORIZED -> {
                    println("✅ Payment captured successfully")
                    "success"
                }
                else -> {
                    println("⚠️  Capture returned: ${captureResponse.status}")
                    "partial"
                }
            }
        }
        else -> {
            println("⚠️  Unexpected status: ${authResponse.status}")
            "error"
        }
    }
}
```

</details>


<details>
<summary><b>Razorpay</b></summary>

**Configuration:**
```kotlin
// Configuration not available
```

**Full Example:**
```kotlin
suspend fun processCheckoutCard(
    connectorName: String,
    credentials: Map<String, String>
): String {
    println("\n${"=".repeat(60)}")
    println("Processing Card Payment via $connectorName")
    println("${"=".repeat(60)}")
    
    // Load probe data for this connector
    val probeData = loadProbeData(connectorName)
    
    // Build config
    val config = getConnectorConfig(connectorName, credentials)
    val client = PaymentClient(config)
    
    // Build request from probe data
    val authRequest = buildAuthorizeRequest(probeData, CaptureMethod.MANUAL)
    
    println("\nRequest: ${Gson().toJson(authRequest)}")
    
    // Step 1: Authorize
    println("\n[1/2] Authorizing...")
    val authResponse = client.authorize(authRequest)
    
    println("Status: ${authResponse.status}")
    println("Connector Transaction ID: ${authResponse.connectorTransactionId}")
    
    // Handle status
    return when (authResponse.status) {
        payments.PaymentStatus.FAILURE -> {
            println("❌ Payment declined: ${authResponse.error?.message ?: "Unknown error"}")
            "failed"
        }
        payments.PaymentStatus.PENDING -> {
            println("⏳ Payment pending - awaiting async confirmation")
            println("   (In production: wait for webhook, then poll get())")
            "pending"
        }
        payments.PaymentStatus.AUTHORIZED -> {
            println("✅ Funds reserved successfully")
            
            // Step 2: Capture
            println("\n[2/2] Capturing...")
            val captureRequest = buildCaptureRequest(
                authResponse.connectorTransactionId,
                authResponse.amount
            )
            
            val captureResponse = client.capture(captureRequest)
            println("Capture Status: ${captureResponse.status}")
            
            when (captureResponse.status) {
                payments.PaymentStatus.CAPTURED, payments.PaymentStatus.AUTHORIZED -> {
                    println("✅ Payment captured successfully")
                    "success"
                }
                else -> {
                    println("⚠️  Capture returned: ${captureResponse.status}")
                    "partial"
                }
            }
        }
        else -> {
            println("⚠️  Unexpected status: ${authResponse.status}")
            "error"
        }
    }
}
```

</details>


<details>
<summary><b>Revolut</b></summary>

**Configuration:**
```kotlin
// Configuration not available
```

**Full Example:**
```kotlin
suspend fun processCheckoutCard(
    connectorName: String,
    credentials: Map<String, String>
): String {
    println("\n${"=".repeat(60)}")
    println("Processing Card Payment via $connectorName")
    println("${"=".repeat(60)}")
    
    // Load probe data for this connector
    val probeData = loadProbeData(connectorName)
    
    // Build config
    val config = getConnectorConfig(connectorName, credentials)
    val client = PaymentClient(config)
    
    // Build request from probe data
    val authRequest = buildAuthorizeRequest(probeData, CaptureMethod.MANUAL)
    
    println("\nRequest: ${Gson().toJson(authRequest)}")
    
    // Step 1: Authorize
    println("\n[1/2] Authorizing...")
    val authResponse = client.authorize(authRequest)
    
    println("Status: ${authResponse.status}")
    println("Connector Transaction ID: ${authResponse.connectorTransactionId}")
    
    // Handle status
    return when (authResponse.status) {
        payments.PaymentStatus.FAILURE -> {
            println("❌ Payment declined: ${authResponse.error?.message ?: "Unknown error"}")
            "failed"
        }
        payments.PaymentStatus.PENDING -> {
            println("⏳ Payment pending - awaiting async confirmation")
            println("   (In production: wait for webhook, then poll get())")
            "pending"
        }
        payments.PaymentStatus.AUTHORIZED -> {
            println("✅ Funds reserved successfully")
            
            // Step 2: Capture
            println("\n[2/2] Capturing...")
            val captureRequest = buildCaptureRequest(
                authResponse.connectorTransactionId,
                authResponse.amount
            )
            
            val captureResponse = client.capture(captureRequest)
            println("Capture Status: ${captureResponse.status}")
            
            when (captureResponse.status) {
                payments.PaymentStatus.CAPTURED, payments.PaymentStatus.AUTHORIZED -> {
                    println("✅ Payment captured successfully")
                    "success"
                }
                else -> {
                    println("⚠️  Capture returned: ${captureResponse.status}")
                    "partial"
                }
            }
        }
        else -> {
            println("⚠️  Unexpected status: ${authResponse.status}")
            "error"
        }
    }
}
```

</details>


<details>
<summary><b>Revolv3</b></summary>

**Configuration:**
```kotlin
// Configuration not available
```

**Full Example:**
```kotlin
suspend fun processCheckoutCard(
    connectorName: String,
    credentials: Map<String, String>
): String {
    println("\n${"=".repeat(60)}")
    println("Processing Card Payment via $connectorName")
    println("${"=".repeat(60)}")
    
    // Load probe data for this connector
    val probeData = loadProbeData(connectorName)
    
    // Build config
    val config = getConnectorConfig(connectorName, credentials)
    val client = PaymentClient(config)
    
    // Build request from probe data
    val authRequest = buildAuthorizeRequest(probeData, CaptureMethod.MANUAL)
    
    println("\nRequest: ${Gson().toJson(authRequest)}")
    
    // Step 1: Authorize
    println("\n[1/2] Authorizing...")
    val authResponse = client.authorize(authRequest)
    
    println("Status: ${authResponse.status}")
    println("Connector Transaction ID: ${authResponse.connectorTransactionId}")
    
    // Handle status
    return when (authResponse.status) {
        payments.PaymentStatus.FAILURE -> {
            println("❌ Payment declined: ${authResponse.error?.message ?: "Unknown error"}")
            "failed"
        }
        payments.PaymentStatus.PENDING -> {
            println("⏳ Payment pending - awaiting async confirmation")
            println("   (In production: wait for webhook, then poll get())")
            "pending"
        }
        payments.PaymentStatus.AUTHORIZED -> {
            println("✅ Funds reserved successfully")
            
            // Step 2: Capture
            println("\n[2/2] Capturing...")
            val captureRequest = buildCaptureRequest(
                authResponse.connectorTransactionId,
                authResponse.amount
            )
            
            val captureResponse = client.capture(captureRequest)
            println("Capture Status: ${captureResponse.status}")
            
            when (captureResponse.status) {
                payments.PaymentStatus.CAPTURED, payments.PaymentStatus.AUTHORIZED -> {
                    println("✅ Payment captured successfully")
                    "success"
                }
                else -> {
                    println("⚠️  Capture returned: ${captureResponse.status}")
                    "partial"
                }
            }
        }
        else -> {
            println("⚠️  Unexpected status: ${authResponse.status}")
            "error"
        }
    }
}
```

</details>


<details>
<summary><b>Shift4</b></summary>

**Configuration:**
```kotlin
// Configuration not available
```

**Full Example:**
```kotlin
suspend fun processCheckoutCard(
    connectorName: String,
    credentials: Map<String, String>
): String {
    println("\n${"=".repeat(60)}")
    println("Processing Card Payment via $connectorName")
    println("${"=".repeat(60)}")
    
    // Load probe data for this connector
    val probeData = loadProbeData(connectorName)
    
    // Build config
    val config = getConnectorConfig(connectorName, credentials)
    val client = PaymentClient(config)
    
    // Build request from probe data
    val authRequest = buildAuthorizeRequest(probeData, CaptureMethod.MANUAL)
    
    println("\nRequest: ${Gson().toJson(authRequest)}")
    
    // Step 1: Authorize
    println("\n[1/2] Authorizing...")
    val authResponse = client.authorize(authRequest)
    
    println("Status: ${authResponse.status}")
    println("Connector Transaction ID: ${authResponse.connectorTransactionId}")
    
    // Handle status
    return when (authResponse.status) {
        payments.PaymentStatus.FAILURE -> {
            println("❌ Payment declined: ${authResponse.error?.message ?: "Unknown error"}")
            "failed"
        }
        payments.PaymentStatus.PENDING -> {
            println("⏳ Payment pending - awaiting async confirmation")
            println("   (In production: wait for webhook, then poll get())")
            "pending"
        }
        payments.PaymentStatus.AUTHORIZED -> {
            println("✅ Funds reserved successfully")
            
            // Step 2: Capture
            println("\n[2/2] Capturing...")
            val captureRequest = buildCaptureRequest(
                authResponse.connectorTransactionId,
                authResponse.amount
            )
            
            val captureResponse = client.capture(captureRequest)
            println("Capture Status: ${captureResponse.status}")
            
            when (captureResponse.status) {
                payments.PaymentStatus.CAPTURED, payments.PaymentStatus.AUTHORIZED -> {
                    println("✅ Payment captured successfully")
                    "success"
                }
                else -> {
                    println("⚠️  Capture returned: ${captureResponse.status}")
                    "partial"
                }
            }
        }
        else -> {
            println("⚠️  Unexpected status: ${authResponse.status}")
            "error"
        }
    }
}
```

</details>


<details>
<summary><b>Silverflow</b></summary>

**Configuration:**
```kotlin
// Configuration not available
```

**Full Example:**
```kotlin
suspend fun processCheckoutCard(
    connectorName: String,
    credentials: Map<String, String>
): String {
    println("\n${"=".repeat(60)}")
    println("Processing Card Payment via $connectorName")
    println("${"=".repeat(60)}")
    
    // Load probe data for this connector
    val probeData = loadProbeData(connectorName)
    
    // Build config
    val config = getConnectorConfig(connectorName, credentials)
    val client = PaymentClient(config)
    
    // Build request from probe data
    val authRequest = buildAuthorizeRequest(probeData, CaptureMethod.MANUAL)
    
    println("\nRequest: ${Gson().toJson(authRequest)}")
    
    // Step 1: Authorize
    println("\n[1/2] Authorizing...")
    val authResponse = client.authorize(authRequest)
    
    println("Status: ${authResponse.status}")
    println("Connector Transaction ID: ${authResponse.connectorTransactionId}")
    
    // Handle status
    return when (authResponse.status) {
        payments.PaymentStatus.FAILURE -> {
            println("❌ Payment declined: ${authResponse.error?.message ?: "Unknown error"}")
            "failed"
        }
        payments.PaymentStatus.PENDING -> {
            println("⏳ Payment pending - awaiting async confirmation")
            println("   (In production: wait for webhook, then poll get())")
            "pending"
        }
        payments.PaymentStatus.AUTHORIZED -> {
            println("✅ Funds reserved successfully")
            
            // Step 2: Capture
            println("\n[2/2] Capturing...")
            val captureRequest = buildCaptureRequest(
                authResponse.connectorTransactionId,
                authResponse.amount
            )
            
            val captureResponse = client.capture(captureRequest)
            println("Capture Status: ${captureResponse.status}")
            
            when (captureResponse.status) {
                payments.PaymentStatus.CAPTURED, payments.PaymentStatus.AUTHORIZED -> {
                    println("✅ Payment captured successfully")
                    "success"
                }
                else -> {
                    println("⚠️  Capture returned: ${captureResponse.status}")
                    "partial"
                }
            }
        }
        else -> {
            println("⚠️  Unexpected status: ${authResponse.status}")
            "error"
        }
    }
}
```

</details>


<details>
<summary><b>Stax</b></summary>

**Configuration:**
```kotlin
// Configuration not available
```

**Full Example:**
```kotlin
suspend fun processCheckoutCard(
    connectorName: String,
    credentials: Map<String, String>
): String {
    println("\n${"=".repeat(60)}")
    println("Processing Card Payment via $connectorName")
    println("${"=".repeat(60)}")
    
    // Load probe data for this connector
    val probeData = loadProbeData(connectorName)
    
    // Build config
    val config = getConnectorConfig(connectorName, credentials)
    val client = PaymentClient(config)
    
    // Build request from probe data
    val authRequest = buildAuthorizeRequest(probeData, CaptureMethod.MANUAL)
    
    println("\nRequest: ${Gson().toJson(authRequest)}")
    
    // Step 1: Authorize
    println("\n[1/2] Authorizing...")
    val authResponse = client.authorize(authRequest)
    
    println("Status: ${authResponse.status}")
    println("Connector Transaction ID: ${authResponse.connectorTransactionId}")
    
    // Handle status
    return when (authResponse.status) {
        payments.PaymentStatus.FAILURE -> {
            println("❌ Payment declined: ${authResponse.error?.message ?: "Unknown error"}")
            "failed"
        }
        payments.PaymentStatus.PENDING -> {
            println("⏳ Payment pending - awaiting async confirmation")
            println("   (In production: wait for webhook, then poll get())")
            "pending"
        }
        payments.PaymentStatus.AUTHORIZED -> {
            println("✅ Funds reserved successfully")
            
            // Step 2: Capture
            println("\n[2/2] Capturing...")
            val captureRequest = buildCaptureRequest(
                authResponse.connectorTransactionId,
                authResponse.amount
            )
            
            val captureResponse = client.capture(captureRequest)
            println("Capture Status: ${captureResponse.status}")
            
            when (captureResponse.status) {
                payments.PaymentStatus.CAPTURED, payments.PaymentStatus.AUTHORIZED -> {
                    println("✅ Payment captured successfully")
                    "success"
                }
                else -> {
                    println("⚠️  Capture returned: ${captureResponse.status}")
                    "partial"
                }
            }
        }
        else -> {
            println("⚠️  Unexpected status: ${authResponse.status}")
            "error"
        }
    }
}
```

</details>


<details>
<summary><b>Stripe</b></summary>

**Configuration:**
```kotlin
fun getStripeConfig(apiKey: String): ConnectorConfig {
    return ConnectorConfig.newBuilder().apply {
        options = SdkOptions.newBuilder().apply {
            environment = Environment.SANDBOX
        }.build()
        connectorConfig = ConnectorSpecificConfig.newBuilder().apply {
            stripeBuilder.apply {
                this.apiKey = apiKey
            }
        }.build()
    }.build()
}
```

**Full Example:**
```kotlin
suspend fun processCheckoutCard(
    connectorName: String,
    credentials: Map<String, String>
): String {
    println("\n${"=".repeat(60)}")
    println("Processing Card Payment via $connectorName")
    println("${"=".repeat(60)}")
    
    // Load probe data for this connector
    val probeData = loadProbeData(connectorName)
    
    // Build config
    val config = getConnectorConfig(connectorName, credentials)
    val client = PaymentClient(config)
    
    // Build request from probe data
    val authRequest = buildAuthorizeRequest(probeData, CaptureMethod.MANUAL)
    
    println("\nRequest: ${Gson().toJson(authRequest)}")
    
    // Step 1: Authorize
    println("\n[1/2] Authorizing...")
    val authResponse = client.authorize(authRequest)
    
    println("Status: ${authResponse.status}")
    println("Connector Transaction ID: ${authResponse.connectorTransactionId}")
    
    // Handle status
    return when (authResponse.status) {
        payments.PaymentStatus.FAILURE -> {
            println("❌ Payment declined: ${authResponse.error?.message ?: "Unknown error"}")
            "failed"
        }
        payments.PaymentStatus.PENDING -> {
            println("⏳ Payment pending - awaiting async confirmation")
            println("   (In production: wait for webhook, then poll get())")
            "pending"
        }
        payments.PaymentStatus.AUTHORIZED -> {
            println("✅ Funds reserved successfully")
            
            // Step 2: Capture
            println("\n[2/2] Capturing...")
            val captureRequest = buildCaptureRequest(
                authResponse.connectorTransactionId,
                authResponse.amount
            )
            
            val captureResponse = client.capture(captureRequest)
            println("Capture Status: ${captureResponse.status}")
            
            when (captureResponse.status) {
                payments.PaymentStatus.CAPTURED, payments.PaymentStatus.AUTHORIZED -> {
                    println("✅ Payment captured successfully")
                    "success"
                }
                else -> {
                    println("⚠️  Capture returned: ${captureResponse.status}")
                    "partial"
                }
            }
        }
        else -> {
            println("⚠️  Unexpected status: ${authResponse.status}")
            "error"
        }
    }
}
```

</details>


<details>
<summary><b>Trust Payments</b></summary>

**Configuration:**
```kotlin
// Configuration not available
```

**Full Example:**
```kotlin
suspend fun processCheckoutCard(
    connectorName: String,
    credentials: Map<String, String>
): String {
    println("\n${"=".repeat(60)}")
    println("Processing Card Payment via $connectorName")
    println("${"=".repeat(60)}")
    
    // Load probe data for this connector
    val probeData = loadProbeData(connectorName)
    
    // Build config
    val config = getConnectorConfig(connectorName, credentials)
    val client = PaymentClient(config)
    
    // Build request from probe data
    val authRequest = buildAuthorizeRequest(probeData, CaptureMethod.MANUAL)
    
    println("\nRequest: ${Gson().toJson(authRequest)}")
    
    // Step 1: Authorize
    println("\n[1/2] Authorizing...")
    val authResponse = client.authorize(authRequest)
    
    println("Status: ${authResponse.status}")
    println("Connector Transaction ID: ${authResponse.connectorTransactionId}")
    
    // Handle status
    return when (authResponse.status) {
        payments.PaymentStatus.FAILURE -> {
            println("❌ Payment declined: ${authResponse.error?.message ?: "Unknown error"}")
            "failed"
        }
        payments.PaymentStatus.PENDING -> {
            println("⏳ Payment pending - awaiting async confirmation")
            println("   (In production: wait for webhook, then poll get())")
            "pending"
        }
        payments.PaymentStatus.AUTHORIZED -> {
            println("✅ Funds reserved successfully")
            
            // Step 2: Capture
            println("\n[2/2] Capturing...")
            val captureRequest = buildCaptureRequest(
                authResponse.connectorTransactionId,
                authResponse.amount
            )
            
            val captureResponse = client.capture(captureRequest)
            println("Capture Status: ${captureResponse.status}")
            
            when (captureResponse.status) {
                payments.PaymentStatus.CAPTURED, payments.PaymentStatus.AUTHORIZED -> {
                    println("✅ Payment captured successfully")
                    "success"
                }
                else -> {
                    println("⚠️  Capture returned: ${captureResponse.status}")
                    "partial"
                }
            }
        }
        else -> {
            println("⚠️  Unexpected status: ${authResponse.status}")
            "error"
        }
    }
}
```

</details>


<details>
<summary><b>TSYS</b></summary>

**Configuration:**
```kotlin
// Configuration not available
```

**Full Example:**
```kotlin
suspend fun processCheckoutCard(
    connectorName: String,
    credentials: Map<String, String>
): String {
    println("\n${"=".repeat(60)}")
    println("Processing Card Payment via $connectorName")
    println("${"=".repeat(60)}")
    
    // Load probe data for this connector
    val probeData = loadProbeData(connectorName)
    
    // Build config
    val config = getConnectorConfig(connectorName, credentials)
    val client = PaymentClient(config)
    
    // Build request from probe data
    val authRequest = buildAuthorizeRequest(probeData, CaptureMethod.MANUAL)
    
    println("\nRequest: ${Gson().toJson(authRequest)}")
    
    // Step 1: Authorize
    println("\n[1/2] Authorizing...")
    val authResponse = client.authorize(authRequest)
    
    println("Status: ${authResponse.status}")
    println("Connector Transaction ID: ${authResponse.connectorTransactionId}")
    
    // Handle status
    return when (authResponse.status) {
        payments.PaymentStatus.FAILURE -> {
            println("❌ Payment declined: ${authResponse.error?.message ?: "Unknown error"}")
            "failed"
        }
        payments.PaymentStatus.PENDING -> {
            println("⏳ Payment pending - awaiting async confirmation")
            println("   (In production: wait for webhook, then poll get())")
            "pending"
        }
        payments.PaymentStatus.AUTHORIZED -> {
            println("✅ Funds reserved successfully")
            
            // Step 2: Capture
            println("\n[2/2] Capturing...")
            val captureRequest = buildCaptureRequest(
                authResponse.connectorTransactionId,
                authResponse.amount
            )
            
            val captureResponse = client.capture(captureRequest)
            println("Capture Status: ${captureResponse.status}")
            
            when (captureResponse.status) {
                payments.PaymentStatus.CAPTURED, payments.PaymentStatus.AUTHORIZED -> {
                    println("✅ Payment captured successfully")
                    "success"
                }
                else -> {
                    println("⚠️  Capture returned: ${captureResponse.status}")
                    "partial"
                }
            }
        }
        else -> {
            println("⚠️  Unexpected status: ${authResponse.status}")
            "error"
        }
    }
}
```

</details>


<details>
<summary><b>Wells Fargo</b></summary>

**Configuration:**
```kotlin
// Configuration not available
```

**Full Example:**
```kotlin
suspend fun processCheckoutCard(
    connectorName: String,
    credentials: Map<String, String>
): String {
    println("\n${"=".repeat(60)}")
    println("Processing Card Payment via $connectorName")
    println("${"=".repeat(60)}")
    
    // Load probe data for this connector
    val probeData = loadProbeData(connectorName)
    
    // Build config
    val config = getConnectorConfig(connectorName, credentials)
    val client = PaymentClient(config)
    
    // Build request from probe data
    val authRequest = buildAuthorizeRequest(probeData, CaptureMethod.MANUAL)
    
    println("\nRequest: ${Gson().toJson(authRequest)}")
    
    // Step 1: Authorize
    println("\n[1/2] Authorizing...")
    val authResponse = client.authorize(authRequest)
    
    println("Status: ${authResponse.status}")
    println("Connector Transaction ID: ${authResponse.connectorTransactionId}")
    
    // Handle status
    return when (authResponse.status) {
        payments.PaymentStatus.FAILURE -> {
            println("❌ Payment declined: ${authResponse.error?.message ?: "Unknown error"}")
            "failed"
        }
        payments.PaymentStatus.PENDING -> {
            println("⏳ Payment pending - awaiting async confirmation")
            println("   (In production: wait for webhook, then poll get())")
            "pending"
        }
        payments.PaymentStatus.AUTHORIZED -> {
            println("✅ Funds reserved successfully")
            
            // Step 2: Capture
            println("\n[2/2] Capturing...")
            val captureRequest = buildCaptureRequest(
                authResponse.connectorTransactionId,
                authResponse.amount
            )
            
            val captureResponse = client.capture(captureRequest)
            println("Capture Status: ${captureResponse.status}")
            
            when (captureResponse.status) {
                payments.PaymentStatus.CAPTURED, payments.PaymentStatus.AUTHORIZED -> {
                    println("✅ Payment captured successfully")
                    "success"
                }
                else -> {
                    println("⚠️  Capture returned: ${captureResponse.status}")
                    "partial"
                }
            }
        }
        else -> {
            println("⚠️  Unexpected status: ${authResponse.status}")
            "error"
        }
    }
}
```

</details>


<details>
<summary><b>Worldpay</b></summary>

**Configuration:**
```kotlin
// Configuration not available
```

**Full Example:**
```kotlin
suspend fun processCheckoutCard(
    connectorName: String,
    credentials: Map<String, String>
): String {
    println("\n${"=".repeat(60)}")
    println("Processing Card Payment via $connectorName")
    println("${"=".repeat(60)}")
    
    // Load probe data for this connector
    val probeData = loadProbeData(connectorName)
    
    // Build config
    val config = getConnectorConfig(connectorName, credentials)
    val client = PaymentClient(config)
    
    // Build request from probe data
    val authRequest = buildAuthorizeRequest(probeData, CaptureMethod.MANUAL)
    
    println("\nRequest: ${Gson().toJson(authRequest)}")
    
    // Step 1: Authorize
    println("\n[1/2] Authorizing...")
    val authResponse = client.authorize(authRequest)
    
    println("Status: ${authResponse.status}")
    println("Connector Transaction ID: ${authResponse.connectorTransactionId}")
    
    // Handle status
    return when (authResponse.status) {
        payments.PaymentStatus.FAILURE -> {
            println("❌ Payment declined: ${authResponse.error?.message ?: "Unknown error"}")
            "failed"
        }
        payments.PaymentStatus.PENDING -> {
            println("⏳ Payment pending - awaiting async confirmation")
            println("   (In production: wait for webhook, then poll get())")
            "pending"
        }
        payments.PaymentStatus.AUTHORIZED -> {
            println("✅ Funds reserved successfully")
            
            // Step 2: Capture
            println("\n[2/2] Capturing...")
            val captureRequest = buildCaptureRequest(
                authResponse.connectorTransactionId,
                authResponse.amount
            )
            
            val captureResponse = client.capture(captureRequest)
            println("Capture Status: ${captureResponse.status}")
            
            when (captureResponse.status) {
                payments.PaymentStatus.CAPTURED, payments.PaymentStatus.AUTHORIZED -> {
                    println("✅ Payment captured successfully")
                    "success"
                }
                else -> {
                    println("⚠️  Capture returned: ${captureResponse.status}")
                    "partial"
                }
            }
        }
        else -> {
            println("⚠️  Unexpected status: ${authResponse.status}")
            "error"
        }
    }
}
```

</details>


<details>
<summary><b>Worldpay Vantiv</b></summary>

**Configuration:**
```kotlin
// Configuration not available
```

**Full Example:**
```kotlin
suspend fun processCheckoutCard(
    connectorName: String,
    credentials: Map<String, String>
): String {
    println("\n${"=".repeat(60)}")
    println("Processing Card Payment via $connectorName")
    println("${"=".repeat(60)}")
    
    // Load probe data for this connector
    val probeData = loadProbeData(connectorName)
    
    // Build config
    val config = getConnectorConfig(connectorName, credentials)
    val client = PaymentClient(config)
    
    // Build request from probe data
    val authRequest = buildAuthorizeRequest(probeData, CaptureMethod.MANUAL)
    
    println("\nRequest: ${Gson().toJson(authRequest)}")
    
    // Step 1: Authorize
    println("\n[1/2] Authorizing...")
    val authResponse = client.authorize(authRequest)
    
    println("Status: ${authResponse.status}")
    println("Connector Transaction ID: ${authResponse.connectorTransactionId}")
    
    // Handle status
    return when (authResponse.status) {
        payments.PaymentStatus.FAILURE -> {
            println("❌ Payment declined: ${authResponse.error?.message ?: "Unknown error"}")
            "failed"
        }
        payments.PaymentStatus.PENDING -> {
            println("⏳ Payment pending - awaiting async confirmation")
            println("   (In production: wait for webhook, then poll get())")
            "pending"
        }
        payments.PaymentStatus.AUTHORIZED -> {
            println("✅ Funds reserved successfully")
            
            // Step 2: Capture
            println("\n[2/2] Capturing...")
            val captureRequest = buildCaptureRequest(
                authResponse.connectorTransactionId,
                authResponse.amount
            )
            
            val captureResponse = client.capture(captureRequest)
            println("Capture Status: ${captureResponse.status}")
            
            when (captureResponse.status) {
                payments.PaymentStatus.CAPTURED, payments.PaymentStatus.AUTHORIZED -> {
                    println("✅ Payment captured successfully")
                    "success"
                }
                else -> {
                    println("⚠️  Capture returned: ${captureResponse.status}")
                    "partial"
                }
            }
        }
        else -> {
            println("⚠️  Unexpected status: ${authResponse.status}")
            "error"
        }
    }
}
```

</details>


<details>
<summary><b>Worldpay XML</b></summary>

**Configuration:**
```kotlin
// Configuration not available
```

**Full Example:**
```kotlin
suspend fun processCheckoutCard(
    connectorName: String,
    credentials: Map<String, String>
): String {
    println("\n${"=".repeat(60)}")
    println("Processing Card Payment via $connectorName")
    println("${"=".repeat(60)}")
    
    // Load probe data for this connector
    val probeData = loadProbeData(connectorName)
    
    // Build config
    val config = getConnectorConfig(connectorName, credentials)
    val client = PaymentClient(config)
    
    // Build request from probe data
    val authRequest = buildAuthorizeRequest(probeData, CaptureMethod.MANUAL)
    
    println("\nRequest: ${Gson().toJson(authRequest)}")
    
    // Step 1: Authorize
    println("\n[1/2] Authorizing...")
    val authResponse = client.authorize(authRequest)
    
    println("Status: ${authResponse.status}")
    println("Connector Transaction ID: ${authResponse.connectorTransactionId}")
    
    // Handle status
    return when (authResponse.status) {
        payments.PaymentStatus.FAILURE -> {
            println("❌ Payment declined: ${authResponse.error?.message ?: "Unknown error"}")
            "failed"
        }
        payments.PaymentStatus.PENDING -> {
            println("⏳ Payment pending - awaiting async confirmation")
            println("   (In production: wait for webhook, then poll get())")
            "pending"
        }
        payments.PaymentStatus.AUTHORIZED -> {
            println("✅ Funds reserved successfully")
            
            // Step 2: Capture
            println("\n[2/2] Capturing...")
            val captureRequest = buildCaptureRequest(
                authResponse.connectorTransactionId,
                authResponse.amount
            )
            
            val captureResponse = client.capture(captureRequest)
            println("Capture Status: ${captureResponse.status}")
            
            when (captureResponse.status) {
                payments.PaymentStatus.CAPTURED, payments.PaymentStatus.AUTHORIZED -> {
                    println("✅ Payment captured successfully")
                    "success"
                }
                else -> {
                    println("⚠️  Capture returned: ${captureResponse.status}")
                    "partial"
                }
            }
        }
        else -> {
            println("⚠️  Unexpected status: ${authResponse.status}")
            "error"
        }
    }
}
```

</details>


<details>
<summary><b>Xendit</b></summary>

**Configuration:**
```kotlin
// Configuration not available
```

**Full Example:**
```kotlin
suspend fun processCheckoutCard(
    connectorName: String,
    credentials: Map<String, String>
): String {
    println("\n${"=".repeat(60)}")
    println("Processing Card Payment via $connectorName")
    println("${"=".repeat(60)}")
    
    // Load probe data for this connector
    val probeData = loadProbeData(connectorName)
    
    // Build config
    val config = getConnectorConfig(connectorName, credentials)
    val client = PaymentClient(config)
    
    // Build request from probe data
    val authRequest = buildAuthorizeRequest(probeData, CaptureMethod.MANUAL)
    
    println("\nRequest: ${Gson().toJson(authRequest)}")
    
    // Step 1: Authorize
    println("\n[1/2] Authorizing...")
    val authResponse = client.authorize(authRequest)
    
    println("Status: ${authResponse.status}")
    println("Connector Transaction ID: ${authResponse.connectorTransactionId}")
    
    // Handle status
    return when (authResponse.status) {
        payments.PaymentStatus.FAILURE -> {
            println("❌ Payment declined: ${authResponse.error?.message ?: "Unknown error"}")
            "failed"
        }
        payments.PaymentStatus.PENDING -> {
            println("⏳ Payment pending - awaiting async confirmation")
            println("   (In production: wait for webhook, then poll get())")
            "pending"
        }
        payments.PaymentStatus.AUTHORIZED -> {
            println("✅ Funds reserved successfully")
            
            // Step 2: Capture
            println("\n[2/2] Capturing...")
            val captureRequest = buildCaptureRequest(
                authResponse.connectorTransactionId,
                authResponse.amount
            )
            
            val captureResponse = client.capture(captureRequest)
            println("Capture Status: ${captureResponse.status}")
            
            when (captureResponse.status) {
                payments.PaymentStatus.CAPTURED, payments.PaymentStatus.AUTHORIZED -> {
                    println("✅ Payment captured successfully")
                    "success"
                }
                else -> {
                    println("⚠️  Capture returned: ${captureResponse.status}")
                    "partial"
                }
            }
        }
        else -> {
            println("⚠️  Unexpected status: ${authResponse.status}")
            "error"
        }
    }
}
```

</details>


<details>
<summary><b>Zift</b></summary>

**Configuration:**
```kotlin
// Configuration not available
```

**Full Example:**
```kotlin
suspend fun processCheckoutCard(
    connectorName: String,
    credentials: Map<String, String>
): String {
    println("\n${"=".repeat(60)}")
    println("Processing Card Payment via $connectorName")
    println("${"=".repeat(60)}")
    
    // Load probe data for this connector
    val probeData = loadProbeData(connectorName)
    
    // Build config
    val config = getConnectorConfig(connectorName, credentials)
    val client = PaymentClient(config)
    
    // Build request from probe data
    val authRequest = buildAuthorizeRequest(probeData, CaptureMethod.MANUAL)
    
    println("\nRequest: ${Gson().toJson(authRequest)}")
    
    // Step 1: Authorize
    println("\n[1/2] Authorizing...")
    val authResponse = client.authorize(authRequest)
    
    println("Status: ${authResponse.status}")
    println("Connector Transaction ID: ${authResponse.connectorTransactionId}")
    
    // Handle status
    return when (authResponse.status) {
        payments.PaymentStatus.FAILURE -> {
            println("❌ Payment declined: ${authResponse.error?.message ?: "Unknown error"}")
            "failed"
        }
        payments.PaymentStatus.PENDING -> {
            println("⏳ Payment pending - awaiting async confirmation")
            println("   (In production: wait for webhook, then poll get())")
            "pending"
        }
        payments.PaymentStatus.AUTHORIZED -> {
            println("✅ Funds reserved successfully")
            
            // Step 2: Capture
            println("\n[2/2] Capturing...")
            val captureRequest = buildCaptureRequest(
                authResponse.connectorTransactionId,
                authResponse.amount
            )
            
            val captureResponse = client.capture(captureRequest)
            println("Capture Status: ${captureResponse.status}")
            
            when (captureResponse.status) {
                payments.PaymentStatus.CAPTURED, payments.PaymentStatus.AUTHORIZED -> {
                    println("✅ Payment captured successfully")
                    "success"
                }
                else -> {
                    println("⚠️  Capture returned: ${captureResponse.status}")
                    "partial"
                }
            }
        }
        else -> {
            println("⚠️  Unexpected status: ${authResponse.status}")
            "error"
        }
    }
}
```

</details>



## Flow Reference

This scenario uses these flows in sequence:

| Step | Flow (Service.RPC) | Purpose | gRPC Request |
|------|-------------------|---------|--------------|
| 1 | PaymentService.Authorize | Authorize a payment amount on a payment method. This reserves funds without capturing them, essential for verifying availability before finalizing. | `PaymentServiceAuthorizeRequest` |
| 2 | PaymentService.Capture | Finalize an authorized payment transaction. Transfers reserved funds from customer to merchant account, completing the payment lifecycle. | `PaymentServiceCaptureRequest` |


## Common Issues

### "capture_method not supported"

Some connectors only support `AUTOMATIC` (single-step). Use the [Auto-Capture scenario](./checkout-autocapture.md) instead.

### Auth expires before capture

Capture timing varies by connector. Check the connector-specific documentation for auth window details.

## Related Scenarios



---

*This documentation was auto-generated from probe data. Last updated: 2026-03-24*