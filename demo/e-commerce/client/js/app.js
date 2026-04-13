// Product catalog
const products = [
  {
    id: '1',
    name: 'Wireless Headphones',
    description: 'Premium noise-canceling Bluetooth headphones',
    priceUSD: 1999,
    priceEUR: 1849,
    image: '🎧'
  },
  {
    id: '2',
    name: 'Smart Watch',
    description: 'Fitness tracking with heart rate monitor',
    priceUSD: 2999,
    priceEUR: 2799,
    image: '⌚'
  },
  {
    id: '3',
    name: 'Laptop Stand',
    description: 'Ergonomic aluminum laptop stand',
    priceUSD: 499,
    priceEUR: 459,
    image: '💻'
  },
  {
    id: '4',
    name: 'Wireless Charger',
    description: 'Fast wireless charging pad',
    priceUSD: 399,
    priceEUR: 369,
    image: '🔋'
  },
  {
    id: '5',
    name: 'USB-C Hub',
    description: '7-in-1 multiport adapter',
    priceUSD: 599,
    priceEUR: 549,
    image: '🔌'
  },
  {
    id: '6',
    name: 'Mechanical Keyboard',
    description: 'RGB backlit gaming keyboard',
    priceUSD: 899,
    priceEUR: 829,
    image: '⌨️'
  }
];

// State
let cart = [];
let currency = 'USD';

// DOM Elements
const productsGrid = document.getElementById('products-grid');
const currencySelector = document.getElementById('currency-selector');
const cartBtn = document.getElementById('cart-btn');
const cartCount = document.getElementById('cart-count');
const cartSidebar = document.getElementById('cart-sidebar');
const cartOverlay = document.getElementById('cart-overlay');
const closeCartBtn = document.getElementById('close-cart');
const cartItems = document.getElementById('cart-items');
const cartTotalAmount = document.getElementById('cart-total-amount');
const checkoutBtn = document.getElementById('checkout-btn');

// Initialize
document.addEventListener('DOMContentLoaded', () => {
  loadCartFromStorage();
  renderProducts();
  renderCart();
  setupEventListeners();
});

// Event Listeners
function setupEventListeners() {
  currencySelector.addEventListener('change', (e) => {
    currency = e.target.value;
    renderProducts();
    renderCart();
    saveCartToStorage();
  });

  cartBtn.addEventListener('click', openCart);
  closeCartBtn.addEventListener('click', closeCart);
  cartOverlay.addEventListener('click', closeCart);
  checkoutBtn.addEventListener('click', goToCheckout);
}

// Product Rendering
function renderProducts() {
  productsGrid.innerHTML = products.map(product => {
    const price = currency === 'EUR' ? product.priceEUR : product.priceUSD;
    const formattedPrice = formatPrice(price, currency);
    
    return `
      <div class="product-card">
        <div class="product-image">${product.image}</div>
        <div class="product-info">
          <h3 class="product-name">${product.name}</h3>
          <p class="product-description">${product.description}</p>
          <p class="product-price">${formattedPrice}</p>
          <button class="add-to-cart-btn" onclick="addToCart('${product.id}')">
            Add to Cart
          </button>
        </div>
      </div>
    `;
  }).join('');
}

// Cart Functions
function addToCart(productId) {
  const product = products.find(p => p.id === productId);
  if (!product) return;

  const existingItem = cart.find(item => 
    item.product.id === productId && item.currency === currency
  );

  if (existingItem) {
    existingItem.quantity++;
  } else {
    cart.push({
      product,
      quantity: 1,
      currency
    });
  }

  saveCartToStorage();
  renderCart();
  
  // Visual feedback
  const btn = event.target;
  btn.textContent = 'Added!';
  btn.classList.add('added');
  setTimeout(() => {
    btn.textContent = 'Add to Cart';
    btn.classList.remove('added');
  }, 1000);
}

function removeFromCart(productId) {
  cart = cart.filter(item => !(item.product.id === productId && item.currency === currency));
  saveCartToStorage();
  renderCart();
}

function updateQuantity(productId, delta) {
  const item = cart.find(item => 
    item.product.id === productId && item.currency === currency
  );
  
  if (item) {
    item.quantity += delta;
    if (item.quantity <= 0) {
      removeFromCart(productId);
    } else {
      saveCartToStorage();
      renderCart();
    }
  }
}

function renderCart() {
  const cartForCurrency = cart.filter(item => item.currency === currency);
  
  if (cartForCurrency.length === 0) {
    cartItems.innerHTML = `
      <div class="empty-cart">
        <div class="empty-cart-icon">🛒</div>
        <p>Your cart is empty</p>
      </div>
    `;
    cartTotalAmount.textContent = formatPrice(0, currency);
    checkoutBtn.disabled = true;
    cartCount.textContent = '0';
    return;
  }

  const totalItems = cartForCurrency.reduce((sum, item) => sum + item.quantity, 0);
  const totalAmount = cartForCurrency.reduce((sum, item) => {
    const price = currency === 'EUR' ? item.product.priceEUR : item.product.priceUSD;
    return sum + (price * item.quantity);
  }, 0);

  cartItems.innerHTML = cartForCurrency.map(item => {
    const price = currency === 'EUR' ? item.product.priceEUR : item.product.priceUSD;
    return `
      <div class="cart-item">
        <div class="cart-item-image">${item.product.image}</div>
        <div class="cart-item-info">
          <div class="cart-item-name">${item.product.name}</div>
          <div class="cart-item-price">${formatPrice(price, currency)}</div>
        </div>
        <div class="cart-item-quantity">
          <button class="qty-btn" onclick="updateQuantity('${item.product.id}', -1)">-</button>
          <span>${item.quantity}</span>
          <button class="qty-btn" onclick="updateQuantity('${item.product.id}', 1)">+</button>
        </div>
      </div>
    `;
  }).join('');

  cartTotalAmount.textContent = formatPrice(totalAmount, currency);
  cartCount.textContent = totalItems;
  checkoutBtn.disabled = false;
}

function openCart() {
  cartSidebar.classList.add('open');
  cartSidebar.classList.remove('hidden');
  cartOverlay.classList.add('open');
  cartOverlay.classList.remove('hidden');
}

function closeCart() {
  cartSidebar.classList.remove('open');
  cartSidebar.classList.add('hidden');
  cartOverlay.classList.remove('open');
  cartOverlay.classList.add('hidden');
}

function goToCheckout() {
  const cartForCurrency = cart.filter(item => item.currency === currency);
  const totalAmount = cartForCurrency.reduce((sum, item) => {
    const price = currency === 'EUR' ? item.product.priceEUR : item.product.priceUSD;
    return sum + (price * item.quantity);
  }, 0);

  const checkoutData = {
    items: cartForCurrency,
    currency,
    totalAmount
  };

  localStorage.setItem('checkoutData', JSON.stringify(checkoutData));
  window.location.href = '/checkout';
}

// Utility Functions
function formatPrice(amount, curr) {
  const dollars = amount / 100;
  const symbol = curr === 'EUR' ? '€' : '$';
  return `${symbol}${dollars.toFixed(2)}`;
}

function saveCartToStorage() {
  localStorage.setItem('cart', JSON.stringify(cart));
  localStorage.setItem('currency', currency);
}

function loadCartFromStorage() {
  const savedCart = localStorage.getItem('cart');
  const savedCurrency = localStorage.getItem('currency');
  
  if (savedCart) {
    cart = JSON.parse(savedCart);
  }
  
  if (savedCurrency) {
    currency = savedCurrency;
    currencySelector.value = currency;
  }
}

// Make functions globally accessible
window.addToCart = addToCart;
window.removeFromCart = removeFromCart;
window.updateQuantity = updateQuantity;