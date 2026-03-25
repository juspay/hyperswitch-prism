# Installation and Configuration


## How to install the prism library?

Start by installing the library from your terminal, using the programming language of your choice. This should be followed by configuring the environment and payment processor API keys to proceed with the next steps.

The below examples are templates for configuring Stripe and Adyen.

### Prerequisites

- Stripe test API key (get one at [stripe.com](https://stripe.com))
- Adyen test API key (get one at [adyen.com/signup](https://www.adyen.com/signup))

{% tabs %}

{% tab title="Node" %}

{% code title="Terminal" overflow="wrap" %}
```bash
npm install hyperswitch-prism
```
{% endcode %}

{% code title="index.js" overflow="wrap" lineNumbers="true" %}
```javascript
const { PaymentClient } = require('hyperswitch-prism');
const { ConnectorConfig, ConnectorSpecificConfig, SdkOptions, Environment } = require('hyperswitch-prism').types;

// Configure Stripe client
const stripeConfig = ConnectorConfig.create({
    options: SdkOptions.create({ environment: Environment.SANDBOX }),
});
stripeConfig.connectorConfig = ConnectorSpecificConfig.create({
    stripe: { apiKey: { value: process.env.STRIPE_API_KEY } }
});
const stripeClient = new PaymentClient(stripeConfig);

// Configure Adyen client
const adyenConfig = ConnectorConfig.create({
    options: SdkOptions.create({ environment: Environment.SANDBOX }),
});
adyenConfig.connectorConfig = ConnectorSpecificConfig.create({
    adyen: {
        apiKey: { value: process.env.ADYEN_API_KEY },
        merchantAccount: process.env.ADYEN_MERCHANT_ACCOUNT
    }
});
const adyenClient = new PaymentClient(adyenConfig);
```
{% endcode %}

{% endtab %}

{% tab title="Python" %}

{% code title="Terminal" overflow="wrap" %}
```bash
pip install hyperswitch-prism
```
{% endcode %}

{% code title="main.py" overflow="wrap" lineNumbers="true" %}
```python
import os
from hyperswitch_prism import PaymentClient
from hyperswitch_prism.generated import sdk_config_pb2

# Configure Stripe client
stripe_config = sdk_config_pb2.ConnectorConfig(
    options=sdk_config_pb2.SdkOptions(environment=sdk_config_pb2.Environment.SANDBOX),
)
stripe_config.connector_config.stripe.api_key.value = os.environ["STRIPE_API_KEY"]
stripe_client = PaymentClient(stripe_config)

# Configure Adyen client
adyen_config = sdk_config_pb2.ConnectorConfig(
    options=sdk_config_pb2.SdkOptions(environment=sdk_config_pb2.Environment.SANDBOX),
)
adyen_config.connector_config.adyen.api_key.value = os.environ["ADYEN_API_KEY"]
adyen_config.connector_config.adyen.merchant_account = os.environ["ADYEN_MERCHANT_ACCOUNT"]
adyen_client = PaymentClient(adyen_config)
```
{% endcode %}

{% endtab %}

{% tab title="Java" %}

{% code title="pom.xml" overflow="wrap" %}
```xml
<dependency>
    <groupId>com.juspay.hyperswitch</groupId>
    <artifactId>hyperswitch-prism</artifactId>
    <version>1.0.0</version>
</dependency>
```
{% endcode %}

{% code title="Main.java" overflow="wrap" lineNumbers="true" %}
```java
import com.juspay.hyperswitch.prism.PaymentClient;
import com.juspay.hyperswitch.prism.config.*;

// Configure Stripe client
ConnectorConfig stripeConfig = ConnectorConfig.builder()
    .options(SdkOptions.builder()
        .environment(Environment.SANDBOX)
        .build())
    .connectorSpecificConfig(ConnectorSpecificConfig.builder()
        .stripe(StripeConfig.builder()
            .apiKey(SecretString.of(System.getenv("STRIPE_API_KEY")))
            .build())
        .build())
    .build();
PaymentClient stripeClient = new PaymentClient(stripeConfig);

// Configure Adyen client
ConnectorConfig adyenConfig = ConnectorConfig.builder()
    .options(SdkOptions.builder()
        .environment(Environment.SANDBOX)
        .build())
    .connectorSpecificConfig(ConnectorSpecificConfig.builder()
        .adyen(AdyenConfig.builder()
            .apiKey(SecretString.of(System.getenv("ADYEN_API_KEY")))
            .merchantAccount(System.getenv("ADYEN_MERCHANT_ACCOUNT"))
            .build())
        .build())
    .build();
PaymentClient adyenClient = new PaymentClient(adyenConfig);
```
{% endcode %}

{% endtab %}

{% tab title="PHP" %}

{% code title="Terminal" overflow="wrap" %}
```bash
composer require hyperswitch-prism
```
{% endcode %}

{% code title="index.php" overflow="wrap" lineNumbers="true" %}
```php
<?php
require_once 'vendor/autoload.php';

use HyperswitchPrism\PaymentClient;
use HyperswitchPrism\Config\ConnectorConfig;
use HyperswitchPrism\Config\Environment;

// Configure Stripe client
$stripeConfig = ConnectorConfig::create([
    'environment' => Environment::SANDBOX,
    'stripe' => [
        'api_key' => $_ENV['STRIPE_API_KEY']
    ]
]);
$stripeClient = new PaymentClient($stripeConfig);

// Configure Adyen client
$adyenConfig = ConnectorConfig::create([
    'environment' => Environment::SANDBOX,
    'adyen' => [
        'api_key' => $_ENV['ADYEN_API_KEY'],
        'merchant_account' => $_ENV['ADYEN_MERCHANT_ACCOUNT']
    ]
]);
$adyenClient = new PaymentClient($adyenConfig);
```
{% endcode %}

{% endtab %}

{% endtabs %}

That would be all. The SDK handles native library loading automatically. Start building in the [Quick Start](./quick-start.md).

## Minimum version supported

The prerequisites are:
- **Node.js**: 16+ (FFI bindings require native compilation)
- **Python**: 3.9+ (uses `ctypes` for FFI)
- **Java**: 11+ (uses JNI bindings)
- **PHP**: 8.0+ (uses FFI extension)
