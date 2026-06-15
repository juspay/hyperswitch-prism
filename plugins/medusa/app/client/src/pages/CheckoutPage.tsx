import { useEffect, useCallback, useState } from "react";
import { useParams, Link, useNavigate } from "react-router-dom";

import {
  StripeWrapper,
  AdyenWrapper,
  PayPalWrapper,
  GlobalPayWrapper,
} from "@juspay/medusa-unified-payment-react";

// The Stripe publishable key and Adyen client key are delivered by the server
// in the payment session (sessionData.publishableKey), sourced from creds.json.

const CART = { cartId: "cart_test_01", amount: 100, currency: "USD" };

const CONNECTOR_LABELS: Record<string, string> = {
  stripe: "Stripe",
  adyen: "Adyen",
  paypal: "PayPal",
  globalpay: "GlobalPay",
};

const SUPPORTED = ["stripe", "adyen", "paypal", "globalpay"];

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

    default:
      return null;
  }
}
