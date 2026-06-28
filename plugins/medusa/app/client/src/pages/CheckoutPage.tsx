import { useEffect, useCallback, useState } from "react";
import { useParams, Link, useNavigate } from "react-router-dom";
import { loadScript } from "../loadScript";

const RAZORPAY_CHECKOUT_JS = "https://checkout.razorpay.com/v1/checkout.js";

import {
  StripeWrapper,
  AdyenWrapper,
  PayPalWrapper,
  GlobalPayWrapper,
  MollieWrapper,
  MollieKlarnaForm,
  type MollieKlarnaBilling,
} from "@juspay-tech/medusa-custom-payments-react";

// The Stripe publishable key and Adyen client key are delivered by the server
// in the payment session (sessionData.publishableKey), sourced from creds.json.

const CART = { cartId: "cart_test_01", amount: 100, currency: "USD" };

const CONNECTOR_LABELS: Record<string, string> = {
  stripe: "Stripe",
  adyen: "Adyen",
  paypal: "PayPal",
  globalpay: "GlobalPay",
  mollie: "Mollie",
  razorpay: "Razorpay",
};

const SUPPORTED = [
  "stripe",
  "adyen",
  "paypal",
  "globalpay",
  "mollie",
  "razorpay",
];

type SessionState = {
  collectionId: string;
  sessionId: string;
  data: Record<string, any>;
};

async function initiateSession(connector: string): Promise<SessionState> {
  const collRes = await fetch("/store/payment-collections", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      cart_id: CART.cartId,
      currency_code: CART.currency,
      amount: CART.amount,
    }),
  });
  if (!collRes.ok) {
    const body = await collRes.json().catch(() => ({}));
    throw new Error(body.error ?? `Payment collection failed (${collRes.status})`);
  }
  const collBody = await collRes.json();
  const collectionId: string = collBody.payment_collection.id;

  const sessRes = await fetch(`/store/payment-collections/${collectionId}/payment-sessions`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ provider_id: connector }),
  });
  if (!sessRes.ok) {
    const body = await sessRes.json().catch(() => ({}));
    throw new Error(body.error ?? `Payment session failed (${sessRes.status})`);
  }
  const sessBody = await sessRes.json();
  const ps = sessBody.payment_collection.payment_sessions[0];
  return { collectionId, sessionId: ps.id, data: ps.data };
}

export default function CheckoutPage() {
  const { connector } = useParams<{ connector: string }>();
  const navigate = useNavigate();

  const [session, setSession] = useState<SessionState | null>(null);
  const [sessionLoading, setSessionLoading] = useState(false);
  const [sessionError, setSessionError] = useState<Error | null>(null);

  useEffect(() => {
    if (!connector || !SUPPORTED.includes(connector)) return;
    setSessionLoading(true);
    setSessionError(null);
    initiateSession(connector)
      .then(setSession)
      .catch(setSessionError)
      .finally(() => setSessionLoading(false));
  }, [connector]);

  const handleComplete = useCallback(async () => {
    navigate("/authorize");
  }, [navigate]);

  const label = connector ? (CONNECTOR_LABELS[connector] ?? connector) : "";
  const sessionData = session?.data;

  return (
    <div style={{ maxWidth: 560, margin: "60px auto", padding: "0 16px" }}>
      <Link to="/" style={{ color: "#555", textDecoration: "none", fontSize: 14 }}>
        ← Back
      </Link>

      <h2 style={{ fontSize: 22, fontWeight: 700, margin: "16px 0 4px" }}>
        {label} Checkout
      </h2>
      <p style={{ color: "#666", fontSize: 13, marginBottom: 28 }}>
        Amount: <strong>${CART.amount} {CART.currency}</strong> · Cart:{" "}
        <code>{CART.cartId}</code>
      </p>

      {sessionError && (
        <div data-testid="checkout-error" style={{ background: "#fff0f0", border: "1px solid #fcc", borderRadius: 6, padding: "12px 16px", color: "#c00", marginBottom: 20 }}>
          <strong>Error:</strong> {sessionError.message}
        </div>
      )}

      {!connector || !SUPPORTED.includes(connector) ? (
        <p style={{ color: "#c00" }}>
          Unknown connector: <code>{connector}</code>. Supported:{" "}
          {SUPPORTED.join(", ")}.
        </p>
      ) : sessionLoading ? (
        <p data-testid="checkout-loading" style={{ color: "#555" }}>Initialising payment session…</p>
      ) : !sessionData ? null : (
        <div data-testid={`checkout-${connector}`}>
          <ConnectorUI
            connector={connector}
            sessionId={session!.sessionId}
            sessionData={sessionData}
            onComplete={handleComplete}
            onError={setSessionError}
          />
        </div>
      )}
    </div>
  );
}

interface ConnectorUIProps {
  connector: string;
  sessionId: string;
  sessionData: Record<string, any>;
  onComplete: () => Promise<void>;
  onError: (e: Error) => void;
}

function ConnectorUI({ connector, sessionId, sessionData, onComplete, onError }: ConnectorUIProps) {
  switch (connector) {
    case "stripe":
      return (
        <StripeWrapper
          publishableKey={sessionData.publishableKey ?? ""}
          clientSecret={sessionData.client_secret ?? ""}
          onSubmit={onComplete}
          onError={onError}
        />
      );

    case "adyen":
      return (
        <AdyenWrapper
          sessionData={{
            clientKey: sessionData.publishableKey ?? "",
            session: {
              id: sessionData.id,
              sessionData:
                sessionData.sessionData?.connectorSpecific?.adyen?.sessionData
                  ?.value ?? "",
            },
            countryCode: sessionData.countryCode ?? "US",
            minorAmount: sessionData.minorAmount ?? 0,
            currency: sessionData.currency ?? CART.currency,
          }}
          onPaymentCompleted={onComplete}
          onError={onError}
        />
      );

    case "paypal":
      return (
        <PayPalWrapper
          clientId={sessionData.paypalClientId ?? ""}
          currency={sessionData.currency ?? CART.currency}
          amount={CART.amount}
          environment="sandbox"
          onCreateOrder={async () => {
            const orderId = sessionData.paypalOrderId;
            if (!orderId) throw new Error("paypalOrderId missing from session data");
            return orderId;
          }}
          onSubmit={async (paymentData: any) => {
            Object.assign(sessionData, {
              ...paymentData,
              id: paymentData.orderId,
              minorAmount: sessionData.minorAmount,
              currency: sessionData.currency ?? CART.currency,
            });
            await onComplete();
          }}
          onError={onError}
        />
      );

    case "globalpay":
      return (
        <GlobalPayWrapper
          accessToken={sessionData.accessToken ?? ""}
          environment="sandbox"
          onSubmit={async (paymentData) => {
            // Mirrors HyperswitchPrismConnectorPanel.onInitiateSession:
            // re-initiates the session with { paymentReference, id } so the
            // prism service stores the card token before authorizePayment runs.
            await fetch(`/store/payment-sessions/${sessionId}/reinitiate`, {
              method: "POST",
              headers: { "Content-Type": "application/json" },
              body: JSON.stringify({
                data: {
                  paymentReference: (paymentData as any).paymentReference,
                  id: sessionData.id,
                },
              }),
            });
            await onComplete();
          }}
          onError={onError}
        />
      );

    case "mollie":
      // Mollie supports two methods here: in-page Card (Components, USD) and
      // Klarna (PayLater redirect, EUR). MollieCheckout renders a toggle.
      return (
        <MollieCheckout
          sessionId={sessionId}
          sessionData={sessionData}
          onError={onError}
        />
      );

    case "razorpay":
      // Razorpay is India-first (INR). The page-load session is USD, so
      // RazorpayCheckout spins up a fresh INR order and opens Razorpay's modal.
      return <RazorpayCheckout onError={onError} />;

    default:
      return null;
  }
}

/**
 * Razorpay checkout via Razorpay's native Standard Checkout (checkout.js). We
 * create an INR order server-side (prism `ServerSessionAuthenticationToken`),
 * then open Razorpay's own modal — which renders the UPI QR / intent / cards UI
 * and handles the waiting-for-approval state itself. On success the modal returns
 * a signed result we verify server-side, then land on the order page. (We no
 * longer hand-build a UPI form / QR — Razorpay provides those elements.)
 */
function RazorpayCheckout({ onError }: { onError: (e: Error) => void }) {
  const [loading, setLoading] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);

  const pay = useCallback(async () => {
    setNotice(null);
    setLoading(true);
    try {
      const readJson = async (res: Response, what: string) => {
        const body = await res.json().catch(() => ({} as any));
        if (!res.ok) throw new Error(body.error ?? `${what} failed (${res.status})`);
        return body;
      };

      // 1. Fresh INR session → prism creates the Razorpay order + surfaces the
      //    order_id + publishable key_id.
      const coll = await readJson(
        await fetch("/store/payment-collections", {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ cart_id: CART.cartId, currency_code: "INR", amount: CART.amount }),
        }),
        "Create payment collection"
      );
      const sess = await readJson(
        await fetch(`/store/payment-collections/${coll.payment_collection.id}/payment-sessions`, {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ provider_id: "razorpay" }),
        }),
        "Create payment session"
      );
      const ps = sess.payment_collection.payment_sessions[0];
      const sessionId: string = ps.id;
      const { orderId, publishableKey, minorAmount } = (ps.data ?? {}) as Record<string, any>;
      if (!orderId || !publishableKey) {
        throw new Error("Razorpay order_id / key_id missing from session — cannot open checkout");
      }

      // 2. Load Razorpay Checkout JS and open the native modal.
      await loadScript(RAZORPAY_CHECKOUT_JS);
      const Razorpay = (window as any).Razorpay;
      if (!Razorpay) throw new Error("Razorpay Checkout JS failed to load");

      const rzp = new Razorpay({
        key: publishableKey,
        order_id: orderId,
        amount: minorAmount,
        currency: "INR",
        name: "Medusa Prism Store",
        description: "Order cart_test_01",
        prefill: { name: "Asha Kumar", email: "asha.kumar@example.com", contact: "9999999999" },
        theme: { color: "#3b5cb8" },
        // 3. Verify the signed result server-side, then go to the order page.
        handler: async (resp: any) => {
          try {
            await readJson(
              await fetch(`/store/payment-sessions/${sessionId}/razorpay-verify`, {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({
                  razorpay_payment_id: resp.razorpay_payment_id,
                  razorpay_order_id: resp.razorpay_order_id,
                  razorpay_signature: resp.razorpay_signature,
                }),
              }),
              "Verify payment"
            );
            window.location.assign(`/order/${sessionId}`);
          } catch (err) {
            onError(err as Error);
          }
        },
        modal: {
          ondismiss: () => {
            setLoading(false);
            setNotice("Payment cancelled. You can try again.");
          },
        },
      });
      rzp.on("payment.failed", (resp: any) => {
        setLoading(false);
        setNotice(`Payment failed: ${resp?.error?.description ?? "unknown error"}`);
      });
      rzp.open();
    } catch (err) {
      setLoading(false);
      onError(err as Error);
    }
  }, [onError]);

  return (
    <div>
      {notice && (
        <div
          style={{
            background: "#fffbeb",
            border: "1px solid #f6e05e",
            borderRadius: 6,
            padding: "10px 12px",
            marginBottom: 14,
            fontSize: 13,
            color: "#744210",
          }}
        >
          {notice}
        </div>
      )}
      <button
        type="button"
        data-testid="razorpay-pay"
        onClick={pay}
        disabled={loading}
        style={{
          width: "100%",
          padding: "12px 16px",
          borderRadius: 8,
          border: "none",
          background: loading ? "#9aa9d6" : "#3b5cb8",
          color: "#fff",
          fontWeight: 600,
          fontSize: 15,
          cursor: loading ? "not-allowed" : "pointer",
        }}
      >
        {loading ? "Opening Razorpay…" : `Pay ${CART.amount} INR with Razorpay`}
      </button>
      <p style={{ color: "#777", fontSize: 12, marginTop: 10, textAlign: "center" }}>
        Opens Razorpay Checkout — UPI (QR / intent), cards, netbanking & wallets.
      </p>
    </div>
  );
}

/**
 * Mollie checkout with a Card | Klarna toggle.
 * - Card: in-page Components → cardToken → reinitiate → complete → 3DS redirect.
 * - Klarna: PayLater redirect. Klarna requires an EU currency, so it spins up a
 *   fresh EUR Mollie session (the page-load session is USD for card), persists
 *   the billing + paymentMethodType, completes, and follows the Klarna redirect.
 */
function MollieCheckout({
  sessionId,
  sessionData,
  onError,
}: {
  sessionId: string;
  sessionData: Record<string, any>;
  onError: (e: Error) => void;
}) {
  const [method, setMethod] = useState<"card" | "klarna">("card");

  // Parse a JSON response, surfacing a clear error if the request failed.
  // Without this, a non-2xx body silently flows on and derefs `undefined`
  // (e.g. coll.payment_collection.id), crashing with an opaque message — or, for
  // the reinitiate call, loses the billing data and fails later as "missing
  // billing" at authorize. Mirrors the /complete handling below.
  const readJson = async (res: Response, what: string) => {
    const body = await res.json().catch(() => ({} as any));
    if (!res.ok) {
      throw new Error(body.error ?? `${what} failed (${res.status})`);
    }
    return body;
  };

  const payKlarna = async (billing: MollieKlarnaBilling) => {
    // Klarna via Mollie is EU-only → create a fresh EUR Mollie session for the
    // cart. createSession overwrites cartToSession, so this becomes the active
    // session that /complete authorizes. NOTE: the original USD session created
    // on page load is left as-is (not voided) — harmless here since /complete
    // authorizes whichever session is active, but a production flow should cancel
    // the abandoned session.
    const collRes = await fetch("/store/payment-collections", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        cart_id: CART.cartId,
        currency_code: "EUR",
        amount: CART.amount,
      }),
    });
    const coll = await readJson(collRes, "Create payment collection");
    const collectionId = coll.payment_collection.id;

    const sessRes = await fetch(
      `/store/payment-collections/${collectionId}/payment-sessions`,
      {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ provider_id: "mollie" }),
      }
    );
    const sess = await readJson(sessRes, "Create payment session");
    const ps = sess.payment_collection.payment_sessions[0];
    const klarnaSessionId = ps.id;

    // Persist the Klarna billing + method on the (EUR) session.
    const reinitRes = await fetch(
      `/store/payment-sessions/${klarnaSessionId}/reinitiate`,
      {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          data: {
            paymentMethodType: "klarna",
            billing,
            id: ps.data?.id,
            returnUrl: `${window.location.origin}/order/${klarnaSessionId}`,
          },
        }),
      }
    );
    await readJson(reinitRes, "Persist Klarna billing");

    // Authorize (cart complete) → Klarna hosted-checkout redirect.
    const res = await fetch(`/store/carts/${CART.cartId}/complete`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({}),
    });
    const body = await res.json().catch(() => ({}));
    if (!res.ok) {
      throw new Error(body.error ?? `Authorize failed (${res.status})`);
    }
    if (body.redirectUrl) {
      window.location.assign(body.redirectUrl);
      return;
    }
    window.location.assign(`/order/${klarnaSessionId}`);
  };

  return (
    <div>
      <div style={{ display: "flex", gap: 8, marginBottom: 16 }}>
        {(["card", "klarna"] as const).map((m) => (
          <button
            key={m}
            type="button"
            data-testid={`mollie-method-${m}`}
            onClick={() => setMethod(m)}
            style={{
              flex: 1,
              padding: "10px 12px",
              borderRadius: 6,
              border: method === m ? "2px solid #0b051d" : "1px solid #ddd",
              background: method === m ? "#f5f5f5" : "#fff",
              fontWeight: method === m ? 600 : 400,
              cursor: "pointer",
            }}
          >
            {m === "card" ? "Card" : "Klarna (Pay later)"}
          </button>
        ))}
      </div>

      {method === "card" ? (
        <MollieWrapper
          profileId={sessionData.profileId ?? ""}
          testmode
          onError={onError}
          onSubmit={async ({ cardToken }) => {
            const reinitRes = await fetch(
              `/store/payment-sessions/${sessionId}/reinitiate`,
              {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({
                  data: {
                    cardToken,
                    id: sessionData.id,
                    returnUrl: `${window.location.origin}/order/${sessionId}`,
                  },
                }),
              }
            );
            await readJson(reinitRes, "Persist card token");
            const res = await fetch(`/store/carts/${CART.cartId}/complete`, {
              method: "POST",
              headers: { "Content-Type": "application/json" },
              body: JSON.stringify({}),
            });
            const body = await res.json().catch(() => ({}));
            if (!res.ok) {
              throw new Error(body.error ?? `Authorize failed (${res.status})`);
            }
            if (body.redirectUrl) {
              window.location.assign(body.redirectUrl);
              return;
            }
            window.location.assign(`/order/${sessionId}`);
          }}
        />
      ) : (
        <MollieKlarnaForm
          amount={CART.amount}
          currency="EUR"
          onError={onError}
          onSubmit={payKlarna}
        />
      )}
    </div>
  );
}
