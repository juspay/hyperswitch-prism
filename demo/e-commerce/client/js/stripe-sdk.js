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
    
    stripe = Stripe(publishableKey);
    currentClientSecret = clientSecret;
    
    // Create Elements instance
    elements = stripe.elements({
      clientSecret: clientSecret,
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
    
    paymentElement.mount('#payment-element');
    
    // Handle real-time validation errors
    paymentElement.on('change', (event) => {
      const errorElement = document.getElementById('stripe-error');
      if (event.error) {
        errorElement.textContent = event.error.message;
      } else {
        errorElement.textContent = '';
      }
    });
    
    console.log('[Stripe] Payment Element mounted');
    return true;
  } catch (error) {
    console.error('[Stripe] Init error:', error);
    throw error;
  }
}

/**
 * Submit payment (no redirect - handles in-app)
 * @returns {Promise<{success: boolean, error?: string, paymentIntent?: object}>}
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
    
    // Confirm payment without redirect
    const { paymentIntent, error: confirmError } = await stripe.confirmPayment({
      elements,
      clientSecret: currentClientSecret,
      confirmParams: {
        return_url: window.location.href, // Required but won't redirect
      },
      redirect: 'if_required' // Only redirect if 3DS required
    });
    
    if (confirmError) {
      throw confirmError;
    }
    
    console.log('[Stripe] Payment successful:', paymentIntent);
    
    return {
      success: true,
      paymentIntent: {
        id: paymentIntent.id,
        status: paymentIntent.status,
        amount: paymentIntent.amount,
        currency: paymentIntent.currency
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