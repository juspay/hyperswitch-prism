"use client";

import { useEffect, useRef, useState } from "react";
import {
  loadBraintreeApplePay,
  loadBraintreeClient,
  loadBraintreeGooglePayment,
  loadBraintreePayPalCheckout,
  loadGooglePayJs,
} from "./utils";

export type BraintreeWalletType = "paypal" | "googlepay" | "applepay";

export interface BraintreeSubmitPayload {
  /** Which wallet produced the nonce — routes the server authorize arm. */
  walletType: BraintreeWalletType;
  /** Single-use Braintree payment_method_nonce. */
  nonce: string;
  /** Raw tokenize details (for debugging / address extraction). */
  details?: Record<string, unknown>;
}

interface BraintreeWrapperProps {
  /** Braintree client authorization (~2.4kb base64 client token). */
  clientToken: string;
  /** ISO 4217 currency code (e.g. "USD"). */
  currency: string;
  /** Amount in major units (e.g. 100 for $100). */
  amount: number;
  environment?: "sandbox" | "production";
  /** Which wallets to attempt. Defaults to all three. */
  enabledWallets?: BraintreeWalletType[];
  googlePay?: {
    merchantId?: string;
    gatewayMerchantId?: string;
    merchantName?: string;
    allowedAuthMethods?: string[];
    allowedCardNetworks?: string[];
  };
  applePay?: {
    supportedNetworks?: string[];
    merchantCapabilities?: string[];
    label?: string;
    countryCode?: string;
  };
  /** Tokenization success — persists the nonce on the session. Awaited. */
  onSubmit: (payload: BraintreeSubmitPayload) => void | Promise<void>;
  onError: (error: Error) => void;
}

const ALL_WALLETS: BraintreeWalletType[] = ["paypal", "googlepay", "applepay"];

/**
 * Multi-wallet braintree-web checkout (PayPal / Google Pay / Apple Pay).
 *
 * One `braintree.client.create({ authorization: clientToken })` is shared by
 * all wallets (the Braintree client token is method-agnostic). Each wallet is
 * initialised independently and only its button renders when that wallet is
 * eligible — so a wallet that is not enabled on the merchant account (or, for
 * Apple Pay, not running in Safari/HTTPS) is simply hidden rather than erroring.
 * Follows the PayPalWrapper Strict-Mode pattern (render-gate + callback refs).
 */
export function BraintreeWrapper({
  clientToken,
  currency,
  amount,
  environment = "sandbox",
  enabledWallets = ALL_WALLETS,
  googlePay,
  applePay,
  onSubmit,
  onError,
}: BraintreeWrapperProps) {
  const paypalRef = useRef<HTMLDivElement>(null);
  const gpayRef = useRef<HTMLDivElement>(null);
  const applePayRef = useRef<HTMLDivElement>(null);

  const renderedRef = useRef(false);
  const paypalButtonsRef = useRef<any>(null);
  const applePayInstanceRef = useRef<any>(null);

  const [available, setAvailable] = useState({
    paypal: false,
    googlepay: false,
    applepay: false,
  });
  const [status, setStatus] = useState<"idle" | "submitting" | "done" | "error">(
    "idle"
  );

  const onSubmitRef = useRef(onSubmit);
  const onErrorRef = useRef(onError);
  useEffect(() => {
    onSubmitRef.current = onSubmit;
  }, [onSubmit]);
  useEffect(() => {
    onErrorRef.current = onError;
  }, [onError]);

  // The PayPal/GooglePay/ApplePay APIs all want a major-unit decimal string.
  const amountStr = amount.toFixed(2);
  const wants = (w: BraintreeWalletType) => enabledWallets.includes(w);

  useEffect(() => {
    if (renderedRef.current) return;
    renderedRef.current = true;

    let closed = false;

    const handleError = (err: any) =>
      onErrorRef.current(err instanceof Error ? err : new Error(String(err)));

    const submit = async (payload: BraintreeSubmitPayload) => {
      setStatus("submitting");
      try {
        await onSubmitRef.current(payload);
        if (!closed) setStatus("done");
      } catch (err) {
        if (!closed) setStatus("error");
        handleError(err);
      }
    };

    const initPayPal = async (client: any) => {
      const paypalCheckout = await loadBraintreePayPalCheckout();
      const instance = await paypalCheckout.create({ client });
      await instance.loadPayPalSDK({ currency, intent: "capture" });
      if (closed || !paypalRef.current || !window.paypal) return;

      const buttons = window.paypal.Buttons({
        fundingSource: window.paypal.FUNDING?.PAYPAL,
        style: { layout: "vertical", label: "paypal" },
        createOrder: () =>
          instance.createPayment({
            flow: "checkout",
            amount: amountStr,
            currency,
            intent: "capture",
          }),
        onApprove: (data: any) =>
          instance
            .tokenizePayment(data)
            .then((payload: any) =>
              submit({
                walletType: "paypal",
                nonce: payload.nonce,
                details: payload,
              })
            ),
        onError: handleError,
      });

      if (buttons.isEligible?.() !== false) {
        paypalButtonsRef.current = buttons;
        buttons.render(paypalRef.current);
        if (!closed) setAvailable((a) => ({ ...a, paypal: true }));
      }
    };

    const initGooglePay = async (client: any) => {
      const googlePayment = await loadBraintreeGooglePayment();
      const instance = await googlePayment.create({
        client,
        googlePayVersion: 2,
        ...(googlePay?.merchantId
          ? { googleMerchantId: googlePay.merchantId }
          : {}),
      });
      const googleApi = await loadGooglePayJs();
      if (closed || !gpayRef.current) return;

      const paymentsClient = new googleApi.PaymentsClient({
        environment: environment === "production" ? "PRODUCTION" : "TEST",
      });

      const request = await instance.createPaymentDataRequest({
        transactionInfo: {
          currencyCode: currency,
          totalPriceStatus: "FINAL",
          totalPrice: amountStr,
        },
        ...(googlePay?.merchantName
          ? {
              merchantInfo: {
                merchantName: googlePay.merchantName,
                ...(googlePay.merchantId
                  ? { merchantId: googlePay.merchantId }
                  : {}),
              },
            }
          : {}),
      });

      // Merge any explicitly-configured card constraints into the request that
      // braintree-web pre-populated from the client token.
      const cardMethod = request.allowedPaymentMethods?.[0];
      if (cardMethod?.parameters) {
        if (googlePay?.allowedAuthMethods) {
          cardMethod.parameters.allowedAuthMethods = googlePay.allowedAuthMethods;
        }
        if (googlePay?.allowedCardNetworks) {
          cardMethod.parameters.allowedCardNetworks =
            googlePay.allowedCardNetworks;
        }
      }

      const readiness = await paymentsClient.isReadyToPay({
        apiVersion: 2,
        apiVersionMinor: 0,
        allowedPaymentMethods: request.allowedPaymentMethods,
      });
      if (!readiness?.result || closed || !gpayRef.current) return;

      const button = paymentsClient.createButton({
        buttonType: "pay",
        buttonSizeMode: "fill",
        onClick: async () => {
          try {
            const paymentData = await paymentsClient.loadPaymentData(request);
            const { nonce } = await instance.parseResponse(paymentData);
            await submit({
              walletType: "googlepay",
              nonce,
              details: paymentData,
            });
          } catch (err: any) {
            // The shopper dismissing the sheet is not an error.
            if (err?.statusCode !== "CANCELED") handleError(err);
          }
        },
      });
      gpayRef.current.appendChild(button);
      if (!closed) setAvailable((a) => ({ ...a, googlepay: true }));
    };

    const initApplePay = async (client: any) => {
      // Apple Pay only exists in Safari on Apple hardware, over HTTPS, with a
      // validated merchant domain. Feature-detect and bail out quietly.
      if (
        !window.ApplePaySession ||
        typeof window.ApplePaySession.canMakePayments !== "function" ||
        !window.ApplePaySession.canMakePayments()
      ) {
        return;
      }
      const applePaySdk = await loadBraintreeApplePay();
      const instance = await applePaySdk.create({ client });
      if (closed) return;
      applePayInstanceRef.current = instance;
      setAvailable((a) => ({ ...a, applepay: true }));
    };

    loadBraintreeClient()
      .then(async (braintreeClient) => {
        const client = await braintreeClient.create({
          authorization: clientToken,
        });
        if (closed) return;

        // Each wallet initialises independently; one failure must not block the
        // others (e.g. Google Pay not enabled on the account).
        const arms: Array<Promise<void>> = [];
        if (wants("paypal"))
          arms.push(initPayPal(client).catch(handleError));
        if (wants("googlepay"))
          arms.push(
            initGooglePay(client).catch((e) => {
              // GPay-not-enabled is common; log but don't surface as a hard error.
              // eslint-disable-next-line no-console
              console.warn("[BraintreeWrapper] Google Pay unavailable:", e?.message);
            })
          );
        if (wants("applepay"))
          arms.push(
            initApplePay(client).catch((e) => {
              // eslint-disable-next-line no-console
              console.warn("[BraintreeWrapper] Apple Pay unavailable:", e?.message);
            })
          );
        await Promise.all(arms);
      })
      .catch((err) => {
        if (!closed) handleError(err);
      });

    return () => {
      closed = true;
      renderedRef.current = false;
      if (paypalButtonsRef.current?.close) {
        try {
          paypalButtonsRef.current.close();
        } catch {
          /* noop */
        }
      }
      paypalButtonsRef.current = null;
      if (applePayInstanceRef.current?.teardown) {
        try {
          applePayInstanceRef.current.teardown();
        } catch {
          /* noop */
        }
      }
      applePayInstanceRef.current = null;
      if (gpayRef.current) gpayRef.current.innerHTML = "";
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [clientToken, currency, environment]);

  const handleApplePayClick = () => {
    const instance = applePayInstanceRef.current;
    if (!instance) return;
    try {
      const paymentRequest = instance.createPaymentRequest({
        total: { label: applePay?.label ?? "Store", amount: amountStr },
        currencyCode: currency,
        countryCode: applePay?.countryCode ?? "US",
        ...(applePay?.supportedNetworks
          ? { supportedNetworks: applePay.supportedNetworks }
          : {}),
        ...(applePay?.merchantCapabilities
          ? { merchantCapabilities: applePay.merchantCapabilities }
          : {}),
      });

      // ApplePaySession MUST be constructed synchronously in the click handler.
      const session = new window.ApplePaySession(3, paymentRequest);
      session.onvalidatemerchant = (event: any) => {
        instance
          .performValidation({
            validationURL: event.validationURL,
            displayName: applePay?.label ?? "Store",
          })
          .then((merchantSession: any) =>
            session.completeMerchantValidation(merchantSession)
          )
          .catch((err: any) => {
            session.abort();
            onErrorRef.current(
              err instanceof Error ? err : new Error(String(err))
            );
          });
      };
      session.onpaymentauthorized = (event: any) => {
        instance
          .tokenize({ token: event.payment.token })
          .then(({ nonce }: any) => {
            session.completePayment(window.ApplePaySession.STATUS_SUCCESS);
            setStatus("submitting");
            return Promise.resolve(
              onSubmitRef.current({
                walletType: "applepay",
                nonce,
                details: event.payment,
              })
            )
              .then(() => setStatus("done"))
              .catch((err) => {
                setStatus("error");
                onErrorRef.current(
                  err instanceof Error ? err : new Error(String(err))
                );
              });
          })
          .catch((err: any) => {
            session.completePayment(window.ApplePaySession.STATUS_FAILURE);
            onErrorRef.current(
              err instanceof Error ? err : new Error(String(err))
            );
          });
      };
      session.begin();
    } catch (err) {
      onErrorRef.current(err instanceof Error ? err : new Error(String(err)));
    }
  };

  const noneAvailable =
    !available.paypal && !available.googlepay && !available.applepay;

  return (
    <div className="braintree-payment-container" data-testid="braintree-wallets">
      {wants("paypal") && (
        <div ref={paypalRef} data-testid="braintree-paypal" />
      )}

      {wants("googlepay") && (
        <div
          ref={gpayRef}
          data-testid="braintree-googlepay"
          style={{ marginTop: available.paypal ? 12 : 0 }}
        />
      )}

      {wants("applepay") && available.applepay && (
        <button
          type="button"
          data-testid="braintree-applepay"
          onClick={handleApplePayClick}
          aria-label="Pay with Apple Pay"
          style={{
            width: "100%",
            marginTop: available.paypal || available.googlepay ? 12 : 0,
            padding: "14px 20px",
            border: "none",
            borderRadius: 8,
            background: "#000",
            color: "#fff",
            fontSize: 16,
            fontWeight: 600,
            cursor: "pointer",
          }}
        >
           Pay
        </button>
      )}

      {status === "submitting" && (
        <p data-testid="braintree-status" style={{ color: "#555", fontSize: 13, marginTop: 12 }}>
          Processing payment…
        </p>
      )}
      {status === "done" && (
        <p data-testid="braintree-status" style={{ color: "#15803d", fontSize: 13, marginTop: 12 }}>
          Payment authorized.
        </p>
      )}

      {noneAvailable && status === "idle" && (
        <p style={{ color: "#6b7280", fontSize: 13 }}>
          Loading wallet payment options…
        </p>
      )}
    </div>
  );
}
