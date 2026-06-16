"use client"

import React, { useState } from "react"
import type { PayPalPaymentButtonProps } from "./types"
import { isNextControlFlowError } from "../../utils/redirect-error"

/**
 * PayPal payment button for the review step.
 *
 * PayPal user approval happens client-side in the payment step
 * (via the PayPal SDK buttons rendered in PayPalContainer).
 * This button only triggers order placement, which calls
 * Medusa's authorizePayment → backend captures the PayPal order.
 */
export function PayPalPaymentButton({
  notReady,
  onPlaceOrder,
  buttonComponent: Button,
  "data-testid": dataTestId,
}: PayPalPaymentButtonProps) {
  const [submitting, setSubmitting] = useState(false)
  const [errorMessage, setErrorMessage] = useState<string | null>(null)

  const handlePayment = async () => {
    setSubmitting(true)
    setErrorMessage(null)

    try {
      await onPlaceOrder()
    } catch (err: any) {
      if (isNextControlFlowError(err)) throw err
      setErrorMessage(err.message || "PayPal order placement failed")
    } finally {
      setSubmitting(false)
    }
  }

  const btnProps = {
    disabled: notReady || submitting,
    onClick: handlePayment,
    "data-testid": dataTestId,
    type: "button" as const,
  }

  return (
    <>
      {Button ? (
        <Button {...btnProps} size="large" isLoading={submitting}>
          Place order
        </Button>
      ) : (
        <button {...btnProps} className="payment-btn">
          {submitting ? "Processing…" : "Place order"}
        </button>
      )}
      {errorMessage && (
        <div className="payment-error" style={{ color: "#c00", marginTop: 8 }}>
          {errorMessage}
        </div>
      )}
    </>
  )
}
