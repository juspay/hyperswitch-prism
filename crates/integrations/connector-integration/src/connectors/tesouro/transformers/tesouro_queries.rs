//! GraphQL query/mutation strings for the Tesouro connector.
//!
//! Kept in a dedicated module (mirroring the Hyperswitch `tesouro_queries` layout) so the
//! transformer logic in `transformers.rs` stays readable and the raw GraphQL documents live
//! in one discoverable place.

/// GraphQL mutation for the SetupMandate flow. `verifyAccount` runs a zero-amount
/// account verification and, on success, returns a stored-credential token used as the
/// mandate id for subsequent MITs.
pub const SETUP_MANDATE: &str = "mutation VerifyAccount(
    $verifyAccountInput: VerifyAccountInput!
) {
    verifyAccount(
        verifyAccountInput: $verifyAccountInput
    ) {
        verifyAccountResponse {
            paymentId
            transactionId
            tokenDetails {
                token
            }
            activityDate
        }
        errors {
            ... on InternalServiceError { message transactionId processorResponseCode }
            ... on AcceptorNotFoundError { message transactionId processorResponseCode }
            ... on RuleInViolationError { message transactionId processorResponseCode }
            ... on SyntaxOnNetworkResponseError { message transactionId processorResponseCode }
            ... on TimeoutOnNetworkResponseError { message transactionId processorResponseCode }
            ... on ValidationFailureError { message processorResponseCode transactionId }
            ... on UnknownCardError { message processorResponseCode transactionId }
            ... on TokenNotFoundError { message processorResponseCode transactionId }
            ... on InvalidTokenError { message processorResponseCode transactionId }
            ... on RouteNotFoundError { message processorResponseCode transactionId }
        }
    }
}";

/// GraphQL mutation for the Authorize flow (customer-initiated / CIT). Returns an
/// approval or decline; when storing a mandate it also returns the stored-credential token.
pub const AUTHORIZE_TRANSACTION: &str = "mutation AuthorizeCustomerInitiatedTransaction(
    $authorizeCustomerInitiatedTransactionInput: AuthorizeCustomerInitiatedTransactionInput!
) {
    authorizeCustomerInitiatedTransaction(
        authorizeCustomerInitiatedTransactionInput: $authorizeCustomerInitiatedTransactionInput
    ) {
        authorizationResponse {
            paymentId
            transactionId
            tokenDetails { token }
            activityDate
            __typename
            ... on AuthorizationApproval {
                __typename paymentId transactionId tokenDetails { token } activityDate
            }
            ... on AuthorizationDecline {
                __typename transactionId paymentId message tokenDetails { token }
            }
        }
        errors {
            ... on InternalServiceError { message transactionId processorResponseCode }
            ... on AcceptorNotFoundError { message transactionId processorResponseCode }
            ... on RuleInViolationError { message transactionId processorResponseCode }
            ... on SyntaxOnNetworkResponseError { message transactionId processorResponseCode }
            ... on TimeoutOnNetworkResponseError { message transactionId processorResponseCode }
            ... on ValidationFailureError { message processorResponseCode transactionId }
            ... on UnknownCardError { message processorResponseCode transactionId }
            ... on TokenNotFoundError { message processorResponseCode transactionId }
            ... on InvalidTokenError { message processorResponseCode transactionId }
            ... on RouteNotFoundError { message processorResponseCode transactionId }
        }
    }
}";

/// GraphQL mutation for the RepeatPayment flow (merchant-initiated / MIT). Charges a
/// stored-credential acquirer token obtained during the SetupMandate/Authorize leg.
pub const AUTHORIZE_RECURRING: &str = "mutation AuthorizeRecurring(
    $authorizeRecurringInput: AuthorizeRecurringInput!
) {
    authorizeRecurring(
        authorizeRecurringInput: $authorizeRecurringInput
    ) {
        authorizationResponse {
            paymentId
            transactionId
            tokenDetails { token }
            activityDate
            __typename
            ... on AuthorizationApproval {
                __typename paymentId transactionId tokenDetails { token } activityDate
            }
            ... on AuthorizationDecline {
                __typename transactionId paymentId message
            }
        }
        errors {
            ... on InternalServiceError { message transactionId processorResponseCode }
            ... on AcceptorNotFoundError { message transactionId processorResponseCode }
            ... on RuleInViolationError { message transactionId processorResponseCode }
            ... on SyntaxOnNetworkResponseError { message transactionId processorResponseCode }
            ... on TimeoutOnNetworkResponseError { message transactionId processorResponseCode }
            ... on ValidationFailureError { message processorResponseCode transactionId }
            ... on UnknownCardError { message processorResponseCode transactionId }
            ... on TokenNotFoundError { message processorResponseCode transactionId }
            ... on InvalidTokenError { message processorResponseCode transactionId }
            ... on RouteNotFoundError { message processorResponseCode transactionId }
            ... on PriorPaymentNotFoundError { message processorResponseCode transactionId }
        }
    }
}";

/// GraphQL query for the PSync flow. Fetches a payment transaction by id; the response
/// `__typename` (e.g. `ApprovedAuthorization`, `Sale`, `DeclinedAuthorization`) drives the
/// attempt-status mapping in `map_sync_status`.
pub const SYNC_TRANSACTION: &str = "query PaymentTransaction($paymentTransactionId: UUID!) {
  paymentTransaction(id: $paymentTransactionId) {
    __typename
    responseType
    reference
    id
    paymentId
    ... on AcceptedSale { __typename id processorResponseCode processorResponseMessage }
    ... on ApprovedAuthorization { __typename id processorResponseCode processorResponseMessage }
    ... on ApprovedCapture { __typename id processorResponseCode processorResponseMessage }
    ... on ApprovedReversal { __typename id processorResponseCode processorResponseMessage }
    ... on DeclinedAuthorization { __typename id processorResponseCode processorResponseMessage }
    ... on DeclinedCapture { __typename id processorResponseCode processorResponseMessage }
    ... on DeclinedReversal { __typename id processorResponseCode processorResponseMessage }
    ... on GenericPaymentTransaction { __typename id processorResponseCode processorResponseMessage }
    ... on Authorization { __typename id processorResponseCode processorResponseMessage }
    ... on Capture { __typename id processorResponseCode processorResponseMessage }
    ... on Reversal { __typename id processorResponseCode processorResponseMessage }
    ... on Sale { __typename id processorResponseCode processorResponseMessage }
  }
}";
