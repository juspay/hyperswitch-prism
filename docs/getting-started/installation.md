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

// Configure Stripe client
const stripeConfig = {
    connectorConfig: {
        stripe: { apiKey: { value: process.env.STRIPE_API_KEY } }
    }
};
const stripeClient = new PaymentClient(stripeConfig);

// Configure Adyen client
const adyenConfig = {
    connectorConfig: {
        adyen: {
            apiKey: { value: process.env.ADYEN_API_KEY },
            merchantAccount: process.env.ADYEN_MERCHANT_ACCOUNT
        }
    }
};
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
stripe_config = {
    "connectorConfig": {
        "stripe": {"apiKey": {"value": os.environ["STRIPE_API_KEY"]}}
    }
}
stripe_client = PaymentClient(stripe_config)

# Configure Adyen client
adyen_config = {
    "connectorConfig": {
        "adyen": {
            "apiKey": {"value": os.environ["ADYEN_API_KEY"]},
            "merchantAccount": os.environ["ADYEN_MERCHANT_ACCOUNT"]
        }
    }
}
adyen_client = PaymentClient(adyen_config)
```
{% endcode %}

{% endtab %}

{% tab title="Java" %}

{% code title="pom.xml" overflow="wrap" %}
```xml
<dependency>
    <groupId>io.hyperswitch</groupId>
    <artifactId>prism</artifactId>
    <version>1.0.0</version>
</dependency>
```
{% endcode %}

{% code title="Main.java" overflow="wrap" lineNumbers="true" %}
```java
import com.juspay.hyperswitch.prism.PaymentClient;

// Configure Stripe client
Map<String, Object> stripeConfig = new HashMap<>();
Map<String, Object> stripeConnectorConfig = new HashMap<>();
stripeConnectorConfig.put("apiKey", SecretString.of(System.getenv("STRIPE_API_KEY")));
stripeConfig.put("connectorConfig", Map.of("stripe", stripeConnectorConfig));
PaymentClient stripeClient = new PaymentClient(stripeConfig);

// Configure Adyen client
Map<String, Object> adyenConfig = new HashMap<>();
Map<String, Object> adyenConnectorConfig = new HashMap<>();
adyenConnectorConfig.put("apiKey", SecretString.of(System.getenv("ADYEN_API_KEY")));
adyenConnectorConfig.put("merchantAccount", System.getenv("ADYEN_MERCHANT_ACCOUNT"));
adyenConfig.put("connectorConfig", Map.of("adyen", adyenConnectorConfig));
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

// Configure Stripe client
$stripeConfig = [
    'connectorConfig' => [
        'stripe' => [
            'apiKey' => ['value' => $_ENV['STRIPE_API_KEY']]
        ]
    ]
];
$stripeClient = new PaymentClient($stripeConfig);

// Configure Adyen client
$adyenConfig = [
    'connectorConfig' => [
        'adyen' => [
            'apiKey' => ['value' => $_ENV['ADYEN_API_KEY']],
            'merchantAccount' => $_ENV['ADYEN_MERCHANT_ACCOUNT']
        ]
    ]
];
$adyenClient = new PaymentClient($adyenConfig);
```
{% endcode %}

{% endtab %}

{% endtabs %}

That would be all. The SDK handles native library loading automatically. Start building in the [First Payment](./first-payment.md).

## Minimum version supported

The prerequisites are:
- **Node.js**: 16+ (FFI bindings require native compilation)
- **Python**: 3.9+ (uses `ctypes` for FFI)
- **Java**: 11+ (uses JNI bindings)
- **PHP**: 8.0+ (uses FFI extension)
