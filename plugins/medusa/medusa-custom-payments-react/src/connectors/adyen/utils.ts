/**
 * Lazy-load the Adyen Web SDK script.
 * Returns the global `AdyenCheckout` constructor.
 */
export function loadAdyenScript(
  environment: "test" | "live" = "test"
): Promise<any> {
  const src =
    environment === "live"
      ? "https://checkoutshopper-live.adyen.com/checkoutshopper/sdk/5.53.0/adyen.js"
      : "https://checkoutshopper-test.adyen.com/checkoutshopper/sdk/5.53.0/adyen.js";

  return new Promise((resolve, reject) => {
    const existing = document.querySelector<HTMLScriptElement>(
      `script[src="${src}"]`
    );
    if (existing) {
      // Already loaded – wait a tick for the global to be ready
      setTimeout(() => resolve((window as any).AdyenCheckout), 100);
      return;
    }

    const script = document.createElement("script");
    script.src = src;
    script.async = true;
    script.onload = () => resolve((window as any).AdyenCheckout);
    script.onerror = () => reject(new Error("Failed to load Adyen SDK"));
    document.head.appendChild(script);
  });
}

/**
 * Inject Adyen CSS if not already present.
 */
export function injectAdyenStyles(environment: "test" | "live" = "test"): void {
  const href =
    environment === "live"
      ? "https://checkoutshopper-live.adyen.com/checkoutshopper/sdk/5.53.0/adyen.css"
      : "https://checkoutshopper-test.adyen.com/checkoutshopper/sdk/5.53.0/adyen.css";

  if (document.querySelector(`link[href="${href}"]`)) return;

  const link = document.createElement("link");
  link.rel = "stylesheet";
  link.href = href;
  document.head.appendChild(link);
}
