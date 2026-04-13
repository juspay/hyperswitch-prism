/**
 * GlobalPay SDK Handler
 * Uses GlobalPay embedded checkout elements (no redirect)
 * Reference: https://developer.globalpayments.com/docs/payments/online/hosted-payment-page-guide
 */

let globalPayInstance = null;
let cardForm = null;

/**
 * Initialize GlobalPay with access token
 * @param {string} accessToken - Access token from server
 * @param {string} publishableKey - Publishable key
 * @returns {Promise<boolean>}
 */
async function initGlobalPay(accessToken, publishableKey) {
  try {
    console.log('[GlobalPay] Initializing embedded checkout');
    
    // Check if GlobalPay is loaded
    if (typeof GlobalPay === 'undefined') {
      throw new Error('GlobalPay SDK not loaded');
    }
    
    // Configure GlobalPay
    globalPayInstance = GlobalPay.configure({
      accessToken: accessToken,
      env: 'sandbox', // Use 'production' for live
      apiVersion: '2021-03-22'
    });
    
    // Create card form with embedded styling
    cardForm = globalPayInstance.create('card-form', {
      target: document.getElementById('globalpay-payment-element'),
      style: {
        base: {
          color: '#30313d',
          fontFamily: '-apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif',
          fontSize: '16px',
          '::placeholder': {
            color: '#aab7c4'
          }
        },
        invalid: {
          color: '#df1b41'
        }
      }
    });
    
    // Mount the card form
    await cardForm.mount();
    
    // Handle validation changes
    cardForm.on('change', (event) => {
      const errorElement = document.getElementById('globalpay-error');
      if (event.error) {
        errorElement.textContent = event.error.message;
      } else {
        errorElement.textContent = '';
      }
    });
    
    console.log('[GlobalPay] Card form mounted');
    return true;
  } catch (error) {
    console.error('[GlobalPay] Init error:', error);
    throw error;
  }
}

/**
 * Submit GlobalPay payment
 * @param {Object} paymentData - Payment details
 * @returns {Promise<{success: boolean, error?: string, token?: string}>}
 */
async function submitGlobalPayPayment(paymentData) {
  if (!cardForm) {
    throw new Error('GlobalPay not initialized');
  }
  
  const submitBtn = document.getElementById('globalpay-submit-btn');
  const btnText = document.getElementById('gp-btn-text');
  const btnSpinner = document.getElementById('gp-btn-spinner');
  const errorElement = document.getElementById('globalpay-error');
  
  // Show loading
  submitBtn.disabled = true;
  btnText.textContent = 'Processing...';
  btnSpinner.classList.remove('hidden');
  errorElement.textContent = '';
  
  try {
    // Tokenize card
    const { token, error } = await cardForm.tokenize();
    
    if (error) {
      throw error;
    }
    
    console.log('[GlobalPay] Card tokenized:', token);
    
    // Return token for server-side processing
    return {
      success: true,
      token: token
    };
    
  } catch (error) {
    console.error('[GlobalPay] Payment error:', error);
    errorElement.textContent = error.message || 'Payment failed';
    submitBtn.disabled = false;
    btnText.textContent = 'Pay Now';
    btnSpinner.classList.add('hidden');
    return { success: false, error: error.message };
  }
}

/**
 * Alternative: Initialize with card element (single field)
 */
async function initGlobalPayCardElement(accessToken) {
  try {
    console.log('[GlobalPay] Initializing card element');
    
    globalPayInstance = GlobalPay.configure({
      accessToken: accessToken,
      env: 'sandbox'
    });
    
    const card = globalPayInstance.create('card', {
      style: {
        base: {
          color: '#30313d',
          fontFamily: '-apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif',
          fontSize: '16px'
        }
      }
    });
    
    await card.mount('#globalpay-payment-element');
    
    return card;
  } catch (error) {
    console.error('[GlobalPay] Card element error:', error);
    throw error;
  }
}

// Export globally
window.initGlobalPay = initGlobalPay;
window.initGlobalPayCardElement = initGlobalPayCardElement;
window.submitGlobalPayPayment = submitGlobalPayPayment;