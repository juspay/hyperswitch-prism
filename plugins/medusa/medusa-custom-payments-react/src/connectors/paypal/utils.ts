/**
 * Lazy-load the PayPal JS SDK.
 *
 * @param clientId – PayPal client id
 * @param currency – ISO 4217 currency code (determines which funding sources show)
 */
export function loadPayPalScript(
  clientId: string,
  currency: string = "USD",
  environment: "sandbox" | "production" = "production"
): Promise<any> {
  // The PayPal JS SDK is always served from www.paypal.com — sandbox vs production
  // is determined by the client ID itself, not a different domain.
  // `components=buttons,card-fields` loads both the wallet buttons and the
  // Advanced Card Fields (hosted fields) component used for the card path.
  const src = `https://www.paypal.com/sdk/js?client-id=${encodeURIComponent(
    clientId
  )}&currency=${encodeURIComponent(
    currency.toUpperCase()
  )}&intent=capture&components=buttons,card-fields`

  return new Promise((resolve, reject) => {
    const existing = document.querySelector<HTMLScriptElement>(
      `script[data-namespace="paypal"]`
    )
    if (existing) {
      const winPaypal = (window as any).paypal
      if (winPaypal) {
        resolve(winPaypal)
        return
      }
      // Script tag exists but window.paypal not ready yet – wait for it
      const onLoad = () => resolve((window as any).paypal)
      existing.addEventListener("load", onLoad, { once: true })
      return
    }

    const script = document.createElement("script")
    script.src = src
    script.async = true
    script.defer = true
    script.dataset.namespace = "paypal"
    script.onload = () => resolve((window as any).paypal)
    script.onerror = () => reject(new Error("Failed to load PayPal SDK"))
    document.head.appendChild(script)
  })
}
