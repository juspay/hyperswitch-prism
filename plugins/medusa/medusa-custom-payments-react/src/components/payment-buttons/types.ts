"use client";

import React, { useState } from "react";

/**
 * Props shared by all connector-specific payment buttons.
 */
export interface BasePaymentButtonProps {
  /** True when the checkout form is incomplete (missing address, shipping, etc.) */
  notReady: boolean;
  /** Callback invoked to place the order. Should call Medusa’s store API. */
  onPlaceOrder: () => Promise<void>;
  /** Optional custom button component (e.g. from @medusajs/ui). Falls back to native <button>. */
  buttonComponent?: React.ComponentType<any>;
  /** data-testid for the button element */
  "data-testid"?: string;
}

/**
 * Props for Stripe payment button.
 */
export interface StripePaymentButtonProps extends BasePaymentButtonProps {
  /** Cart object used to build Stripe billing_details */
  cart: {
    billing_address?: {
      first_name?: string;
      last_name?: string;
      city?: string;
      country_code?: string;
      address_1?: string;
      address_2?: string;
      postal_code?: string;
      province?: string;
      phone?: string;
    } | null;
    email?: string | null;
    payment_collection?: {
      payment_sessions?: Array<{ status: string; data?: Record<string, unknown> }>;
    } | null;
  };
}

/**
 * Props for GlobalPay payment button.
 */
export interface GlobalPayPaymentButtonProps extends BasePaymentButtonProps {
  /** Callback to update cart metadata with the tokenized payment reference */
  onUpdateCart?: (metadata: Record<string, unknown>) => Promise<void>;
}

/**
 * Props for PayPal payment button.
 *
 * PayPal handles approval client-side via the PayPal SDK. The button
 * in the review step only needs to trigger `onPlaceOrder()` which
 * calls Medusa's authorizePayment → backend captures the order.
 */
export interface PayPalPaymentButtonProps extends BasePaymentButtonProps {
  /** Cart object used to read the PayPal session data */
  cart: {
    payment_collection?: {
      payment_sessions?: Array<{
        status: string
        data?: Record<string, unknown>
      }>
    } | null
  }
}

/**
 * Props for the auto-dispatching PaymentButton.
 */
export interface PaymentButtonProps extends BasePaymentButtonProps {
  /** Medusa provider_id, e.g. "pp_hyperswitch-prism_hyperswitch-prism-globalpay" */
  providerId?: string;
  /** Cart object (required for stripe / paypal connectors) */
  cart?: StripePaymentButtonProps["cart"];
  /** Callback to update cart metadata (required for globalpay connector) */
  onUpdateCart?: (metadata: Record<string, unknown>) => Promise<void>;
}
