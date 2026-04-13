/**
 * Checkout Flow
 * Orchestrates payment SDK initialization and submission
 */

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
      await initStripeCheckout();
    } else if (sdkConfig.connector === 'globalpay') {
      await initGlobalPayCheckout();
    }
    
    // Hide loading, show form
    loadingEl.classList.add('hidden');
    
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
 */
async function initGlobalPayCheckout() {
  globalpayContainer.classList.remove('hidden');
  
  // Use access token from server
  await initGlobalPay(sdkConfig.clientToken, sdkConfig.publishableKey);
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
    // PaymentIntent already processed by Stripe
    // We need to authorize on our server too
    await authorizePayment(result.paymentIntent.id);
  }
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
    
    // PaymentStatus: 8 = CHARGED, 6 = AUTHORIZED
    if (result.status === 8 || result.status === 6) {
      showSuccess(result.connectorTransactionId);
    } else {
      showError(result.error || 'Payment failed');
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