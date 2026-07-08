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
    // Adyen SDK v6+ exposes AdyenCheckout and components on window.AdyenWeb
    let AdyenCheckout, Card;
    if (window.AdyenWeb) {
      AdyenCheckout = window.AdyenWeb.AdyenCheckout;
      Card = window.AdyenWeb.Card;
    } else if (typeof window.AdyenCheckout === 'function') {
      AdyenCheckout = window.AdyenCheckout;
    } else {
      throw new Error('Adyen Web SDK not loaded properly. Make sure the script tag is included and loaded in the HTML.');
    }

    if (!AdyenCheckout) {
      throw new Error('AdyenCheckout not found in SDK');
    }

    // Validate session data
    if (!session.id || !session.sessionData) {
      throw new Error(`Invalid session data: id=${!!session.id}, sessionData=${!!session.sessionData}`);
    }

    // Validate client key
    if (!clientKey) {
      throw new Error('Client key is required. Please check your ADYEN_CLIENT_KEY environment variable.');
    }

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
        if (paymentResolve) {
          paymentResolve({
            success: true,
            result: result
          });
        }
      },

      // REQUIRED: Called when payment fails
      onPaymentFailed: (result, component) => {
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
        // Don't reject on CANCEL - that's a user action
        if (paymentReject && error.name !== 'CANCEL') {
          paymentReject(new Error(error.message));
        }
      }
    };

    adyenCheckout = await AdyenCheckout(globalConfiguration);

    // Create and mount Card Component
    const cardContainer = document.getElementById('adyen-card-container');

    if (!cardContainer) {
      throw new Error('Card container (#adyen-card-container) not found in DOM');
    }

    // Create card component using v6.x API
    if (Card && typeof Card === 'function') {
      try {
        cardComponent = new Card(adyenCheckout, {
          hasHolderName: true,
          holderNameRequired: false,
          enableStoreDetails: false,
          hideCVC: false,
          brands: ['visa', 'mc', 'amex', 'discover']
        });

        cardComponent.mount(cardContainer);
      } catch (err) {
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

  await cardComponent.submit();
}

// Export functions globally for use in checkout.js
window.initAdyen = initAdyen;
window.waitForPaymentCompletion = waitForPaymentCompletion;
window.isAdyenInitialized = isAdyenInitialized;
window.getCardComponent = getCardComponent;
window.submitAdyenPayment = submitAdyenPayment;
