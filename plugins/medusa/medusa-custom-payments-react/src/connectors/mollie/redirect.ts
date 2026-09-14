// Mollie 3DS redirect helpers. Pure (no React / no "use client") — the DOM is
// only touched at call time inside the browser.

export type MollieRedirect = {
  url?: string
  form?: { endpoint: string; fields: Record<string, string>; method: string }
}

/**
 * Reads the active payment session straight from the Medusa store API (no cache)
 * to retrieve the 3DS redirect the plugin persisted during a failed cart
 * completion. `placeOrder()` does not revalidate the storefront cache on that
 * path, so we fetch live.
 */
export async function fetchMollieRedirect(
  cartId: string,
  opts: { backendUrl: string; publishableKey: string }
): Promise<MollieRedirect> {
  try {
    const res = await fetch(
      `${opts.backendUrl}/store/carts/${cartId}?fields=*payment_collection.payment_sessions`,
      { headers: { "x-publishable-api-key": opts.publishableKey }, cache: "no-store" }
    )
    if (!res.ok) return {}
    const body = await res.json()
    const sessions: Array<{ status: string; data?: Record<string, any> }> =
      body?.cart?.payment_collection?.payment_sessions ?? []
    const active =
      sessions.find((s) => s.status === "requires_more") ??
      sessions.find((s) => s.status === "pending")
    return {
      url: active?.data?.redirectUrl as string | undefined,
      form: active?.data?.redirectForm as MollieRedirect["form"],
    }
  } catch {
    return {}
  }
}

/**
 * Follows the Mollie 3DS redirect. The plugin surfaces a ready GET URL
 * (`url`); a `form` (POST) is supported as a fallback for connectors that
 * return form-based redirects. Returns true if a redirect was initiated.
 */
export function followMollieRedirect(redirect: MollieRedirect): boolean {
  if (redirect.form?.endpoint) {
    const form = document.createElement("form")
    form.method = (redirect.form.method || "POST").toUpperCase()
    form.action = redirect.form.endpoint
    for (const [name, value] of Object.entries(redirect.form.fields || {})) {
      const input = document.createElement("input")
      input.type = "hidden"
      input.name = name
      input.value = String(value)
      form.appendChild(input)
    }
    document.body.appendChild(form)
    form.submit()
    return true
  }
  if (redirect.url) {
    window.location.assign(redirect.url)
    return true
  }
  return false
}
