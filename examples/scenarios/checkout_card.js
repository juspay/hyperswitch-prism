#!/usr/bin/env node
/**
 * Card Payment (Authorize + Capture) - Universal Example
 *
 * Works with any connector that supports card payments.
 * Usage: node checkout_card.js --connector=stripe
 */

const { PaymentClient } = require('hs-playlib');
const { ConnectorConfig, ConnectorSpecificConfig, SdkOptions, Environment } = require('hs-playlib').types;
const fs = require('fs');
const path = require('path');

// [START imports]
// Already imported above
// [END imports]

// [START load_probe_data]
function loadProbeData(connectorName) {
    const probePath = path.join(__dirname, '..', '..', 'data', 'field_probe', `${connectorName}.json`);
    return JSON.parse(fs.readFileSync(probePath, 'utf8'));
}
// [END load_probe_data]

// [START stripe_config]
function getStripeConfig(apiKey) {
    const config = ConnectorConfig.create({
        options: SdkOptions.create({ environment: Environment.SANDBOX }),
    });
    config.connectorConfig = ConnectorSpecificConfig.create({
        stripe: { apiKey: { value: apiKey } }
    });
    return config;
}
// [END stripe_config]

// [START adyen_config]
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
// [END adyen_config]

// [START checkout_config]
function getCheckoutConfig(apiKey) {
    const config = ConnectorConfig.create({
        options: SdkOptions.create({ environment: Environment.SANDBOX }),
    });
    config.connectorConfig = ConnectorSpecificConfig.create({
        checkout: { apiKey: { value: apiKey } }
    });
    return config;
}
// [END checkout_config]

// [START get_connector_config]
function getConnectorConfig(connectorName, credentials) {
    switch (connectorName) {
        case 'stripe':
            return getStripeConfig(credentials.apiKey);
        case 'adyen':
            return getAdyenConfig(credentials.apiKey, credentials.merchantAccount);
        case 'checkout':
            return getCheckoutConfig(credentials.apiKey);
        default:
            throw new Error(`Unknown connector: ${connectorName}`);
    }
}
// [END get_connector_config]

// [START build_authorize_request]
function buildAuthorizeRequest(probeData, captureMethod = 'MANUAL') {
    const flows = probeData.flows || {};
    const authorizeFlows = flows.authorize || {};
    
    // Find Card payment method or first supported
    let cardData = null;
    for (const [pmKey, pmData] of Object.entries(authorizeFlows)) {
        if (pmData.status === 'supported') {
            if (pmKey === 'Card') {
                cardData = pmData;
                break;
            } else if (!cardData) {
                cardData = pmData;
            }
        }
    }
    
    if (!cardData) {
        throw new Error('No supported payment method found for authorize flow');
    }
    
    const protoRequest = { ...cardData.protoRequest };
    protoRequest.captureMethod = captureMethod;
    
    return protoRequest;
}
// [END build_authorize_request]

// [START build_capture_request]
function buildCaptureRequest(connectorTransactionId, amount, merchantCaptureId = 'capture_001') {
    return {
        merchantCaptureId: merchantCaptureId,
        connectorTransactionId: connectorTransactionId,
        amountToCapture: amount,
    };
}
// [END build_capture_request]

// [START process_checkout_card]
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
// [END process_checkout_card]

// [START main]
async function main() {
    const args = process.argv.slice(2);
    const connectorArg = args.find(arg => arg.startsWith('--connector='));
    const credsArg = args.find(arg => arg.startsWith('--credentials='));
    
    if (!connectorArg) {
        console.error('Error: --connector is required');
        process.exit(1);
    }
    
    const connectorName = connectorArg.split('=')[1];
    
    // Load credentials
    let credentials;
    if (credsArg) {
        const credsPath = credsArg.split('=')[1];
        credentials = JSON.parse(fs.readFileSync(credsPath, 'utf8'));
    } else {
        credentials = { apiKey: 'sk_test_dummy' };
        console.log('⚠️  Using dummy credentials. Set --credentials for real API calls.');
    }
    
    try {
        // Run the flow
        const result = await processCheckoutCard(connectorName, credentials);
        
        console.log('\n' + '='.repeat(60));
        console.log(`Result: ${result}`);
        console.log('='.repeat(60));
        
        process.exit(result === 'success' || result === 'pending' ? 0 : 1);
    } catch (error) {
        console.error('Error:', error.message);
        process.exit(1);
    }
}

if (require.main === module) {
    main();
}
// [END main]

module.exports = { processCheckoutCard };
