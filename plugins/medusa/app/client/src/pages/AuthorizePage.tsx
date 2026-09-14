import { useEffect } from "react";
import { useNavigate } from "react-router-dom";

const CART_ID = "cart_test_01";

export default function AuthorizePage() {
  const navigate = useNavigate();

  useEffect(() => {
    fetch(`/store/carts/${CART_ID}/complete`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({}),
    })
      .then((res) => res.json())
      .then((body) => {
        if (body.order?.payment_session_id) {
          navigate(`/order/${body.order.payment_session_id}`, { replace: true });
        } else {
          navigate("/", { replace: true });
        }
      })
      .catch(() => navigate("/", { replace: true }));
  }, [navigate]);

  return (
    <div style={{ maxWidth: 560, margin: "60px auto", padding: "0 16px", textAlign: "center" }}>
      <p data-testid="authorizing" style={{ color: "#555", fontSize: 16 }}>Authorising payment…</p>
    </div>
  );
}
