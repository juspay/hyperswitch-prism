import { Link } from "react-router-dom";

const CONNECTORS = [
  "stripe",
  "adyen",
  "paypal",
  "globalpay",
  "mollie",
  "razorpay",
] as const;

const LABELS: Record<string, string> = {
  stripe: "Stripe",
  adyen: "Adyen",
  paypal: "PayPal",
  globalpay: "GlobalPay",
  mollie: "Mollie",
  razorpay: "Razorpay",
};

export default function Home() {
  return (
    <div style={{ maxWidth: 480, margin: "80px auto", padding: "0 16px" }}>
      <h1 style={{ fontSize: 24, fontWeight: 700, marginBottom: 8 }}>
        Payment UI Test Client
      </h1>
      <p style={{ color: "#666", marginBottom: 32 }}>
        Select a connector to test its checkout element. All payments use hardcoded
        cart data: <strong>$100 USD</strong>, cart ID <code>cart_test_01</code>.
      </p>
      <div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
        {CONNECTORS.map((c) => (
          <Link
            key={c}
            to={`/${c}`}
            style={{
              display: "block",
              padding: "14px 20px",
              background: "#fff",
              border: "1px solid #ddd",
              borderRadius: 8,
              textDecoration: "none",
              color: "#222",
              fontWeight: 500,
              fontSize: 16,
              transition: "box-shadow 0.15s",
            }}
          >
            {LABELS[c]} →
          </Link>
        ))}
      </div>
    </div>
  );
}
