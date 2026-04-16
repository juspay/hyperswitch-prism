import { types } from 'hyperswitch-prism';

const { PaymentStatus } = types;

/**
 * Maps PaymentStatus enum values to human-readable strings
 */
export const paymentStatusMap: Record<number, string> = {
  [PaymentStatus.PAYMENT_STATUS_UNSPECIFIED]: 'UNSPECIFIED',
  [PaymentStatus.STARTED]: 'STARTED',
  [PaymentStatus.PAYMENT_METHOD_AWAITED]: 'PAYMENT_METHOD_AWAITED',
  [PaymentStatus.DEVICE_DATA_COLLECTION_PENDING]: 'DEVICE_DATA_COLLECTION_PENDING',
  [PaymentStatus.CONFIRMATION_AWAITED]: 'CONFIRMATION_AWAITED',
  [PaymentStatus.AUTHENTICATION_PENDING]: 'AUTHENTICATION_PENDING',
  [PaymentStatus.AUTHENTICATION_SUCCESSFUL]: 'AUTHENTICATION_SUCCESSFUL',
  [PaymentStatus.AUTHENTICATION_FAILED]: 'AUTHENTICATION_FAILED',
  [PaymentStatus.AUTHORIZING]: 'AUTHORIZING',
  [PaymentStatus.AUTHORIZED]: 'AUTHORIZED',
  [PaymentStatus.AUTHORIZATION_FAILED]: 'AUTHORIZATION_FAILED',
  [PaymentStatus.PARTIALLY_AUTHORIZED]: 'PARTIALLY_AUTHORIZED',
  [PaymentStatus.CHARGED]: 'CHARGED',
  [PaymentStatus.PARTIAL_CHARGED]: 'PARTIAL_CHARGED',
  [PaymentStatus.PARTIAL_CHARGED_AND_CHARGEABLE]: 'PARTIAL_CHARGED_AND_CHARGEABLE',
  [PaymentStatus.AUTO_REFUNDED]: 'AUTO_REFUNDED',
  [PaymentStatus.CAPTURE_INITIATED]: 'CAPTURE_INITIATED',
  [PaymentStatus.CAPTURE_FAILED]: 'CAPTURE_FAILED',
  [PaymentStatus.VOID_INITIATED]: 'VOID_INITIATED',
  [PaymentStatus.VOIDED]: 'VOIDED',
  [PaymentStatus.VOID_FAILED]: 'VOID_FAILED',
  [PaymentStatus.VOIDED_POST_CAPTURE]: 'VOIDED_POST_CAPTURE',
  [PaymentStatus.COD_INITIATED]: 'COD_INITIATED',
  [PaymentStatus.EXPIRED]: 'EXPIRED',
  [PaymentStatus.ROUTER_DECLINED]: 'ROUTER_DECLINED',
  [PaymentStatus.PENDING]: 'PENDING',
  [PaymentStatus.FAILURE]: 'FAILURE',
  [PaymentStatus.UNRESOLVED]: 'UNRESOLVED'
};

/**
 * Gets the human-readable status text for a PaymentStatus enum value
 * @param status - The PaymentStatus enum value
 * @returns The human-readable status string
 */
export function getPaymentStatusText(status: number): string {
  return paymentStatusMap[status] || 'UNKNOWN';
}
