import { useEffect, useCallback, useState } from "react";
import { useParams, Link } from "react-router-dom";

const CART = { amount: 100, currency: "USD" };

type Payment = {
  id: string;
  connector: string;
  status: string;
  connectorTransactionId?: string;
};

type ActionResult = {
  loading: boolean;
  ok?: boolean;
  message?: string;
};

async function fetchPayment(sessionId: string): Promise<Payment> {
  const res = await fetch(`/admin/payments/${sessionId}`);
  const body = await res.json();
  if (!res.ok) throw new Error(body.error ?? `Failed to fetch payment (${res.status})`);
  return body.payment as Payment;
}

async function adminCapture(sessionId: string): Promise<ActionResult> {
  const res = await fetch(`/admin/payments/${sessionId}/capture`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({}),
  });
  const body = await res.json();
  if (!res.ok) return { loading: false, ok: false, message: body.error ?? "Capture failed" };
  return { loading: false, ok: true, message: `Captured (status: ${body.payment?.status})` };
}

async function adminVoid(sessionId: string): Promise<ActionResult> {
  const res = await fetch(`/admin/payments/${sessionId}/cancel`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({}),
  });
  const body = await res.json();
  if (!res.ok) return { loading: false, ok: false, message: body.error ?? "Void failed" };
  return { loading: false, ok: true, message: `Voided (status: ${body.payment?.status})` };
}

async function adminRefund(sessionId: string): Promise<ActionResult> {
  const res = await fetch(`/admin/payments/${sessionId}/refund`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ amount: CART.amount }),
  });
  const body = await res.json();
  if (!res.ok) return { loading: false, ok: false, message: body.error ?? "Refund failed" };
  const status = body.refund?.status;
  return {
    loading: false,
    ok: true,
    message: `Refund of $${CART.amount} ${CART.currency} submitted (status: ${status})`,
  };
}

export default function OrderPage() {
  const { sessionId } = useParams<{ sessionId: string }>();

  const [payment, setPayment] = useState<Payment | null>(null);
  const [fetchLoading, setFetchLoading] = useState(true);
  const [fetchError, setFetchError] = useState<string | null>(null);

  const [captureResult, setCaptureResult] = useState<ActionResult | null>(null);
  const [voidResult, setVoidResult] = useState<ActionResult | null>(null);
  const [refundResult, setRefundResult] = useState<ActionResult | null>(null);

  useEffect(() => {
    if (!sessionId) return;
    fetchPayment(sessionId)
      .then(setPayment)
      .catch((err) => setFetchError((err as Error).message))
      .finally(() => setFetchLoading(false));
  }, [sessionId]);

  const effectiveStatus = (() => {
    if (!payment) return null;
    if (captureResult?.ok) return "captured";
    if (voidResult?.ok) return "canceled";
    return payment.status;
  })();

  const handleCapture = useCallback(async () => {
    if (!sessionId) return;
    setCaptureResult({ loading: true });
    setCaptureResult(await adminCapture(sessionId));
  }, [sessionId]);

  const handleVoid = useCallback(async () => {
    if (!sessionId) return;
    setVoidResult({ loading: true });
    setVoidResult(await adminVoid(sessionId));
  }, [sessionId]);

  const handleRefund = useCallback(async () => {
    if (!sessionId) return;
    setRefundResult({ loading: true });
    setRefundResult(await adminRefund(sessionId));
  }, [sessionId]);

  return (
    <div style={{ maxWidth: 560, margin: "60px auto", padding: "0 16px" }}>
      <Link to="/" style={{ color: "#555", textDecoration: "none", fontSize: 14 }}>
        ← Back
      </Link>

      <h2 style={{ fontSize: 22, fontWeight: 700, margin: "16px 0 4px" }}>Order</h2>
      <p style={{ color: "#666", fontSize: 13, marginBottom: 28 }}>
        Session: <code>{sessionId}</code>
      </p>

      {fetchLoading && <p style={{ color: "#555" }}>Loading payment status…</p>}

      {fetchError && (
        <div style={{ background: "#fff0f0", border: "1px solid #fcc", borderRadius: 6, padding: "12px 16px", color: "#c00" }}>
          <strong>Error:</strong> {fetchError}
        </div>
      )}

      {payment && effectiveStatus && (
        <PaymentPanel
          status={effectiveStatus}
          sessionId={payment.id}
          connector={payment.connector}
          captureResult={captureResult}
          voidResult={voidResult}
          refundResult={refundResult}
          onCapture={handleCapture}
          onVoid={handleVoid}
          onRefund={handleRefund}
        />
      )}
    </div>
  );
}

interface PaymentPanelProps {
  status: string;
  sessionId: string;
  connector: string;
  captureResult: ActionResult | null;
  voidResult: ActionResult | null;
  refundResult: ActionResult | null;
  onCapture: () => void;
  onVoid: () => void;
  onRefund: () => void;
}

function PaymentPanel({
  status,
  sessionId,
  connector,
  captureResult,
  voidResult,
  refundResult,
  onCapture,
  onVoid,
  onRefund,
}: PaymentPanelProps) {
  const isAuthorized = status === "authorized" || status === "AUTHORIZED";
  const isCaptured = status === "captured" || status === "succeeded" || status === "CAPTURED";
  const isCanceled = status === "canceled" || status === "CANCELED";
  const isFailed = status === "error" || status === "failed" || status === "FAILED";

  const captureDone = captureResult?.ok === true;
  const voidDone = voidResult?.ok === true;
  const refundDone = refundResult?.ok === true;

  return (
    <div
      style={{
        background: isCanceled || isFailed ? "#fffbeb" : "#f0fff4",
        border: `1px solid ${isCanceled || isFailed ? "#f6e05e" : "#a3e6b8"}`,
        borderRadius: 8,
        padding: "28px 24px",
      }}
    >
      <div style={{ fontSize: 40, lineHeight: 1, textAlign: "center" }}>
        {isCanceled ? "🚫" : isFailed ? "❌" : "✅"}
      </div>
      <h3
        style={{
          fontSize: 20,
          fontWeight: 700,
          margin: "12px 0 4px",
          color: isCanceled || isFailed ? "#744210" : "#276749",
          textAlign: "center",
        }}
      >
        {isCanceled ? "Payment voided" : isFailed ? "Payment failed" : "Payment successful"}
      </h3>
      <p style={{ color: isCanceled || isFailed ? "#744210" : "#276749", fontSize: 13, margin: "0 0 20px", textAlign: "center" }}>
        ${CART.amount} {CART.currency} · connector: <code data-testid="order-connector">{connector}</code> · status: <code data-testid="order-status">{status}</code>
      </p>

      <div
        style={{
          background: "#fff",
          border: "1px solid #e2e8f0",
          borderRadius: 6,
          padding: "10px 12px",
          marginBottom: 20,
          fontSize: 13,
          color: "#333",
        }}
      >
        Session ID: <code style={{ fontWeight: 600 }}>{sessionId}</code>
      </div>

      {/* Action results */}
      {[captureResult, voidResult, refundResult].map((r, i) =>
        r?.message ? (
          <div
            key={i}
            style={{
              padding: "10px 12px",
              borderRadius: 6,
              marginBottom: 12,
              fontSize: 13,
              background: r.ok ? "#ebf8ff" : "#fff0f0",
              border: r.ok ? "1px solid #90cdf4" : "1px solid #fcc",
              color: r.ok ? "#2c5282" : "#c00",
            }}
          >
            {r.message}
          </div>
        ) : null
      )}

      {/* Authorized: Capture + Void + Refund */}
      {isAuthorized && !captureDone && !voidDone && (
        <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
          <ActionButton
            testid="capture-button"
            label="Capture"
            loadingLabel="Capturing…"
            loading={captureResult?.loading === true}
            done={captureDone}
            onClick={onCapture}
            color="#276749"
            bg="#f0fff4"
            border="#a3e6b8"
          />
          <ActionButton
            testid="void-button"
            label="Void"
            loadingLabel="Voiding…"
            loading={voidResult?.loading === true}
            done={voidDone}
            onClick={onVoid}
            color="#744210"
            bg="#fffbeb"
            border="#f6e05e"
          />
          <ActionButton
            testid="refund-button"
            label={`Refund $${CART.amount}`}
            loadingLabel="Refunding…"
            loading={refundResult?.loading === true}
            done={refundDone}
            onClick={onRefund}
            color="#c53030"
            bg="#fff5f5"
            border="#fcc"
          />
        </div>
      )}

      {/* Captured: Refund only */}
      {isCaptured && (
        <ActionButton
          testid="refund-button"
          label={`Refund $${CART.amount} ${CART.currency}`}
          loadingLabel="Refunding…"
          loading={refundResult?.loading === true}
          done={refundDone}
          onClick={onRefund}
          color="#c53030"
          bg="#fff5f5"
          border="#fcc"
        />
      )}
    </div>
  );
}

interface ActionButtonProps {
  label: string;
  loadingLabel: string;
  loading: boolean;
  done: boolean;
  onClick: () => void;
  color: string;
  bg: string;
  border: string;
  testid?: string;
}

function ActionButton({ label, loadingLabel, loading, done, onClick, color, bg, border, testid }: ActionButtonProps) {
  return (
    <button
      type="button"
      data-testid={testid}
      onClick={onClick}
      disabled={loading || done}
      style={{
        padding: "10px 20px",
        borderRadius: 6,
        border: `1px solid ${done ? "#ccc" : border}`,
        background: done ? "#eee" : bg,
        color: done ? "#999" : color,
        fontWeight: 600,
        fontSize: 14,
        cursor: loading || done ? "not-allowed" : "pointer",
        flex: "1 1 auto",
      }}
    >
      {loading ? loadingLabel : done ? `${label} ✓` : label}
    </button>
  );
}
