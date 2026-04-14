/**
 * Checkout Flow
 * Orchestrates payment SDK initialization and submission
 */

// PaymentStatus enum from types (mirrored from SDK)
const PaymentStatus = {
  PAYMENT_STATUS_UNSPECIFIED: 0,
  STARTED: 1,
  PAYMENT_METHOD_AWAITED: 22,
  DEVICE_DATA_COLLECTION_PENDING: 24,
  CONFIRMATION_AWAITED: 23,
  AUTHENTICATION_PENDING: 4,
  AUTHENTICATION_SUCCESSFUL: 5,
  AUTHENTICATION_FAILED: 2,
  AUTHORIZING: 9,
  AUTHORIZED: 6,
  AUTHORIZATION_FAILED: 7,
  PARTIALLY_AUTHORIZED: 25,
  CHARGED: 8,
  PARTIAL_CHARGED: 17,
  PARTIAL_CHARGED_AND_CHARGEABLE: 18,
  AUTO_REFUNDED: 16,
  CAPTURE_INITIATED: 13,
  CAPTURE_FAILED: 14,
  VOID_INITIATED: 12,
  VOIDED: 11,
  VOID_FAILED: 15,
  VOIDED_POST_CAPTURE: 57,
  COD_INITIATED: 10,
  EXPIRED: 26,
  ROUTER_DECLINED: 3,
  PENDING: 20,
  FAILURE: 21,
  UNRESOLVED: 19
};

// RefundStatus enum from types
const RefundStatus = {
  REFUND_STATUS_UNSPECIFIED: 0,
  PENDING: 1,
  COMPLETED: 4,
  FAILED: 3,
  MANUAL_REVIEW: 2
};

// State
let checkoutData = null;
let sdkConfig = null;
let currentConnector = null;

// Test card configurations for each connector
const TEST_CARDS = {
  adyen: {
    number: '4111 1111 4555 1142',
    expiry: '03/2030',
    cvv: '737'
  },
  globalpay: {
    number: '4263 9700 0000 5262',
    expiry: '03/2030',
    cvv: '737'
  },
  stripe: {
    number: '4242 4242 4242 4242',
    expiry: '03/2030',
    cvv: '737'
  }
};

// DOM Elements
const loadingEl = document.getElementById('loading');
const stripeContainer = document.getElementById('stripe-checkout-container');
const globalpayContainer = document.getElementById('globalpay-checkout-container');
const adyenContainer = document.getElementById('adyen-checkout-container');
const connectorInfo = document.getElementById('payment-connector-info');
const paymentResult = document.getElementById('payment-result');
const orderItemsEl = document.getElementById('order-items');
const orderTotalEl = document.getElementById('order-total-amount');
const testCardInfo = document.getElementById('test-card-info');
const testCardNumberEl = document.getElementById('test-card-number');

// Initialize on page load
document.addEventListener('DOMContentLoaded', async () => {
  loadCheckoutData();
  renderOrderSummary();
  await initializePayment();
  setupEventListeners();
});

/**
 * Load checkout data from localStorage
 */
function loadCheckoutData() {
  const data = localStorage.getItem('checkoutData');
  if (!data) {
    window.location.href = '/';
    return;
  }
  checkoutData = JSON.parse(data);
  console.log('[Checkout] Data loaded:', checkoutData);
}

/**
 * Render order summary
 */
function renderOrderSummary() {
  if (!checkoutData) return;
  
  const { items, currency, totalAmount } = checkoutData;
  
  // Render items
  orderItemsEl.innerHTML = items.map(item => {
    const price = currency === 'EUR' ? item.product.priceEUR : item.product.priceUSD;
    return `
      <div class="order-item">
        <div class="order-item-image">${item.product.image}</div>
        <div class="order-item-info">
          <div class="order-item-name">${item.product.name}</div>
          <div class="order-item-qty">Qty: ${item.quantity}</div>
        </div>
        <div class="order-item-price">${formatPrice(price * item.quantity, currency)}</div>
      </div>
    `;
  }).join('');
  
  // Set total
  orderTotalEl.textContent = formatPrice(totalAmount, currency);
}

/**
 * Initialize payment SDK based on currency
 */
async function initializePayment() {
  if (!checkoutData) return;
  
  const { currency, totalAmount } = checkoutData;
  
  try {
    // Step 1: Get SDK session from server
    const response = await fetch(`/api/auth/sdk-session?currency=${currency}&amount=${totalAmount}`);
    
    if (!response.ok) {
      throw new Error('Failed to initialize payment session');
    }
    
    sdkConfig = await response.json();
    currentConnector = sdkConfig.connector;
    
    console.log('[Checkout] SDK config received:', sdkConfig);
    
    // Show connector info
    connectorInfo.innerHTML = `
      <strong>Payment Processor:</strong> ${sdkConfig.connector.toUpperCase()}<br>
      <small>Currency: ${currency} | Amount: ${formatPrice(totalAmount, currency)}</small>
    `;
    
    // Step 2: Initialize appropriate SDK
    if (sdkConfig.connector === 'stripe') {
      console.log('[Checkout] Initializing Stripe checkout...');
      await initStripeCheckout();
      console.log('[Checkout] Stripe checkout initialized');
    } else if (sdkConfig.connector === 'globalpay') {
      console.log('[Checkout] Initializing GlobalPay checkout...');
      await initGlobalPayCheckout();
      console.log('[Checkout] GlobalPay checkout initialized');
    } else if (sdkConfig.connector === 'adyen') {
      console.log('[Checkout] Initializing Adyen checkout...');
      await initAdyenCheckout();
      // Adyen checkout handles its own completion, no need to hide loading here
      console.log('[Checkout] Adyen checkout flow completed');
      return; // Adyen handles completion via callbacks
    }
    
    // Hide loading, show form
    console.log('[Checkout] Hiding loading indicator');
    loadingEl.classList.add('hidden');
    console.log('[Checkout] Loading hidden, form should be visible');
    
  } catch (error) {
    console.error('[Checkout] Init error:', error);
    showError('Failed to initialize payment. Please try again.');
  }
}

/**
 * Initialize Stripe checkout
 */
async function initStripeCheckout() {
  stripeContainer.classList.remove('hidden');

  // Show test card info for Stripe
  showTestCardInfo('stripe');

  // Use publishable key and client token from server
  await initStripe(sdkConfig.publishableKey, sdkConfig.clientToken);
}

/**
 * Initialize GlobalPay checkout
 * Uses auto-submit form (no button) - NON PCI compliant
 */
async function initGlobalPayCheckout() {
  globalpayContainer.classList.remove('hidden');

  // Show test card info for GlobalPay
  showTestCardInfo('globalpay');

  // Use the clientToken directly (it's the access token with PMT_POST_Create_Single permission)
  const accessToken = sdkConfig.clientToken;

  // Initialize GlobalPay with the access token
  await initGlobalPay(accessToken);

  // Setup handlers - form auto-submits when user presses Enter
  setupGlobalPayHandlers(async (token) => {
    console.log('[Checkout] GlobalPay token received, authorizing...');
    await authorizePayment(token);
  });
}

/**
 * Initialize Adyen checkout
 * Uses Adyen Web Components with Sessions Flow (PCI compliant)
 * Sessions Flow handles complete payment client-side, no server authorization needed
 */
async function initAdyenCheckout() {
  adyenContainer.classList.remove('hidden');

  try {
    // Extract Adyen session data from server response
    // Server returns sessionData in the format:
    // {
    //   connector: 'adyen',
    //   clientToken: '<session_id>',
    //   sessionData: { sessionData: '<session_data>', connectorSpecific: { adyen: {...} } },
    //   publishableKey: '<client_key>'
    // }

    const session = {
      id: sdkConfig.clientToken,
      sessionData: sdkConfig.sessionData?.sessionData || ''
    };

    // Show test card info for Adyen
    showTestCardInfo('adyen');

    console.log('[Checkout] Adyen session prepared:', {
      id: session.id?.substring(0, 20) + '...',
      sessionDataLength: session.sessionData?.length
    });

    // Derive country code from currency
    const countryCode = checkoutData.currency === 'EUR' ? 'NL' : 'US';

    // Initialize Adyen checkout
    await initAdyen(
      session,
      sdkConfig.publishableKey,  // Adyen clientKey
      {
        amount: checkoutData.totalAmount,
        currency: checkoutData.currency,
        countryCode: countryCode,
        locale: 'en-US'
      }
    );

    // Hide loading - Adyen component is now visible with its own Pay button
    loadingEl.classList.add('hidden');

    // Wait for payment completion (handled by Adyen component via callbacks)
    console.log('[Checkout] Waiting for Adyen payment completion...');
    const result = await waitForPaymentCompletion();

    console.log('[Checkout] Adyen payment result:', result);

    // Handle result
    if (result.success) {
      // Payment authorized successfully
      const transactionId = result.result?.pspReference || result.result?.merchantReference || 'adyen-payment';
      showSuccess(transactionId);
    } else {
      // Payment failed
      const errorMsg = result.error || 'Payment failed';
      showError(errorMsg);
    }

  } catch (error) {
    console.error('[Checkout] Adyen initialization error:', error);
    showError('Failed to initialize Adyen checkout: ' + error.message);
  }
}

/**
 * Setup event listeners
 */
function setupEventListeners() {
  // Stripe submit
  const stripeBtn = document.getElementById('stripe-submit-btn');
  if (stripeBtn) {
    stripeBtn.addEventListener('click', handleStripeSubmit);
  }

  // GlobalPay submit
  const globalpayBtn = document.getElementById('globalpay-submit-btn');
  if (globalpayBtn) {
    globalpayBtn.addEventListener('click', handleGlobalPaySubmit);
  }

  // Refund button
  const refundBtn = document.getElementById('refund-btn');
  if (refundBtn) {
    refundBtn.addEventListener('click', handleRefund);
  }

  // Copy test card field buttons
  document.querySelectorAll('.copy-field-btn').forEach(btn => {
    btn.addEventListener('click', (e) => {
      const field = e.currentTarget.dataset.field;
      copyTestCardField(field);
    });
  });
}

/**
 * Handle Stripe payment submission
 */
async function handleStripeSubmit(e) {
  e.preventDefault();
  
  const result = await submitStripePayment();
  
  if (result.success) {
    // Send payment method token to server for authorization
    await authorizePayment(result.paymentMethod.id);
  }
  // Note: Button state is handled by authorizePayment -> showSuccess/showError
}

/**
 * Handle GlobalPay payment submission
 */
async function handleGlobalPaySubmit(e) {
  e.preventDefault();
  
  const result = await submitGlobalPayPayment();
  
  if (result.success) {
    // Tokenize card, then authorize with token
    await authorizePayment(result.token);
  }
}

/**
 * Authorize payment on server
 */
async function authorizePayment(token) {
  try {
    const response = await fetch('/api/payments/token-authorize', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        token: token,
        merchantTransactionId: sdkConfig.merchantTransactionId,
        amount: checkoutData.totalAmount,
        currency: checkoutData.currency
      })
    });
    
    const result = await response.json();
    
    // Check if payment succeeded using statusText from server
    const successStatuses = ['CHARGED', 'AUTHORIZED'];
    if (successStatuses.includes(result.statusText)) {
      showSuccess(result.connectorTransactionId);
    } else {
      showError(result.error || `Payment ${result.statusText || 'failed'}`);
    }
    
  } catch (error) {
    console.error('[Checkout] Authorize error:', error);
    showError('Payment authorization failed');
  }
}

/**
 * Show success result
 */
function showSuccess(transactionId) {
  // Reset button state
  const submitBtn = document.getElementById('stripe-submit-btn');
  const btnText = document.getElementById('btn-text');
  const btnSpinner = document.getElementById('btn-spinner');
  if (submitBtn) {
    submitBtn.disabled = false;
    btnText.textContent = 'Pay Now';
    btnSpinner.classList.add('hidden');
  }

  // Hide payment forms and test card
  stripeContainer.classList.add('hidden');
  globalpayContainer.classList.add('hidden');
  adyenContainer.classList.add('hidden');
  testCardInfo.classList.add('hidden');

  // Show result
  paymentResult.classList.remove('hidden');
  document.getElementById('result-icon').textContent = '✅';
  document.getElementById('result-title').textContent = 'Payment Successful!';
  document.getElementById('result-message').textContent = 'Your payment has been processed successfully.';
  document.getElementById('result-txn-id').textContent = `Transaction ID: ${transactionId}`;

  // Clear cart - remove only the items that were purchased (matching the checkout currency)
  clearPurchasedItemsFromCart();
}

/**
 * Clear purchased items from cart after successful payment
 */
function clearPurchasedItemsFromCart() {
  if (!checkoutData) return;

  // Get current cart
  const savedCart = localStorage.getItem('cart');
  if (savedCart) {
    let cart = JSON.parse(savedCart);
    // Remove only items with the same currency as the checkout
    cart = cart.filter(item => item.currency !== checkoutData.currency);

    if (cart.length === 0) {
      // Cart is empty, remove completely
      localStorage.removeItem('cart');
    } else {
      // Save remaining items
      localStorage.setItem('cart', JSON.stringify(cart));
    }
  }

  // Clear checkout data
  localStorage.removeItem('checkoutData');
}

/**
 * Show error result
 */
function showError(message) {
  // Hide loading
  loadingEl.classList.add('hidden');
  
  // Show result
  paymentResult.classList.remove('hidden');
  document.getElementById('result-icon').textContent = '❌';
  document.getElementById('result-title').textContent = 'Payment Failed';
  document.getElementById('result-message').textContent = message;
  document.getElementById('result-txn-id').textContent = '';
}

/**
 * Handle refund
 */
async function handleRefund() {
  const transactionId = paymentResult.dataset.transactionId;
  const refundBtn = document.getElementById('refund-btn');
  
  refundBtn.disabled = true;
  refundBtn.textContent = 'Processing...';
  
  try {
    const response = await fetch('/api/payments/refund', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        connectorTransactionId: transactionId,
        refundAmount: checkoutData.totalAmount,
        currency: checkoutData.currency,
        merchantRefundId: `ref_${Date.now()}`
      })
    });
    
    const result = await response.json();
    
    // RefundStatus: 4 = SUCCESS, 3 = PENDING
    if (result.status === 4 || result.status === 3) {
      document.getElementById('result-icon').textContent = '💸';
      document.getElementById('result-title').textContent = 'Refund Successful!';
      document.getElementById('result-message').textContent = 'Your refund has been processed.';
      refundBtn.classList.add('hidden');
    } else {
      throw new Error(result.error || 'Refund failed');
    }
    
  } catch (error) {
    console.error('[Checkout] Refund error:', error);
    alert('Refund failed: ' + error.message);
    refundBtn.disabled = false;
    refundBtn.textContent = 'Process Refund';
  }
}

/**
 * Show test card info for the current connector
 */
function showTestCardInfo(connector) {
  const testCard = TEST_CARDS[connector];
  if (!testCard) return;

  document.getElementById('test-card-number').textContent = testCard.number;
  document.getElementById('test-card-expiry').textContent = testCard.expiry;
  document.getElementById('test-card-cvv').textContent = testCard.cvv;
  testCardInfo.classList.remove('hidden');
}

/**
 * Copy a specific test card field to clipboard
 */
async function copyTestCardField(field) {
  let value = '';
  let btn = null;

  switch (field) {
    case 'number':
      value = document.getElementById('test-card-number').textContent.replace(/\s/g, '');
      btn = document.querySelector('.copy-field-btn[data-field="number"]');
      break;
    case 'expiry':
      value = document.getElementById('test-card-expiry').textContent;
      btn = document.querySelector('.copy-field-btn[data-field="expiry"]');
      break;
    case 'cvv':
      value = document.getElementById('test-card-cvv').textContent;
      btn = document.querySelector('.copy-field-btn[data-field="cvv"]');
      break;
  }

  if (!value || !btn) return;

  try {
    await navigator.clipboard.writeText(value);
    const originalText = btn.innerHTML;
    btn.innerHTML = '✓';
    btn.classList.add('copied');

    setTimeout(() => {
      btn.innerHTML = originalText;
      btn.classList.remove('copied');
    }, 1500);
  } catch (err) {
    console.error('Failed to copy:', err);
  }
}

/**
 * Format price
 */
function formatPrice(amount, currency) {
  const value = amount / 100;
  const symbol = currency === 'EUR' ? '€' : '$';
  return `${symbol}${value.toFixed(2)}`;
}

// Note: Cart clearing is now handled in showSuccess() function after successful payment