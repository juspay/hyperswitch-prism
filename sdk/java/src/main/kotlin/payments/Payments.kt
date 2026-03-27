/**
 * Re-exports for payment request/response types and enums.
 *
 * Usage:
 *   import payments.PaymentServiceAuthorizeRequest
 *   import payments.Currency
 *
 * Mirrors the JavaScript `payments` namespace and Python `PaymentsNamespace`.
 */
@file:Suppress("unused")

package payments

// ---------------------------------------------------------------------------
// Request / Response types
// ---------------------------------------------------------------------------
typealias PaymentServiceAuthorizeRequest = types.Payment.PaymentServiceAuthorizeRequest
typealias PaymentServiceAuthorizeResponse = types.Payment.PaymentServiceAuthorizeResponse
typealias PaymentServiceCaptureRequest = types.Payment.PaymentServiceCaptureRequest
typealias PaymentServiceCaptureResponse = types.Payment.PaymentServiceCaptureResponse
typealias PaymentServiceVoidRequest = types.Payment.PaymentServiceVoidRequest
typealias PaymentServiceVoidResponse = types.Payment.PaymentServiceVoidResponse
typealias PaymentServiceRefundRequest = types.Payment.PaymentServiceRefundRequest
typealias RefundResponse = types.Payment.RefundResponse
typealias PaymentServiceReverseRequest = types.Payment.PaymentServiceReverseRequest
typealias PaymentServiceReverseResponse = types.Payment.PaymentServiceReverseResponse
typealias PaymentServiceGetRequest = types.Payment.PaymentServiceGetRequest
typealias PaymentServiceGetResponse = types.Payment.PaymentServiceGetResponse
typealias PaymentServiceCreateOrderRequest = types.Payment.PaymentServiceCreateOrderRequest
typealias PaymentServiceCreateOrderResponse = types.Payment.PaymentServiceCreateOrderResponse
typealias PaymentServiceSetupRecurringRequest = types.Payment.PaymentServiceSetupRecurringRequest
typealias PaymentServiceSetupRecurringResponse = types.Payment.PaymentServiceSetupRecurringResponse
typealias PaymentServiceIncrementalAuthorizationRequest = types.Payment.PaymentServiceIncrementalAuthorizationRequest
typealias PaymentServiceIncrementalAuthorizationResponse = types.Payment.PaymentServiceIncrementalAuthorizationResponse
typealias PaymentServiceVerifyRedirectResponseRequest = types.Payment.PaymentServiceVerifyRedirectResponseRequest
typealias PaymentServiceVerifyRedirectResponseResponse = types.Payment.PaymentServiceVerifyRedirectResponseResponse
typealias PaymentServiceDisputeRequest = types.Payment.PaymentServiceDisputeRequest
typealias DisputeResponse = types.Payment.DisputeResponse
typealias DisputeServiceAcceptRequest = types.Payment.DisputeServiceAcceptRequest
typealias DisputeServiceAcceptResponse = types.Payment.DisputeServiceAcceptResponse
typealias DisputeServiceDefendRequest = types.Payment.DisputeServiceDefendRequest
typealias DisputeServiceDefendResponse = types.Payment.DisputeServiceDefendResponse
typealias DisputeServiceSubmitEvidenceRequest = types.Payment.DisputeServiceSubmitEvidenceRequest
typealias DisputeServiceSubmitEvidenceResponse = types.Payment.DisputeServiceSubmitEvidenceResponse

// Authentication service
typealias MerchantAuthenticationServiceCreateAccessTokenRequest = types.Payment.MerchantAuthenticationServiceCreateAccessTokenRequest
typealias MerchantAuthenticationServiceCreateAccessTokenResponse = types.Payment.MerchantAuthenticationServiceCreateAccessTokenResponse
typealias MerchantAuthenticationServiceCreateSessionTokenRequest = types.Payment.MerchantAuthenticationServiceCreateSessionTokenRequest
typealias MerchantAuthenticationServiceCreateSessionTokenResponse = types.Payment.MerchantAuthenticationServiceCreateSessionTokenResponse
typealias MerchantAuthenticationServiceCreateSdkSessionTokenRequest = types.Payment.MerchantAuthenticationServiceCreateSdkSessionTokenRequest
typealias MerchantAuthenticationServiceCreateSdkSessionTokenResponse = types.Payment.MerchantAuthenticationServiceCreateSdkSessionTokenResponse

// Payment method authentication
typealias PaymentMethodAuthenticationServicePreAuthenticateRequest = types.Payment.PaymentMethodAuthenticationServicePreAuthenticateRequest
typealias PaymentMethodAuthenticationServicePreAuthenticateResponse = types.Payment.PaymentMethodAuthenticationServicePreAuthenticateResponse
typealias PaymentMethodAuthenticationServiceAuthenticateRequest = types.Payment.PaymentMethodAuthenticationServiceAuthenticateRequest
typealias PaymentMethodAuthenticationServiceAuthenticateResponse = types.Payment.PaymentMethodAuthenticationServiceAuthenticateResponse
typealias PaymentMethodAuthenticationServicePostAuthenticateRequest = types.Payment.PaymentMethodAuthenticationServicePostAuthenticateRequest
typealias PaymentMethodAuthenticationServicePostAuthenticateResponse = types.Payment.PaymentMethodAuthenticationServicePostAuthenticateResponse

// Tokenization
typealias PaymentMethodServiceTokenizeRequest = types.Payment.PaymentMethodServiceTokenizeRequest
typealias PaymentMethodServiceTokenizeResponse = types.Payment.PaymentMethodServiceTokenizeResponse

// Recurring payments
typealias RecurringPaymentServiceChargeRequest = types.Payment.RecurringPaymentServiceChargeRequest
typealias RecurringPaymentServiceChargeResponse = types.Payment.RecurringPaymentServiceChargeResponse

// Customer service
typealias CustomerServiceCreateRequest = types.Payment.CustomerServiceCreateRequest
typealias CustomerServiceCreateResponse = types.Payment.CustomerServiceCreateResponse

// ---------------------------------------------------------------------------
// Data types
// ---------------------------------------------------------------------------
typealias Money = types.Payment.Money
typealias ErrorInfo = types.Payment.ErrorInfo
typealias Customer = types.Payment.Customer
typealias PaymentAddress = types.Payment.PaymentAddress
typealias Address = types.Payment.Address
typealias Identifier = types.Payment.Identifier
typealias ConnectorState = types.Payment.ConnectorState
typealias AccessToken = types.Payment.AccessToken
typealias SecretString = types.PaymentMethods.SecretString
typealias BrowserInformation = types.Payment.BrowserInformation
typealias CustomerAcceptance = types.Payment.CustomerAcceptance
typealias SessionToken = types.Payment.SessionToken
typealias ConnectorResponseData = types.Payment.ConnectorResponseData
typealias CardConnectorResponse = types.Payment.CardConnectorResponse
typealias AuthenticationData = types.Payment.AuthenticationData
typealias Metadata = types.Payment.Metadata
typealias ConnectorSpecificConfig = types.Payment.ConnectorSpecificConfig

// ---------------------------------------------------------------------------
// Enums
// ---------------------------------------------------------------------------
typealias Currency = types.Payment.Currency
typealias CaptureMethod = types.Payment.CaptureMethod
typealias AuthenticationType = types.Payment.AuthenticationType
typealias PaymentMethodType = types.Payment.PaymentMethodType
typealias PaymentStatus = types.Payment.PaymentStatus
typealias RefundStatus = types.Payment.RefundStatus
typealias DisputeStatus = types.Payment.DisputeStatus
typealias MandateStatus = types.Payment.MandateStatus
typealias AuthorizationStatus = types.Payment.AuthorizationStatus
typealias OperationStatus = types.Payment.OperationStatus
typealias HttpMethod = types.Payment.HttpMethod
typealias FutureUsage = types.Payment.FutureUsage
typealias PaymentExperience = types.Payment.PaymentExperience
typealias PaymentChannel = types.Payment.PaymentChannel
typealias Connector = types.Payment.Connector
typealias ProductType = types.Payment.ProductType
typealias DisputeStage = types.Payment.DisputeStage
typealias Tokenization = types.Payment.Tokenization
typealias WebhookEventType = types.Payment.WebhookEventType
typealias ThreeDsCompletionIndicator = types.Payment.ThreeDsCompletionIndicator
typealias TransactionStatus = types.Payment.TransactionStatus
typealias ExemptionIndicator = types.Payment.ExemptionIndicator
typealias MitCategory = types.Payment.MitCategory
typealias SyncRequestType = types.Payment.SyncRequestType
typealias AcceptanceType = types.Payment.AcceptanceType
typealias CavvAlgorithm = types.Payment.CavvAlgorithm
