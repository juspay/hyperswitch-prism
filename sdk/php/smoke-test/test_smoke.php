<?php

declare(strict_types=1);

/**
 * PHP SDK smoke test — mirrors the JavaScript and Python smoke tests.
 *
 * Loads connector credentials from creds.json and runs the authorize flow
 * against each specified connector, verifying that:
 *   1. The FFI req_transformer builds a valid HTTP request (URL, method, headers).
 *   2. The full round-trip (req → HTTP → res) either returns a PaymentStatus
 *      or a structured error proto — never a raw exception from bad plumbing.
 *
 * Usage:
 *   php -d ffi.enable=1 test_smoke.php --connectors stripe
 *   php -d ffi.enable=1 test_smoke.php --connectors stripe,adyen --dry-run
 *   php -d ffi.enable=1 test_smoke.php --all
 */

// ---------------------------------------------------------------------------
// Autoloading — works both in the local source tree and after composer install
// ---------------------------------------------------------------------------
$autoloaders = [
    __DIR__ . '/../vendor/autoload.php',       // normal composer install
    __DIR__ . '/vendor/autoload.php',           // test-package isolation dir
    __DIR__ . '/../../../vendor/autoload.php',  // monorepo root (rare)
];
$loaded = false;
foreach ($autoloaders as $al) {
    if (file_exists($al)) {
        require_once $al;
        $loaded = true;
        break;
    }
}
if (!$loaded) {
    fwrite(STDERR, "Could not find vendor/autoload.php. Run 'composer install' first.\n");
    exit(1);
}

use Payments\PaymentClient;
use Payments\RequestException;
use Payments\ResponseException;
use Types\AuthenticationType;
use Types\CaptureMethod;
use Types\ConnectorConfig;
use Types\Currency;
use Types\Environment;
use Types\PaymentServiceAuthorizeRequest;

// ---------------------------------------------------------------------------
// Test card data
// ---------------------------------------------------------------------------
const TEST_CARDS = [
    'visa' => [
        'number'  => '4111111111111111',
        'expMonth'=> '12',
        'expYear' => '2050',
        'cvc'     => '123',
        'holder'  => 'Test User',
    ],
    'mastercard' => [
        'number'  => '5555555555554444',
        'expMonth'=> '12',
        'expYear' => '2050',
        'cvc'     => '123',
        'holder'  => 'Test User',
    ],
];

const PLACEHOLDER_VALUES = ['', 'placeholder', 'test', 'dummy', 'sk_test_placeholder'];

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function loadCredentials(string $credsFile): array
{
    if (!file_exists($credsFile)) {
        throw new \RuntimeException("Credentials file not found: {$credsFile}");
    }
    $content = file_get_contents($credsFile);
    return json_decode($content, true, 512, JSON_THROW_ON_ERROR);
}

function isPlaceholder(string $value): bool
{
    if ($value === '') return true;
    $lower = strtolower($value);
    if (in_array($lower, PLACEHOLDER_VALUES, true)) return true;
    return str_contains($lower, 'placeholder');
}

function hasValidCredentials(array $authConfig): bool
{
    foreach ($authConfig as $key => $value) {
        if (in_array($key, ['metadata', '_comment'], true)) continue;
        // SecretString: { "value": "..." }
        if (is_array($value) && isset($value['value']) && is_string($value['value'])) {
            if (!isPlaceholder($value['value'])) return true;
        }
        if (is_string($value) && !isPlaceholder($value)) return true;
    }
    return false;
}

function buildAuthorizeRequest(string $cardType = 'visa'): PaymentServiceAuthorizeRequest
{
    $card = TEST_CARDS[$cardType] ?? TEST_CARDS['visa'];

    // Build card number wrapper (CardNumberType) and SecretString wrappers
    $cardNumber     = new \Types\CardNumberType(); $cardNumber->setValue($card['number']);
    $secretExpMonth = new \Types\SecretString(); $secretExpMonth->setValue($card['expMonth']);
    $secretExpYear  = new \Types\SecretString(); $secretExpYear->setValue($card['expYear']);
    $secretCvc      = new \Types\SecretString(); $secretCvc->setValue($card['cvc']);
    $secretHolder   = new \Types\SecretString(); $secretHolder->setValue($card['holder']);
    $secretEmail    = new \Types\SecretString(); $secretEmail->setValue('test@example.com');

    $cardMsg = new \Types\CardDetails();
    $cardMsg->setCardNumber($cardNumber);
    $cardMsg->setCardExpMonth($secretExpMonth);
    $cardMsg->setCardExpYear($secretExpYear);
    $cardMsg->setCardCvc($secretCvc);
    $cardMsg->setCardHolderName($secretHolder);

    $paymentMethod = new \Types\PaymentMethod();
    $paymentMethod->setCard($cardMsg);

    $money = new \Types\Money();
    $money->setMinorAmount(1000);
    $money->setCurrency(Currency::USD);

    $customer = new \Types\Customer();
    $customer->setEmail($secretEmail);
    $customer->setName('Test User');

    $req = new PaymentServiceAuthorizeRequest();
    $req->setMerchantTransactionId('smoke_test_php_' . time() . '_' . bin2hex(random_bytes(4)));
    $req->setAmount($money);
    $req->setCaptureMethod(CaptureMethod::AUTOMATIC);
    $req->setPaymentMethod($paymentMethod);
    $req->setAddress(new \Types\PaymentAddress());  // empty address, matching JS: address: {}
    $req->setCustomer($customer);
    $req->setAuthType(AuthenticationType::NO_THREE_DS);
    $req->setReturnUrl('https://example.com/return');
    $req->setWebhookUrl('https://example.com/webhook');
    $req->setTestMode(true);

    return $req;
}

function buildConnectorConfig(string $connectorName, array $authConfig): ConnectorConfig
{
    // Build connector-specific config message e.g. StripeConfig, AdyenConfig
    $configMsgClass = '\\Types\\' . ucfirst($connectorName) . 'Config';
    if (!class_exists($configMsgClass)) {
        throw new \InvalidArgumentException(
            "No config class found for connector '{$connectorName}'. Expected: {$configMsgClass}"
        );
    }
    $connectorSpecificMsg = new $configMsgClass();
    foreach ($authConfig as $key => $value) {
        if (in_array($key, ['_comment', 'metadata'], true)) continue;
        $camelKey = lcfirst(str_replace('_', '', ucwords($key, '_')));
        $setter = 'set' . ucfirst($camelKey);
        if (method_exists($connectorSpecificMsg, $setter)) {
            if (is_array($value) && isset($value['value'])) {
                $secret = new \Types\SecretString();
                $secret->setValue((string) $value['value']);
                $connectorSpecificMsg->$setter($secret);
            } else {
                $connectorSpecificMsg->$setter($value);
            }
        }
    }

    // Wrap in ConnectorSpecificConfig (oneof keyed by connector name)
    $connectorSpecificConfig = new \Types\ConnectorSpecificConfig();
    $setter = 'set' . ucfirst($connectorName);  // e.g. setStripe()
    if (!method_exists($connectorSpecificConfig, $setter)) {
        throw new \InvalidArgumentException(
            "ConnectorSpecificConfig has no setter for connector '{$connectorName}'. "
            . "Expected method: {$setter}()"
        );
    }
    $connectorSpecificConfig->$setter($connectorSpecificMsg);

    $sdkOptions = new \Types\SdkOptions();
    $sdkOptions->setEnvironment(Environment::SANDBOX);

    $config = new ConnectorConfig();
    $config->setConnectorConfig($connectorSpecificConfig);
    $config->setOptions($sdkOptions);

    return $config;
}

// ---------------------------------------------------------------------------
// Test execution
// ---------------------------------------------------------------------------

function testConnector(
    string $connectorName,
    array $authConfig,
    bool $dryRun = false
): array {
    $result = [
        'connector' => $connectorName,
        'status'    => 'pending',
        'error'     => null,
    ];

    try {
        $config = buildConnectorConfig($connectorName, $authConfig);
        $client = new PaymentClient($config);
        $req    = buildAuthorizeRequest();

        if ($dryRun) {
            $result['status'] = 'dry_run';
            return $result;
        }

        if (!hasValidCredentials($authConfig)) {
            $result['status'] = 'skipped';
            $result['reason'] = 'placeholder_credentials';
            return $result;
        }

        try {
            $response = $client->authorize($req);
            $result['status']     = 'passed';
            $result['httpStatus'] = $response->getStatus();
        } catch (RequestException $e) {
            $result['status'] = 'passed_with_error';
            $result['error']  = $e->getErrorMessage() ?: "status={$e->getStatus()}";
        } catch (ResponseException $e) {
            $result['status'] = 'passed_with_error';
            $result['error']  = $e->getErrorMessage() ?: "status={$e->getStatus()}";
        }
    } catch (\Throwable $e) {
        $result['status'] = 'failed';
        $result['error']  = $e->getMessage();
    }

    return $result;
}

// ---------------------------------------------------------------------------
// Argument parsing
// ---------------------------------------------------------------------------

function parseArgs(): array
{
    global $argv;
    $args       = array_slice($argv, 1);
    $credsFile  = 'creds.json';
    $connectors = null;
    $all        = false;
    $dryRun     = false;

    for ($i = 0; $i < count($args); $i++) {
        switch ($args[$i]) {
            case '--creds-file':
                $credsFile = $args[++$i] ?? 'creds.json';
                break;
            case '--connectors':
                $connectors = array_map('trim', explode(',', $args[++$i] ?? 'stripe'));
                break;
            case '--all':
                $all = true;
                break;
            case '--dry-run':
                $dryRun = true;
                break;
            case '--help':
            case '-h':
                echo <<<HELP
Usage: php -d ffi.enable=1 test_smoke.php [options]

Options:
  --creds-file <path>     Path to credentials JSON (default: creds.json)
  --connectors <list>     Comma-separated list of connectors to test
  --all                   Test all connectors in the credentials file
  --dry-run               Build requests without executing HTTP calls
  --help, -h              Show this help message

Examples:
  php -d ffi.enable=1 test_smoke.php --all
  php -d ffi.enable=1 test_smoke.php --connectors stripe,adyen
  php -d ffi.enable=1 test_smoke.php --all --dry-run

HELP;
                exit(0);
        }
    }

    if (!$all && $connectors === null) {
        fwrite(STDERR, "Error: Must specify either --all or --connectors\n");
        exit(1);
    }

    return compact('credsFile', 'connectors', 'all', 'dryRun');
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

function main(): void
{
    ['credsFile' => $credsFile, 'connectors' => $connectors, 'all' => $all, 'dryRun' => $dryRun]
        = parseArgs();

    $credentials     = loadCredentials($credsFile);
    $testConnectors  = $connectors ?? array_keys($credentials);
    $results         = [];

    $sep = str_repeat('=', 60);
    echo "\n{$sep}\n";
    echo 'Running PHP SDK smoke tests for ' . count($testConnectors) . " connector(s)\n";
    echo "{$sep}\n\n";

    foreach ($testConnectors as $name) {
        $authConfig = $credentials[$name] ?? null;
        echo "--- Testing {$name} ---\n";

        if ($authConfig === null) {
            echo "  SKIPPED (not found in credentials file)\n";
            $results[] = ['connector' => $name, 'status' => 'skipped', 'error' => 'not_found'];
            continue;
        }

        // Support multi-instance connectors (array of auth configs)
        $instances = is_array($authConfig) && isset($authConfig[0]) ? $authConfig : [$authConfig];

        foreach ($instances as $idx => $auth) {
            $instanceName = count($instances) > 1 ? "{$name}[" . ($idx + 1) . ']' : $name;

            if (!hasValidCredentials($auth)) {
                echo "  SKIPPED (placeholder credentials)\n";
                $results[] = ['connector' => $instanceName, 'status' => 'skipped'];
                continue;
            }

            $result = testConnector($name, $auth, $dryRun);
            $result['connector'] = $instanceName;
            $results[] = $result;

            match ($result['status']) {
                'passed'            => print("  ✓ PASSED\n"),
                'passed_with_error' => print("  ✓ PASSED (connector error: {$result['error']})\n"),
                'dry_run'           => print("  ✓ DRY RUN\n"),
                default             => print("  ✗ {$result['status']}: {$result['error']}\n"),
            };
        }
    }

    // Summary
    echo "\n{$sep}\nTEST SUMMARY\n{$sep}\n\n";

    $passed  = count(array_filter($results, fn($r) => in_array($r['status'], ['passed', 'passed_with_error', 'dry_run'], true)));
    $skipped = count(array_filter($results, fn($r) => $r['status'] === 'skipped'));
    $failed  = count(array_filter($results, fn($r) => $r['status'] === 'failed'));

    echo "Total:   " . count($results) . "\n";
    echo "Passed:  {$passed} ✓\n";
    echo "Skipped: {$skipped} (placeholder credentials)\n";
    echo "Failed:  {$failed} ✗\n\n";

    if ($failed > 0) {
        echo "Failed tests:\n";
        foreach ($results as $r) {
            if ($r['status'] === 'failed') {
                echo "  - {$r['connector']}: {$r['error']}\n";
            }
        }
        echo "\n";
        exit(1);
    }

    if ($passed === 0 && $skipped > 0) {
        echo "All tests skipped (no valid credentials). Update creds.json to run tests.\n";
        exit(1);
    }

    echo "All tests completed successfully!\n";
    exit(0);
}

main();
