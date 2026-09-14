/**
 * Dynamically load Mollie Components (mollie.js) from CDN.
 * Mirrors connectors/globalpay/utils.ts.
 */

const MOLLIE_SCRIPT_URL = "https://js.mollie.com/v1/mollie.js";

let scriptLoadPromise: Promise<any> | null = null;

declare global {
  interface Window {
    Mollie?: any;
  }
}

/**
 * Inject the mollie.js script tag. Resolves with the global `Mollie` factory.
 */
export function loadMollieScript(): Promise<any> {
  if (typeof window === "undefined") {
    return Promise.reject(new Error("Mollie SDK can only be loaded in the browser"));
  }

  if (window.Mollie) {
    return Promise.resolve(window.Mollie);
  }

  if (scriptLoadPromise) {
    return scriptLoadPromise;
  }

  scriptLoadPromise = new Promise((resolve, reject) => {
    const onLoad = () => {
      if (window.Mollie) {
        resolve(window.Mollie);
      } else {
        reject(new Error("Mollie SDK loaded but window.Mollie is undefined"));
      }
    };

    const existing = document.querySelector(`script[src="${MOLLIE_SCRIPT_URL}"]`);
    if (existing) {
      existing.addEventListener("load", onLoad);
      existing.addEventListener("error", () => reject(new Error("Failed to load Mollie SDK")));
      return;
    }

    const script = document.createElement("script");
    script.src = MOLLIE_SCRIPT_URL;
    script.async = true;
    script.onload = onLoad;
    script.onerror = () => reject(new Error("Failed to load Mollie SDK from CDN"));
    document.head.appendChild(script);
  });

  return scriptLoadPromise;
}
