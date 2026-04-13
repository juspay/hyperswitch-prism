/**
 * GlobalPay SDK Handler - NON PCI Compliant
 * Uses GlobalPay Credit Card Form (auto-submit)
 * Reference: /Users/jeeva.ramachandran/Downloads/archive/connector/globalpay.html
 */

let cardForm = null;

/**
 * Initialize GlobalPay Credit Card Form
 * @param {string} accessToken - Access token from server (with PMT_POST_Create_Single permission)
 * @returns {Promise<boolean>}
 */
async function initGlobalPay(accessToken) {
  try {
    console.log('[GlobalPay] Initializing Credit Card Form');

    // Check if GlobalPayments is loaded
    if (typeof GlobalPayments === 'undefined') {
      throw new Error('GlobalPayments SDK not loaded');
    }

    console.log('[GlobalPay] Using access token:', accessToken?.substring(0, 15) + '...');

    // Configure GlobalPayments
    GlobalPayments.configure({
      accessToken: accessToken,
      env: 'sandbox'
    });

    // Create the credit card form - auto-mounts to #credit-card
    // This creates hosted fields for card number, expiry, and CVV
    cardForm = GlobalPayments.creditCard.form('#credit-card');

    // Handle form ready
    cardForm.ready(() => {
      console.log('[GlobalPay] Credit Card Form ready');
    });

    console.log('[GlobalPay] Credit Card Form initialized');
    return true;
  } catch (error) {
    console.error('[GlobalPay] Init error:', error);
    throw error;
  }
}

/**
 * Setup GlobalPay event handlers
 * @param {Function} onTokenSuccess - Callback when token is created
 */
function setupGlobalPayHandlers(onTokenSuccess) {
  if (!cardForm) {
    throw new Error('GlobalPay not initialized');
  }

  const errorElement = document.getElementById('globalpay-error');

  // Handle token success - auto-submitted when user presses Enter or form validates
  cardForm.on('token-success', (resp) => {
    const token = resp.paymentReference;
    console.log('[GlobalPay] Token created:', token);
    
    if (token) {
      onTokenSuccess(token);
    } else {
      console.error('[GlobalPay] No paymentReference in response:', resp);
      if (errorElement) {
        errorElement.textContent = 'Token creation failed - no payment reference';
      }
    }
  });

  // Handle token error
  cardForm.on('token-error', (err) => {
    console.error('[GlobalPay] Tokenization failed:', err);
    
    let errorMsg = 'Payment tokenization failed';
    if (err.error_code === 'ACTION_NOT_AUTHORIZED' || err.detailed_error_code === '40022') {
      errorMsg = 'Access token lacks tokenization permissions. Contact GlobalPay support to enable PMT_POST_Create permission.';
    } else if (err.reason || err.detailed_error_description) {
      errorMsg = err.reason || err.detailed_error_description;
    }
    
    if (errorElement) {
      errorElement.textContent = errorMsg;
    }
  });

  // Global error handler
  GlobalPayments.on('error', (err) => {
    console.error('[GlobalPay] SDK error:', err);
    if (errorElement) {
      errorElement.textContent = err.message || 'An error occurred';
    }
  });
}

// Export globally
window.initGlobalPay = initGlobalPay;
window.setupGlobalPayHandlers = setupGlobalPayHandlers;
