"use client"

import React, { useEffect, useRef, useState } from "react"
import { isNextControlFlowError } from "../utils/redirect-error"

export interface MollieReturnHandlerProps {
  /**
   * Finalise the order. Should call the host's place-order action (e.g. Medusa
   * `placeOrder()`), which re-runs cart.complete → the plugin PSyncs the existing
   * Mollie payment → on success it server-redirects to the order-confirmed page.
   */
  onFinalize: () => Promise<void>
  /** Where to send the shopper if finalisation ultimately fails. */
  backHref: string
  /** Max finalize attempts (Mollie's `paid` status can lag the redirect). Default 8. */
  maxAttempts?: number
  /** Delay between attempts in ms. Default 1500. */
  retryDelayMs?: number
  /** Optional host link component (e.g. Medusa `LocalizedClientLink`). Falls back to `<a>`. */
  linkComponent?: React.ComponentType<{
    href: string
    className?: string
    "data-testid"?: string
    children?: React.ReactNode
  }>
  className?: string
}

/**
 * Landing component for the Mollie 3DS return. Polls `onFinalize` until the
 * order completes (the host's place-order navigates away) or the attempts are
 * exhausted, then shows a neutral error with a link back to checkout.
 *
 * Render this from a route the consumer maps to the `returnUrl` passed to the
 * connector panel (e.g. `/[cc]/checkout/mollie-return`).
 */
export function MollieReturnHandler({
  onFinalize,
  backHref,
  maxAttempts = 8,
  retryDelayMs = 1500,
  linkComponent: Link,
  className,
}: MollieReturnHandlerProps) {
  const ranRef = useRef(false)
  const [failed, setFailed] = useState(false)

  useEffect(() => {
    if (ranRef.current) return
    ranRef.current = true

    const finalize = async () => {
      for (let attempt = 0; attempt < maxAttempts; attempt++) {
        try {
          await onFinalize()
          return
        } catch (err) {
          // The success path throws a Next redirect — let it navigate.
          if (isNextControlFlowError(err)) throw err
          if (attempt < maxAttempts - 1) {
            await new Promise((r) => setTimeout(r, retryDelayMs))
          }
        }
      }
      setFailed(true)
    }

    finalize()
  }, [onFinalize, maxAttempts, retryDelayMs])

  const wrap: React.CSSProperties = {
    display: "flex",
    flexDirection: "column",
    alignItems: "center",
    justifyContent: "center",
    gap: 16,
    padding: "8rem 1rem",
    textAlign: "center",
  }

  if (failed) {
    return (
      <div className={className} style={wrap} data-testid="mollie-return-failed">
        <h1 style={{ fontSize: "1.5rem", fontWeight: 500 }}>
          We couldn&apos;t confirm your payment
        </h1>
        <p style={{ maxWidth: 480, color: "#6b7280" }}>
          Your payment was not completed. If you were charged, it will be
          refunded automatically. You can return to checkout and try again.
        </p>
        {Link ? (
          <Link href={backHref} data-testid="mollie-return-retry-link">
            Return to checkout
          </Link>
        ) : (
          <a href={backHref} data-testid="mollie-return-retry-link">
            Return to checkout
          </a>
        )}
      </div>
    )
  }

  return (
    <div className={className} style={wrap} data-testid="mollie-return-finalizing">
      <span
        aria-hidden
        style={{
          width: 28,
          height: 28,
          border: "3px solid #d1d5db",
          borderTopColor: "#111827",
          borderRadius: "50%",
          display: "inline-block",
          animation: "mollie-return-spin 0.8s linear infinite",
        }}
      />
      <style>{"@keyframes mollie-return-spin{to{transform:rotate(360deg)}}"}</style>
      <h1 style={{ fontSize: "1.5rem", fontWeight: 500 }}>Finalizing your payment…</h1>
      <p style={{ color: "#6b7280" }}>
        Please wait while we confirm your order. Do not close this window.
      </p>
    </div>
  )
}
