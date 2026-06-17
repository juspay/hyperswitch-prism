/**
 * Dynamically load the GlobalPay/Heartland Payments JS SDK from CDN.
 */

const GLOBALPAY_SCRIPT_URL = "https://js.globalpay.com/4.1.21/globalpayments.js";

let scriptLoadPromise: Promise<any> | null = null;

declare global {
  interface Window {
    GlobalPayments?: any;
  }
}

/**
 * Inject the GlobalPay SDK script tag into the document head.
 * Returns a promise that resolves when `window.GlobalPayments` is available.
 */
export function loadGlobalPayScript(): Promise<any> {
  if (typeof window === "undefined") {
    return Promise.reject(new Error("GlobalPay SDK can only be loaded in the browser"));
  }

  if (window.GlobalPayments) {
    return Promise.resolve(window.GlobalPayments);
  }

  if (scriptLoadPromise) {
    return scriptLoadPromise;
  }

  scriptLoadPromise = new Promise((resolve, reject) => {
    const existing = document.querySelector(`script[src="${GLOBALPAY_SCRIPT_URL}"]`);
    if (existing) {
      existing.addEventListener("load", () => {
        if (window.GlobalPayments) {
          resolve(window.GlobalPayments);
        } else {
          reject(new Error("GlobalPay SDK loaded but window.GlobalPayments is undefined"));
        }
      });
      existing.addEventListener("error", () => {
        reject(new Error("Failed to load GlobalPay SDK"));
      });
      return;
    }

    const script = document.createElement("script");
    script.src = GLOBALPAY_SCRIPT_URL;
    script.async = true;
    script.defer = true;

    script.onload = () => {
      if (window.GlobalPayments) {
        resolve(window.GlobalPayments);
      } else {
        reject(new Error("GlobalPay SDK loaded but window.GlobalPayments is undefined"));
      }
    };

    script.onerror = () => {
      reject(new Error("Failed to load GlobalPay SDK from CDN"));
    };

    document.head.appendChild(script);
  });

  return scriptLoadPromise;
}
