/**
 * Lazy-load the braintree-web SDK component scripts (and the Google Pay JS API)
 * from CDN. Each braintree-web component attaches a sub-namespace onto the
 * shared `window.braintree` object, so the loaders resolve that sub-namespace
 * after the script is ready. Apple Pay needs no script — `ApplePaySession` is a
 * native Safari global — and the PayPal SDK is injected by braintree's
 * `paypalCheckout` instance itself (`loadPayPalSDK`), not loaded here.
 */

const BT_VERSION = "3.103.0";
const BT_BASE = `https://js.braintreegateway.com/web/${BT_VERSION}/js`;
const GOOGLE_PAY_JS_URL = "https://pay.google.com/gp/p/js/pay.js";

declare global {
  interface Window {
    braintree?: any;
    google?: any;
    paypal?: any;
    ApplePaySession?: any;
  }
}

// Per-URL promise cache so concurrent callers share a single <script> tag.
const scriptPromises: Record<string, Promise<void> | undefined> = {};

function loadScript(src: string): Promise<void> {
  if (typeof window === "undefined") {
    return Promise.reject(
      new Error("Braintree SDK can only be loaded in the browser")
    );
  }
  const cached = scriptPromises[src];
  if (cached) return cached;

  const promise = new Promise<void>((resolve, reject) => {
    const existing = document.querySelector<HTMLScriptElement>(
      `script[src="${src}"]`
    );
    if (existing) {
      if (existing.dataset.loaded === "true") {
        resolve();
        return;
      }
      existing.addEventListener("load", () => resolve(), { once: true });
      existing.addEventListener(
        "error",
        () => reject(new Error(`Failed to load ${src}`)),
        { once: true }
      );
      return;
    }

    const script = document.createElement("script");
    script.src = src;
    script.async = true;
    script.onload = () => {
      script.dataset.loaded = "true";
      resolve();
    };
    script.onerror = () => reject(new Error(`Failed to load ${src}`));
    document.head.appendChild(script);
  });

  scriptPromises[src] = promise;
  return promise;
}

function requireBraintreeComponent(component: string): any {
  const bt = window.braintree;
  if (!bt || !bt[component]) {
    throw new Error(`braintree-web ${component} unavailable after load`);
  }
  return bt[component];
}

/** braintree-web core client — required to initialise every wallet. */
export async function loadBraintreeClient(): Promise<any> {
  await loadScript(`${BT_BASE}/client.min.js`);
  return requireBraintreeComponent("client");
}

/** braintree-web PayPal Checkout component. */
export async function loadBraintreePayPalCheckout(): Promise<any> {
  await loadScript(`${BT_BASE}/paypal-checkout.min.js`);
  return requireBraintreeComponent("paypalCheckout");
}

/** braintree-web Google Payment component. */
export async function loadBraintreeGooglePayment(): Promise<any> {
  await loadScript(`${BT_BASE}/google-payment.min.js`);
  return requireBraintreeComponent("googlePayment");
}

/** braintree-web Apple Pay component. */
export async function loadBraintreeApplePay(): Promise<any> {
  await loadScript(`${BT_BASE}/apple-pay.min.js`);
  return requireBraintreeComponent("applePay");
}

/** Google Pay JS API (`window.google.payments.api`). */
export async function loadGooglePayJs(): Promise<any> {
  await loadScript(GOOGLE_PAY_JS_URL);
  const api = window.google?.payments?.api;
  if (!api) throw new Error("Google Pay JS API unavailable after load");
  return api;
}
