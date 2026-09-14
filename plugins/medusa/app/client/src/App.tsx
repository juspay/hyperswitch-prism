import { Routes, Route } from "react-router-dom";
import Home from "./pages/Home";
import CheckoutPage from "./pages/CheckoutPage";
import AuthorizePage from "./pages/AuthorizePage";
import OrderPage from "./pages/OrderPage";

export default function App() {
  return (
    <Routes>
      <Route path="/" element={<Home />} />
      <Route path="/:connector" element={<CheckoutPage />} />
      <Route path="/authorize" element={<AuthorizePage />} />
      <Route path="/order/:sessionId" element={<OrderPage />} />
    </Routes>
  );
}
