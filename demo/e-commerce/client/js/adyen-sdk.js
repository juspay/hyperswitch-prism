/**
 * Adyen SDK Handler
 * Uses Adyen Web Components v6.31.1 with Sessions Flow
 * Reference: https://docs.adyen.com/online-payments/build-your-integration
 *
 * Sessions Flow handles the complete payment client-side:
 * - No manual tokenization needed
 * - No server-side authorization call needed
 * - Payment is authorized within the Adyen component
 */

let adyenCheckout = null;
let cardComponent = null;
let paymentResolve = null;
let paymentReject = null;

/**
 * Initialize Adyen Checkout with Sessions Flow
 *
 * @param {Object} session - Session object from server { id, sessionData }
 * @param {string} session.id - Session ID from /sessions call
 * @param {string} session.sessionData - Session data from /sessions call
 * @param {string} clientKey - Adyen client key for client-side authentication
 * @param {Object} config - Payment configuration
 * @param {number} config.amount - Amount in minor units (e.g., 1000 for $10.00)
 * @param {string} config.currency - Currency code (e.g., 'USD', 'EUR')
 * @param {string} config.countryCode - Country code (e.g., 'US', 'NL')
 * @param {string} config.locale - Locale for UI (e.g., 'en-US')
 * @returns {Promise<void>}
 */
async function initAdyen(session, clientKey, config) {
  try {
    console.log('[Adyen] Initializing checkout with Sessions Flow');

    // Verify Adyen Web SDK is loaded
    console.log('[Adyen] Checking SDK availability...');
    console.log('[Adyen] window.AdyenCheckout:', typeof window.AdyenCheckout);
    console.log('[Adyen] window.AdyenWeb:', typeof window.AdyenWeb);
    console.log('[Adyen] window.adyen:', typeof window.adyen);
    
    // Adyen SDK v6+ exposes AdyenCheckout and components on window.AdyenWeb
    let AdyenCheckout, Card;
    if (window.AdyenWeb) {
      console.log('[Adyen] Using window.AdyenWeb');
      AdyenCheckout = window.AdyenWeb.AdyenCheckout;
      Card = window.AdyenWeb.Card;
    } else if (typeof window.AdyenCheckout === 'function') {
      console.log('[Adyen] Using window.AdyenCheckout');
      AdyenCheckout = window.AdyenCheckout;
    } else {
      throw new Error('Adyen Web SDK not loaded properly. Make sure the script tag is included and loaded in the HTML.');
    }
    
    if (!AdyenCheckout) {
      throw new Error('AdyenCheckout not found in SDK');
    }
    
    console.log('[Adyen] Card component on window.AdyenWeb:', !!Card);
    console.log('[Adyen] window.AdyenWeb keys:', Object.keys(window.AdyenWeb));

    // Validate session data
    if (!session.id || !session.sessionData) {
      throw new Error(`Invalid session data: id=${!!session.id}, sessionData=${!!session.sessionData}`);
    }

    // Validate client key - must start with "test_" for test environment
    // and should only contain alphanumeric characters and underscores
    if (!clientKey) {
      throw new Error('Client key is required. Please check your ADYEN_CLIENT_KEY environment variable.');
    }

    // Check for invalid characters in client key
    const invalidCharsPattern = /[^a-zA-Z0-9_\-]/;
    const keyWithoutPrefix = clientKey.replace(/^test_/, '');
    if (invalidCharsPattern.test(keyWithoutPrefix)) {
      console.error('[Adyen] Invalid characters detected in client key (excluding prefix)');
      console.error('[Adyen] Client key format should be: test_<alphanumeric_characters>');
      throw new Error('Invalid client key format. Client key contains invalid characters. ' +
        'Please verify your ADYEN_CLIENT_KEY in the environment configuration.');
    }

    if (!clientKey.startsWith('test_') && !clientKey.startsWith('live_')) {
      console.warn('[Adyen] WARNING: Client key should start with "test_" or "live_"');
    }

    // Debug: Log session data (safely)
    console.log('[Adyen] Session ID:', session.id?.substring(0, 20) + '...');
    console.log('[Adyen] Session Data length:', session.sessionData?.length);
    console.log('[Adyen] Client Key prefix:', clientKey?.substring(0, 10) + '...');
    console.log('[Adyen] Amount:', config.amount, config.currency);

    // Build global configuration object as per Adyen docs
    const globalConfiguration = {
      // Session configuration - REQUIRED
      session: {
        id: session.id,
        sessionData: session.sessionData
      },

      // Client key for client-side authentication - REQUIRED
      clientKey: clientKey,

      // Environment - REQUIRED (use 'test' for sandbox, 'live' for production)
      environment: 'test',

      // Amount for Pay Button display - REQUIRED
      amount: {
        value: config.amount,
        currency: config.currency
      },

      // Country code for filtering payment methods - REQUIRED
      countryCode: config.countryCode,

      // Locale for UI language - REQUIRED
      locale: config.locale || 'en-US',

      // Show pay button in component (default: true)
      showPayButton: true,

      // REQUIRED: Called when payment is successfully completed
      onPaymentCompleted: (result, component) => {
        console.log('[Adyen] Payment completed:', result);
        console.log('[Adyen] Result code:', result.resultCode);

        if (paymentResolve) {
          paymentResolve({
            success: true,
            result: result,
            // result.resultCode values:
            // - 'Authorised': Payment was successfully authorized
            // - 'Pending': Payment is pending (additional action may be needed)
            // - 'Received': Payment was received (async methods like bank transfer)
          });
        }
      },

      // REQUIRED: Called when payment fails
      onPaymentFailed: (result, component) => {
        console.error('[Adyen] Payment failed:', result);
        console.error('[Adyen] Result code:', result.resultCode);

        // result.resultCode values for failures:
        // - 'Cancelled': Shopper cancelled the payment
        // - 'Error': An error occurred during payment processing
        // - 'Refused': Payment was refused by issuer

        if (paymentResolve) {
          paymentResolve({
            success: false,
            error: `Payment ${result.resultCode || 'failed'}`,
            result: result
          });
        }
      },

      // OPTIONAL: Called when an error occurs in the component
      onError: (error, component) => {
        console.error('[Adyen] SDK Error:', error);
        console.error('[Adyen] Error name:', error.name);
        console.error('[Adyen] Error message:', error.message);

        // error.name can be:
        // - 'NETWORK_ERROR': Call to server failed (timeout, missing info)
        // - 'CANCEL': Shopper cancelled (Apple Pay, PayPal)
        // - 'IMPLEMENTATION_ERROR': Incorrect method/parameter
        // - 'ERROR': Generic catch-all error

        // Don't reject on CANCEL - that's a user action
        if (paymentReject && error.name !== 'CANCEL') {
          paymentReject(new Error(error.message));
        }
      },

      // OPTIONAL: Called when form state changes (validation)
      onChange: (state, component) => {
        // state.isValid indicates if the form is valid
        // Can be used to enable/disable custom submit buttons
        console.log('[Adyen] Form state changed - Valid:', state.isValid);
      },

      // OPTIONAL: Called when an action (3DS, QR, etc.) is shown
      onActionHandled: (data) => {
        console.log('[Adyen] Action handled:', data);
        // data.actionType: 'threeDS', 'qr', 'await'
        // data.componentType: Type of component showing the action
        // data.actionDescription: Description of the action
      }
    };

    // Create AdyenCheckout instance
    console.log('[Adyen] Creating checkout instance...');
    console.log('[Adyen] AdyenCheckout type:', typeof AdyenCheckout);
    console.log('[Adyen] AdyenCheckout is function:', typeof AdyenCheckout === 'function');
    
    adyenCheckout = await AdyenCheckout(globalConfiguration);
    
    console.log('[Adyen] Checkout instance created:', !!adyenCheckout);
    console.log('[Adyen] Checkout instance type:', typeof adyenCheckout);
    console.log('[Adyen] Checkout instance keys:', adyenCheckout ? Object.keys(adyenCheckout) : 'null');
    console.log('[Adyen] Has create method:', adyenCheckout && typeof adyenCheckout.create === 'function');

    // Create and mount Card Component
    // The card component includes: card number, expiry date, cvc fields
    console.log('[Adyen] Mounting card component...');
    const cardContainer = document.getElementById('adyen-card-container');

    if (!cardContainer) {
      throw new Error('Card container (#adyen-card-container) not found in DOM');
    }

    // Create card component using v6.x API
    console.log('[Adyen] Creating card component...');
    console.log('[Adyen] Card variable:', !!Card);
    console.log('[Adyen] createComponent:', typeof window.AdyenWeb.createComponent);
    
    // Adyen v6: Card constructor signature is Card(checkoutInstance, props)
    // or use createComponent helper
    if (Card && typeof Card === 'function') {
      console.log('[Adyen] Using Card class with checkout instance');
      
      try {
        // Try: Card(checkout, config).mount(container)
        cardComponent = new Card(adyenCheckout, {
          hasHolderName: true,
          holderNameRequired: false,
          enableStoreDetails: false,
          hideCVC: false,
          brands: ['visa', 'mc', 'amex', 'discover']
        });
        
        cardComponent.mount(cardContainer);
      } catch (err) {
        console.log('[Adyen] First attempt failed:', err.message);
        console.log('[Adyen] Trying createComponent...');
        
        // Fallback: use createComponent helper
        const createComponent = window.AdyenWeb.createComponent;
        cardComponent = createComponent('card', adyenCheckout, {
          hasHolderName: true,
          holderNameRequired: false,
          enableStoreDetails: false,
          hideCVC: false,
          brands: ['visa', 'mc', 'amex', 'discover']
        });
        cardComponent.mount(cardContainer);
      }
    } else {
      throw new Error('Unable to create Card component: Card class not available');
    }

    console.log('[Adyen] Checkout initialized and card component mounted successfully');

  } catch (error) {
    console.error('[Adyen] Initialization error:', error);
    throw error;
  }
}

/**
 * Wait for payment completion
 *
 * Sessions Flow handles the complete payment authorization client-side.
 * This function returns a promise that resolves when the payment completes
 * (either successfully or with failure).
 *
 * @returns {Promise<{success: boolean, result?: object, error?: string}>}
 */
function waitForPaymentCompletion() {
  return new Promise((resolve, reject) => {
    paymentResolve = resolve;
    paymentReject = reject;
  });
}

/**
 * Check if Adyen checkout is initialized
 * @returns {boolean}
 */
function isAdyenInitialized() {
  return adyenCheckout !== null && cardComponent !== null;
}

/**
 * Get the card component instance (for advanced operations)
 * @returns {object|null}
 */
function getCardComponent() {
  return cardComponent;
}

/**
 * Submit payment programmatically (if showPayButton is false)
 * Note: Not needed if showPayButton is true (default)
 *
 * @returns {Promise<void>}
 */
async function submitAdyenPayment() {
  if (!cardComponent) {
    throw new Error('Card component not initialized');
  }

  console.log('[Adyen] Submitting payment programmatically...');
  await cardComponent.submit();
}

// Export functions globally for use in checkout.js
window.initAdyen = initAdyen;
window.waitForPaymentCompletion = waitForPaymentCompletion;
window.isAdyenInitialized = isAdyenInitialized;
window.getCardComponent = getCardComponent;
window.submitAdyenPayment = submitAdyenPayment;
