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

// DOM Elements
const loadingEl = document.getElementById('loading');
const stripeContainer = document.getElementById('stripe-checkout-container');
const globalpayContainer = document.getElementById('globalpay-checkout-container');
const connectorInfo = document.getElementById('payment-connector-info');
const paymentResult = document.getElementById('payment-result');
const orderItemsEl = document.getElementById('order-items');
const orderTotalEl = document.getElementById('order-total-amount');

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
  
  // Use publishable key and client token from server
  await initStripe(sdkConfig.publishableKey, sdkConfig.clientToken);
}

/**
 * Initialize GlobalPay checkout
 * Uses auto-submit form (no button) - NON PCI compliant
 */
async function initGlobalPayCheckout() {
  globalpayContainer.classList.remove('hidden');

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
  
  // Hide payment forms
  stripeContainer.classList.add('hidden');
  globalpayContainer.classList.add('hidden');
  
  // Show result
  paymentResult.classList.remove('hidden');
  document.getElementById('result-icon').textContent = '✅';
  document.getElementById('result-title').textContent = 'Payment Successful!';
  document.getElementById('result-message').textContent = 'Your payment has been processed successfully.';
  document.getElementById('result-txn-id').textContent = `Transaction ID: ${transactionId}`;
  
  // Show refund button
  document.getElementById('refund-btn').classList.remove('hidden');
  
  // Store transaction ID for refund
  paymentResult.dataset.transactionId = transactionId;
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
 * Format price
 */
function formatPrice(amount, currency) {
  const value = amount / 100;
  const symbol = currency === 'EUR' ? '€' : '$';
  return `${symbol}${value.toFixed(2)}`;
}

// Clear checkout data on successful payment
window.addEventListener('beforeunload', () => {
  if (paymentResult.classList.contains('hidden')) {
    // Payment not completed, keep data
  } else {
    // Payment completed, clear data
    localStorage.removeItem('checkoutData');
  }
});