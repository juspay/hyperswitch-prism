/**
 * Stripe SDK Handler
 * Uses Stripe Payment Element for embedded checkout (no redirect)
 */

let stripe = null;
let elements = null;
let currentClientSecret = null;

/**
 * Initialize Stripe with Payment Element
 * @param {string} publishableKey - Stripe publishable key
 * @param {string} clientSecret - Payment Intent client secret from server
 * @returns {Promise<boolean>}
 */
async function initStripe(publishableKey, clientSecret) {
  try {
    console.log('[Stripe] Initializing Payment Element');
    console.log('[Stripe] Publishable key:', publishableKey?.substring(0, 10) + '...');
    console.log('[Stripe] Client secret:', clientSecret?.substring(0, 20) + '...');
    
    if (!publishableKey || !clientSecret) {
      throw new Error('Missing publishable key or client secret');
    }
    
    stripe = Stripe(publishableKey);
    currentClientSecret = clientSecret;
    
    // Create Elements instance with manual payment method creation
    elements = stripe.elements({
      clientSecret: clientSecret,
      paymentMethodCreation: 'manual',
      appearance: {
        theme: 'stripe',
        variables: {
          colorPrimary: '#2563eb',
          colorBackground: '#ffffff',
          colorText: '#30313d',
          colorDanger: '#df1b41',
          fontFamily: '-apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif',
          borderRadius: '8px'
        }
      }
    });
    
    // Create and mount Payment Element
    const paymentElement = elements.create('payment', {
      layout: 'tabs'
    });
    
    // Check if container exists
    const container = document.getElementById('payment-element');
    if (!container) {
      throw new Error('Payment element container not found');
    }
    
    paymentElement.mount('#payment-element');
    
    // Handle when element is ready
    paymentElement.on('ready', () => {
      console.log('[Stripe] Payment Element ready');
    });
    
    // Handle load errors
    paymentElement.on('loaderror', (event) => {
      console.error('[Stripe] Element load error:', event);
    });
    
    // Handle real-time validation errors
    paymentElement.on('change', (event) => {
      const errorElement = document.getElementById('stripe-error');
      if (event.error) {
        errorElement.textContent = event.error.message;
      } else {
        errorElement.textContent = '';
      }
    });
    
    console.log('[Stripe] Payment Element mounted successfully');
    return true;
  } catch (error) {
    console.error('[Stripe] Init error:', error);
    throw error;
  }
}

/**
 * Submit payment - tokenize card and return payment method
 * @returns {Promise<{success: boolean, error?: string, paymentMethod?: object}>}
 */
async function submitStripePayment() {
  if (!stripe || !elements) {
    throw new Error('Stripe not initialized');
  }
  
  const submitBtn = document.getElementById('stripe-submit-btn');
  const btnText = document.getElementById('btn-text');
  const btnSpinner = document.getElementById('btn-spinner');
  const errorElement = document.getElementById('stripe-error');
  
  // Show loading
  submitBtn.disabled = true;
  btnText.textContent = 'Processing...';
  btnSpinner.classList.remove('hidden');
  errorElement.textContent = '';
  
  try {
    // Submit the form
    const { error: submitError } = await elements.submit();
    if (submitError) {
      throw submitError;
    }
    
    // Create payment method (tokenize card) - don't confirm yet
    const { paymentMethod, error: createError } = await stripe.createPaymentMethod({
      elements
    });
    
    if (createError) {
      throw createError;
    }
    
    console.log('[Stripe] Payment method created:', paymentMethod);
    
    // Don't reset button here - let authorizePayment handle it
    return {
      success: true,
      paymentMethod: {
        id: paymentMethod.id,
        type: paymentMethod.type
      }
    };
    
  } catch (error) {
    console.error('[Stripe] Payment error:', error);
    errorElement.textContent = error.message;
    submitBtn.disabled = false;
    btnText.textContent = 'Pay Now';
    btnSpinner.classList.add('hidden');
    return { success: false, error: error.message };
  }
}

// Export globally
window.initStripe = initStripe;
window.submitStripePayment = submitStripePayment;