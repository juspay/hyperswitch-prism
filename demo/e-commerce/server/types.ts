// API Request/Response types

export interface SdkSessionRequest {
  currency: string;
  amount: number;
}

export interface SdkSessionResponse {
  connector: string;
  clientToken: string;
  publishableKey: string;
  sessionData: Record<string, unknown>;
  merchantTransactionId: string;
}

export interface TokenAuthorizeRequest {
  token: string;
  merchantTransactionId: string;
  amount: number;
  currency: string;
}

export interface PaymentResponse {
  status: number;
  connectorTransactionId?: string;
  error?: string;
}

export interface RefundRequest {
  connectorTransactionId: string;
  refundAmount: number;
  currency: string;
  merchantRefundId: string;
}

export interface RefundResponse {
  status: number;
  refundId?: string;
  error?: string;
}

export interface PaymentStatusResponse {
  status: number;
  connectorTransactionId?: string;
  error?: string;
}

// Product types
export interface Product {
  id: string;
  name: string;
  price: number;
  currency: string;
  description: string;
  image: string;
}

// Cart types
export interface CartItem {
  product: Product;
  quantity: number;
}

export interface Cart {
  items: CartItem[];
  currency: string;
  totalAmount: number;
}