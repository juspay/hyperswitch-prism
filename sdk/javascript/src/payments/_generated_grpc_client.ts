// AUTO-GENERATED — do not edit by hand.
// Source: services.proto  |  Regenerate: make generate  (or: python3 scripts/generators/code/generate.py --lang javascript)

import koffi from "koffi";
import path from "path";
// @ts-ignore - generated CommonJS module
import { types } from "./generated/proto.js";

// Standard Node.js __dirname
declare const __dirname: string;
const _dirname = __dirname;

// ── Config ────────────────────────────────────────────────────────────────────

/**
 * Connection configuration for the gRPC client.
 * Field names must be snake_case — they are serialised to JSON and sent to the
 * Rust FFI layer which deserialises them into GrpcConfigInput.
 * 
 * The connector_config field should contain the connector-specific authentication
 * and configuration in the format expected by the server:
 * {"config": {"ConnectorName": {"api_key": "...", ...}}}
 */
export interface GrpcConfig {
  endpoint: string;
  connector: string;
  connector_config: Record<string, unknown>;
}

// ── koffi FFI bindings ────────────────────────────────────────────────────────

interface GrpcFfi {
  call: (
    method:    string,
    configPtr: Buffer,
    configLen: number,
    reqPtr:    Buffer,
    reqLen:    number,
    outLen:    number[],
  ) => unknown; // opaque koffi pointer
  free: (ptr: unknown, len: number) => void;
}

function loadGrpcFfi(libPath?: string): GrpcFfi {
  if (!libPath) {
    const ext = process.platform === "darwin" ? "dylib" : "so";
    libPath = path.join(_dirname, "generated", `libhyperswitch_grpc_ffi.${ext}`);
  }

  const lib = koffi.load(libPath);

  const call = lib.func("hyperswitch_grpc_call",
    koffi.pointer("uint8"),
    [
      "str",                               // method (null-terminated C string)
      koffi.pointer("uint8"),              // config_ptr
      "uint32",                            // config_len
      koffi.pointer("uint8"),              // req_ptr
      "uint32",                            // req_len
      koffi.out(koffi.pointer("uint32")), // out_len (written by callee)
    ],
  );

  const free = lib.func("hyperswitch_grpc_free", "void", [
    koffi.pointer("uint8"),
    "uint32",
  ]);

  return { call, free };
}

// ── SecretString field normalization ─────────────────────────────────────────
// protobufjs fromObject requires SecretString fields to be {value: "..."} objects,
// but the field-probe JSON stores them as plain strings (Secret<String> serde format).
// These maps (generated from the proto descriptor) drive a type-aware pre-pass
// that wraps those strings before fromObject is called.

const _SECRET_STRING_FIELDS: Record<string, readonly string[]> = {
  TokenPaymentMethodType: ["token"],
  CardDetails: ["cardNumber", "cardExpMonth", "cardExpYear", "cardCvc", "cardHolderName"],
  ApplePayDecryptedData: ["applicationPrimaryAccountNumber", "applicationExpirationMonth", "applicationExpirationYear"],
  ApplePayCryptogramData: ["onlinePaymentCryptogram"],
  GooglePayDecryptedData: ["cardExpMonth", "cardExpYear", "applicationPrimaryAccountNumber", "cryptogram"],
  PaymentCredential: ["dpanLastFourDigits", "cardLastFourDigits"],
  TokenData: ["data"],
  PazeWallet: ["completeResponse"],
  PazeToken: ["paymentToken", "tokenExpirationMonth", "tokenExpirationYear", "paymentAccountReference"],
  PazeDynamicData: ["dynamicDataValue"],
  PazePhoneNumber: ["countryCode", "phoneNumber"],
  PazeConsumer: ["firstName", "lastName", "fullName", "emailAddress"],
  PazeAddress: ["name", "line1", "line2", "line3", "city", "state", "zip"],
  PazeDecryptedData: ["clientId"],
  MifinityWallet: ["dateOfBirth"],
  PaypalRedirectWallet: ["email"],
  ApplePayThirdPartySdkWallet: ["token"],
  GooglePayThirdPartySdkWallet: ["token"],
  PaypalSdkWallet: ["token"],
  UpiCollect: ["vpaId"],
  PixPayment: ["pixKey", "cpf", "cnpj"],
  OnlineBankingFinland: ["email"],
  Giropay: ["bankAccountBic", "bankAccountIban"],
  Interac: ["email"],
  BancontactCard: ["cardNumber", "cardExpMonth", "cardExpYear", "cardHolderName"],
  Becs: ["accountNumber", "bsbNumber", "bankAccountHolderName"],
  Ach: ["accountNumber", "routingNumber", "cardHolderName", "bankAccountHolderName"],
  Sepa: ["iban", "bankAccountHolderName"],
  Bacs: ["accountNumber", "sortCode", "bankAccountHolderName"],
  SepaGuaranteedDebit: ["iban", "bankAccountHolderName"],
  Givex: ["number", "cvc"],
  CardDetailsForNetworkTransactionId: ["cardNumber", "cardExpMonth", "cardExpYear", "nickName", "cardHolderName"],
  DecryptedWalletTokenDetailsForNetworkTransactionId: ["decryptedToken", "tokenExpMonth", "tokenExpYear", "cardHolderName"],
  NetworkTokenData: ["tokenNumber", "tokenExpMonth", "tokenExpYear", "tokenCryptogram", "nickName"],
  Metadata: ["general"],
  Customer: ["email"],
  Address: ["firstName", "lastName", "line1", "line2", "line3", "city", "state", "zipCode", "email", "phoneNumber"],
  AccessToken: ["token"],
  BillingDescriptor: ["name", "city", "phone"],
  NetworkTokenWithNTI: ["tokenExpMonth", "tokenExpYear"],
  TaxInfo: ["customerTaxRegistrationId", "merchantTaxRegistrationId"],
  CustomerInfo: ["customerName", "customerEmail", "customerPhoneNumber", "customerBankId", "customerBankName"],
  GpayTokenParameters: ["publicKey"],
  ConnectorSessionTokenResponse: ["clientSecret"],
  SecretInfoToInitiateSdk: ["display", "payment"],
  PaymentServiceAuthorizeRequest: ["metadata", "connectorFeatureData", "paymentMethodToken"],
  PaymentServiceAuthorizeResponse: ["rawConnectorResponse", "rawConnectorRequest", "connectorFeatureData"],
  PaymentServiceGetRequest: ["metadata", "connectorFeatureData"],
  PaymentServiceGetResponse: ["metadata", "rawConnectorResponse", "rawConnectorRequest"],
  PaymentServiceVoidRequest: ["metadata", "connectorFeatureData"],
  PaymentServiceVoidResponse: ["rawConnectorRequest", "connectorFeatureData"],
  PaymentServiceReverseRequest: ["metadata", "connectorFeatureData"],
  MerchantAuthenticationServiceCreateAccessTokenRequest: ["metadata", "connectorFeatureData"],
  MerchantAuthenticationServiceCreateAccessTokenResponse: ["accessToken"],
  MerchantAuthenticationServiceCreateSessionTokenRequest: ["metadata", "connectorFeatureData"],
  MerchantAuthenticationServiceCreateSdkSessionTokenRequest: ["metadata", "connectorFeatureData"],
  MerchantAuthenticationServiceCreateSdkSessionTokenResponse: ["rawConnectorResponse", "rawConnectorRequest"],
  PaymentServiceCaptureRequest: ["metadata", "connectorFeatureData"],
  PaymentServiceCaptureResponse: ["rawConnectorRequest", "connectorFeatureData"],
  PaymentServiceCreateOrderRequest: ["metadata", "connectorFeatureData"],
  PaymentServiceCreateOrderResponse: ["rawConnectorRequest", "rawConnectorResponse"],
  PaymentServiceRefundRequest: ["metadata", "refundMetadata", "connectorFeatureData"],
  RefundResponse: ["email", "metadata", "refundMetadata", "acquirerReferenceNumber", "rawConnectorResponse", "rawConnectorRequest"],
  DisputeResponse: ["rawConnectorRequest"],
  PaymentServiceSetupRecurringRequest: ["metadata", "connectorFeatureData", "paymentMethodToken", "connectorTestingData"],
  PaymentServiceSetupRecurringResponse: ["rawConnectorRequest", "connectorFeatureData"],
  RecurringPaymentServiceChargeRequest: ["metadata", "connectorFeatureData", "email", "merchantAccountId", "connectorTestingData"],
  RecurringPaymentServiceChargeResponse: ["connectorFeatureData", "rawConnectorResponse", "rawConnectorRequest"],
  RecurringPaymentServiceRevokeResponse: ["rawConnectorResponse", "rawConnectorRequest"],
  PaymentMethodAuthenticationServicePreAuthenticateRequest: ["metadata", "connectorFeatureData"],
  PaymentMethodAuthenticationServicePreAuthenticateResponse: ["connectorFeatureData", "rawConnectorResponse"],
  PaymentMethodAuthenticationServiceAuthenticateRequest: ["metadata", "connectorFeatureData"],
  PaymentMethodAuthenticationServiceAuthenticateResponse: ["connectorFeatureData", "rawConnectorResponse"],
  PaymentMethodAuthenticationServicePostAuthenticateRequest: ["metadata", "connectorFeatureData"],
  PaymentMethodAuthenticationServicePostAuthenticateResponse: ["connectorFeatureData", "rawConnectorResponse"],
  PaymentServiceIncrementalAuthorizationRequest: ["connectorFeatureData"],
  PaymentServiceVerifyRedirectResponseResponse: ["rawConnectorResponse"],
  RefundServiceGetRequest: ["refundMetadata", "connectorFeatureData"],
  DisputeServiceSubmitEvidenceResponse: ["rawConnectorRequest"],
  DisputeServiceDefendResponse: ["rawConnectorRequest"],
  DisputeServiceAcceptResponse: ["rawConnectorRequest"],
  PaymentMethodServiceTokenizeRequest: ["metadata", "connectorFeatureData"],
  CustomerServiceCreateRequest: ["email", "metadata", "connectorFeatureData"],
  CustomerServiceUpdateRequest: ["email", "metadata", "connectorFeatureData"],
  StripeConfig: ["apiKey"],
  BamboraConfig: ["merchantId", "apiKey"],
  CalidaConfig: ["apiKey"],
  CeleroConfig: ["apiKey"],
  NexinetsConfig: ["merchantId", "apiKey"],
  NexixpayConfig: ["apiKey"],
  RevolutConfig: ["secretApiKey", "signingSecret"],
  Revolv3Config: ["apiKey"],
  Shift4Config: ["apiKey"],
  StaxConfig: ["apiKey"],
  XenditConfig: ["apiKey"],
  HelcimConfig: ["apiKey"],
  MifinityConfig: ["key", "brandId", "destinationAccountNumber"],
  MultisafepayConfig: ["apiKey"],
  ScreenstreamConfig: ["apiKey"],
  AuthorizedotnetConfig: ["name", "transactionKey"],
  BraintreeConfig: ["publicKey", "privateKey", "merchantAccountId"],
  AirwallexConfig: ["apiKey", "clientId"],
  AuthipayConfig: ["apiKey", "apiSecret"],
  BillwerkConfig: ["apiKey", "publicApiKey"],
  BluesnapConfig: ["username", "password"],
  CashfreeConfig: ["appId", "secretKey"],
  CryptopayConfig: ["apiKey", "apiSecret"],
  DatatransConfig: ["merchantId", "password"],
  FiservemeaConfig: ["apiKey", "apiSecret"],
  GlobalpayConfig: ["appId", "appKey"],
  HipayConfig: ["apiKey", "apiSecret"],
  JpmorganConfig: ["clientId", "clientSecret", "companyName", "productName", "merchantPurchaseDescription", "statementDescriptor"],
  PaysafeCardAccountId: ["noThreeDs", "threeDs"],
  PaysafeAchAccountId: ["accountId"],
  LoonioConfig: ["merchantId", "merchantToken"],
  PaysafeConfig: ["username", "password"],
  PayuConfig: ["apiKey", "apiSecret"],
  PowertranzConfig: ["powerTranzId", "powerTranzPassword"],
  RapydConfig: ["accessKey", "secretKey"],
  WorldpayConfig: ["username", "password", "entityId", "merchantName"],
  AdyenConfig: ["apiKey", "merchantAccount", "reviewKey"],
  BankOfAmericaConfig: ["apiKey", "merchantAccount", "apiSecret"],
  BarclaycardConfig: ["apiKey", "merchantAccount", "apiSecret"],
  CybersourceConfig: ["apiKey", "merchantAccount", "apiSecret"],
  DlocalConfig: ["xLogin", "xTransKey", "secret"],
  ElavonConfig: ["sslMerchantId", "sslUserId", "sslPin"],
  FiservConfig: ["apiKey", "merchantAccount", "apiSecret", "terminalId"],
  GetnetConfig: ["apiKey", "apiSecret", "sellerId"],
  HyperpgConfig: ["username", "password", "merchantId"],
  IatapayConfig: ["clientId", "merchantId", "clientSecret"],
  NuveiConfig: ["merchantId", "merchantSiteId", "merchantSecret"],
  NovalnetConfig: ["productActivationKey", "paymentAccessKey", "tariffId"],
  NoonConfig: ["apiKey", "applicationIdentifier", "businessIdentifier"],
  RedsysConfig: ["merchantId", "terminalId", "sha256Pwd"],
  SilverflowConfig: ["apiKey", "apiSecret", "merchantAcceptorKey"],
  TrustpayConfig: ["apiKey", "projectId", "secretKey"],
  TrustpaymentsConfig: ["username", "password", "siteReference"],
  TsysConfig: ["deviceId", "transactionKey", "developerId"],
  WellsfargoConfig: ["apiKey", "merchantAccount", "apiSecret"],
  WorldpayvantivConfig: ["user", "password", "merchantId"],
  WorldpayxmlConfig: ["apiUsername", "apiPassword", "merchantCode"],
  ZiftConfig: ["userName", "password", "accountId"],
  FiservcommercehubConfig: ["apiKey", "secret", "merchantId", "terminalId"],
  GigadatConfig: ["campaignId", "accessToken", "securityToken"],
  PhonepeConfig: ["merchantId", "saltKey", "saltIndex"],
  ForteConfig: ["apiAccessId", "organizationId", "locationId", "apiSecretKey"],
  VoltConfig: ["username", "password", "clientId", "clientSecret"],
  PayboxConfig: ["site", "rank", "key", "merchantId"],
  PaytmConfig: ["merchantId", "merchantKey", "website", "clientId"],
  CashtocodeCurrencyAuthData: ["passwordClassic", "passwordEvoucher", "usernameClassic", "usernameEvoucher", "merchantIdClassic", "merchantIdEvoucher"],
  MollieConfig: ["apiKey", "profileToken"],
  NmiConfig: ["apiKey", "publicKey"],
  PaymeConfig: ["sellerPaymeId", "paymeClientKey"],
  PayloadCurrencyAuthData: ["apiKey", "processingAccountId"],
  EbanxConfig: ["apiKey"],
  FiuuConfig: ["merchantId", "verifyKey", "secretKey"],
  GlobepayConfig: ["apiKey"],
  CoinbaseConfig: ["apiKey"],
  CoingateConfig: ["apiKey"],
  PeachpaymentsConfig: ["apiKey", "tenantId", "clientMerchantReferenceId", "merchantPaymentMethodRouteId"],
  PaypalConfig: ["clientId", "clientSecret", "payerId"],
  TruelayerConfig: ["clientId", "clientSecret", "merchantAccountId", "accountHolderName", "privateKey", "kid"],
};

// {MessageName: {camelFieldName: NestedMessageTypeName}} — for recursive traversal
const _MSG_FIELD_TYPES: Record<string, Record<string, string>> = {
  PaymentMethod: { "card": "CardDetails", "cardRedirect": "CardRedirect", "cardProxy": "CardDetails", "token": "TokenPaymentMethodType", "applePay": "AppleWallet", "googlePay": "GoogleWallet", "applePayThirdPartySdk": "ApplePayThirdPartySdkWallet", "googlePayThirdPartySdk": "GooglePayThirdPartySdkWallet", "paypalSdk": "PaypalSdkWallet", "amazonPayRedirect": "AmazonPayRedirectWallet", "cashappQr": "CashappQrWallet", "paypalRedirect": "PaypalRedirectWallet", "weChatPayQr": "WeChatPayQrWallet", "aliPayRedirect": "AliPayRedirectWallet", "revolutPay": "RevolutPayWallet", "mifinity": "MifinityWallet", "bluecode": "Bluecode", "paze": "PazeWallet", "samsungPay": "SamsungWallet", "mbWay": "MBWay", "satispay": "Satispay", "wero": "Wero", "upiCollect": "UpiCollect", "upiIntent": "UpiIntent", "upiQr": "UpiQr", "onlineBankingThailand": "OnlineBankingThailand", "onlineBankingCzechRepublic": "OnlineBankingCzechRepublic", "onlineBankingFinland": "OnlineBankingFinland", "onlineBankingFpx": "OnlineBankingFPX", "onlineBankingPoland": "OnlineBankingPoland", "onlineBankingSlovakia": "OnlineBankingSlovakia", "openBankingUk": "OpenBankingUK", "openBankingPis": "OpenBankingPIS", "localBankRedirect": "LocalBankRedirect", "ideal": "Ideal", "sofort": "Sofort", "trustly": "Trustly", "giropay": "Giropay", "eps": "Eps", "przelewy24": "Przelewy24", "pse": "Pse", "bancontactCard": "BancontactCard", "blik": "Blik", "openBanking": "OpenBanking", "interac": "Interac", "bizum": "Bizum", "eft": "Eft", "duitNow": "DuitNow", "crypto": "CryptoCurrency", "classicReward": "ClassicReward", "eVoucher": "EVoucher", "instantBankTransfer": "InstantBankTransfer", "achBankTransfer": "AchBankTransfer", "sepaBankTransfer": "SepaBankTransfer", "bacsBankTransfer": "BacsBankTransfer", "multibancoBankTransfer": "MultibancoBankTransfer", "instantBankTransferFinland": "InstantBankTransferFinland", "instantBankTransferPoland": "InstantBankTransferPoland", "pix": "PixPayment", "permataBankTransfer": "PermataBankTransfer", "bcaBankTransfer": "BCABankTransfer", "bniVaBankTransfer": "BNIVaBankTransfer", "briVaBankTransfer": "BRIVaBankTransfer", "cimbVaBankTransfer": "CIMBVaBankTransfer", "danamonVaBankTransfer": "DanamonVaBankTransfer", "mandiriVaBankTransfer": "MandiriVaBankTransfer", "localBankTransfer": "LocalBankTransfer", "indonesianBankTransfer": "IndonesianBankTransfer", "ach": "Ach", "sepa": "Sepa", "bacs": "Bacs", "becs": "Becs", "sepaGuaranteedDebit": "SepaGuaranteedDebit", "affirm": "Affirm", "afterpayClearpay": "AfterpayClearpay", "klarna": "Klarna", "cardDetailsForNetworkTransactionId": "CardDetailsForNetworkTransactionId", "networkToken": "NetworkTokenData", "decryptedWalletTokenDetailsForNetworkTransactionId": "DecryptedWalletTokenDetailsForNetworkTransactionId", "givex": "Givex", "paySafeCard": "PaySafeCard", "boleto": "Boleto", "efecty": "Efecty", "pagoEfectivo": "PagoEfectivo", "redCompra": "RedCompra", "redPagos": "RedPagos", "alfamart": "Alfamart", "indomaret": "Indomaret", "oxxo": "Oxxo", "sevenEleven": "SevenEleven", "lawson": "Lawson", "miniStop": "MiniStop", "familyMart": "FamilyMart", "seicomart": "Seicomart", "payEasy": "PayEasy" },
  AppleWallet: { "paymentData": "PaymentData", "paymentMethod": "PaymentMethod" },
  PaymentData: { "decryptedData": "ApplePayDecryptedData" },
  ApplePayDecryptedData: { "paymentData": "ApplePayCryptogramData" },
  GoogleWallet: { "info": "PaymentMethodInfo", "tokenizationData": "TokenizationData" },
  PaymentMethodInfo: { "assuranceDetails": "AssuranceDetails" },
  TokenizationData: { "decryptedData": "GooglePayDecryptedData", "encryptedData": "GooglePayEncryptedTokenizationData" },
  SamsungWallet: { "paymentCredential": "PaymentCredential" },
  PaymentCredential: { "tokenData": "TokenData" },
  PazeWallet: { "decryptedData": "PazeDecryptedData" },
  PazeConsumer: { "mobileNumber": "PazePhoneNumber" },
  PazeDecryptedData: { "token": "PazeToken", "dynamicData": "PazeDynamicData", "billingAddress": "PazeAddress", "consumer": "PazeConsumer" },
  ErrorInfo: { "unifiedDetails": "UnifiedErrorDetails", "issuerDetails": "IssuerErrorDetails", "connectorDetails": "ConnectorErrorDetails" },
  IssuerErrorDetails: { "networkDetails": "NetworkErrorDetails" },
  Identifier: { "noResponseIdMarker": "Empty" },
  ConnectorState: { "accessToken": "AccessToken" },
  NetworkParams: { "cartesBancaires": "CartesBancairesParams" },
  AuthenticationData: { "networkParams": "NetworkParams" },
  CustomerAcceptance: { "onlineMandateDetails": "OnlineMandate" },
  MandateType: { "singleUse": "MandateAmountData", "multiUse": "MandateAmountData" },
  SetupMandateDetails: { "customerAcceptance": "CustomerAcceptance", "mandateType": "MandateType" },
  MandateReference: { "connectorMandateId": "ConnectorMandateReferenceId", "networkTokenWithNti": "NetworkTokenWithNTI" },
  PaymentAddress: { "shippingAddress": "Address", "billingAddress": "Address" },
  OrderInfo: { "orderDetails": "OrderDetailsWithAmount" },
  L2L3Data: { "orderInfo": "OrderInfo", "taxInfo": "TaxInfo" },
  RedirectionResponse: { "payload": "PayloadEntry" },
  RedirectForm: { "form": "FormData", "html": "HtmlData", "uri": "UriData", "braintree": "BraintreeData", "mifinity": "MifinityData" },
  FormData: { "formFields": "FormFieldsEntry" },
  RequestDetails: { "headers": "HeadersEntry" },
  EventServiceHandleResponse: { "eventContent": "EventContent", "eventAckResponse": "EventAckResponse" },
  EventAckResponse: { "headers": "HeadersEntry" },
  EventContent: { "paymentsResponse": "PaymentServiceGetResponse", "refundsResponse": "RefundResponse", "disputesResponse": "DisputeResponse", "incompleteTransformation": "IncompleteTransformationResponse" },
  InteracCustomerInfo: { "customerInfo": "CustomerInfo" },
  BankRedirectConnectorResponse: { "interac": "InteracCustomerInfo" },
  AdditionalPaymentMethodConnectorResponse: { "card": "CardConnectorResponse", "upi": "UpiConnectorResponse", "googlePay": "GooglePayConnectorResponse", "applePay": "ApplePayConnectorResponse", "bankRedirect": "BankRedirectConnectorResponse" },
  ConnectorResponseData: { "additionalPaymentMethodData": "AdditionalPaymentMethodConnectorResponse", "extendedAuthorizationResponseData": "ExtendedAuthorizationResponseData" },
  PaymentMethodUpdate: { "card": "CardDetailUpdate" },
  SessionToken: { "googlePay": "GpaySessionTokenResponse", "paypal": "PaypalSessionTokenResponse", "applePay": "ApplepaySessionTokenResponse", "connector": "ConnectorSessionTokenResponse" },
  GpaySessionTokenResponse: { "googlePaySession": "GooglePaySessionResponse" },
  GooglePaySessionResponse: { "merchantInfo": "GpayMerchantInfo", "shippingAddressParameters": "GpayShippingAddressParameters", "allowedPaymentMethods": "GpayAllowedPaymentMethods", "transactionInfo": "GpayTransactionInfo", "secrets": "SecretInfoToInitiateSdk" },
  GpayAllowedPaymentMethods: { "parameters": "GpayAllowedMethodsParameters", "tokenizationSpecification": "GpayTokenizationSpecification" },
  GpayAllowedMethodsParameters: { "billingAddressParameters": "GpayBillingAddressParameters" },
  GpayTokenizationSpecification: { "parameters": "GpayTokenParameters" },
  ApplepaySessionTokenResponse: { "sessionTokenData": "ApplePaySessionResponse", "paymentRequestData": "ApplePayPaymentRequest" },
  ApplePaySessionResponse: { "thirdPartySdk": "ThirdPartySdkSessionResponse" },
  ThirdPartySdkSessionResponse: { "secrets": "SecretInfoToInitiateSdk" },
  ApplePayPaymentRequest: { "total": "AmountInfo" },
  ApplePayRecurringPaymentRequest: { "regularBilling": "ApplePayRegularBillingRequest" },
  PaypalSessionTokenResponse: { "transactionInfo": "PaypalTransactionInfo" },
  PaymentServiceAuthorizeRequest: { "amount": "Money", "paymentMethod": "PaymentMethod", "customer": "Customer", "address": "PaymentAddress", "authenticationData": "AuthenticationData", "customerAcceptance": "CustomerAcceptance", "browserInfo": "BrowserInformation", "setupMandateDetails": "SetupMandateDetails", "billingDescriptor": "BillingDescriptor", "state": "ConnectorState", "orderDetails": "OrderDetailsWithAmount", "redirectionResponse": "RedirectionResponse", "l2L3Data": "L2L3Data" },
  PaymentServiceAuthorizeResponse: { "error": "ErrorInfo", "responseHeaders": "ResponseHeadersEntry", "redirectionData": "RedirectForm", "state": "ConnectorState", "mandateReference": "MandateReference", "connectorResponse": "ConnectorResponseData" },
  PaymentServiceGetRequest: { "amount": "Money", "state": "ConnectorState" },
  PaymentServiceGetResponse: { "error": "ErrorInfo", "responseHeaders": "ResponseHeadersEntry", "mandateReference": "MandateReference", "amount": "Money", "connectorResponse": "ConnectorResponseData", "state": "ConnectorState", "redirectionData": "RedirectForm", "paymentMethodUpdate": "PaymentMethodUpdate" },
  PaymentServiceVoidRequest: { "browserInfo": "BrowserInformation", "amount": "Money", "state": "ConnectorState" },
  PaymentServiceVoidResponse: { "error": "ErrorInfo", "responseHeaders": "ResponseHeadersEntry", "state": "ConnectorState", "mandateReference": "MandateReference" },
  PaymentServiceReverseRequest: { "browserInfo": "BrowserInformation" },
  PaymentServiceReverseResponse: { "error": "ErrorInfo", "responseHeaders": "ResponseHeadersEntry" },
  MerchantAuthenticationServiceCreateAccessTokenResponse: { "error": "ErrorInfo" },
  MerchantAuthenticationServiceCreateSessionTokenRequest: { "amount": "Money", "state": "ConnectorState", "browserInfo": "BrowserInformation" },
  MerchantAuthenticationServiceCreateSessionTokenResponse: { "error": "ErrorInfo" },
  MerchantAuthenticationServiceCreateSdkSessionTokenRequest: { "amount": "Money", "customer": "Customer" },
  MerchantAuthenticationServiceCreateSdkSessionTokenResponse: { "sessionToken": "SessionToken", "error": "ErrorInfo" },
  PaymentServiceCaptureRequest: { "amountToCapture": "Money", "multipleCaptureData": "MultipleCaptureRequestData", "browserInfo": "BrowserInformation", "state": "ConnectorState" },
  PaymentServiceCaptureResponse: { "error": "ErrorInfo", "responseHeaders": "ResponseHeadersEntry", "state": "ConnectorState", "mandateReference": "MandateReference" },
  PaymentServiceCreateOrderRequest: { "amount": "Money", "state": "ConnectorState" },
  PaymentServiceCreateOrderResponse: { "error": "ErrorInfo", "responseHeaders": "ResponseHeadersEntry", "sessionToken": "SessionToken" },
  PaymentServiceRefundRequest: { "refundAmount": "Money", "browserInfo": "BrowserInformation", "state": "ConnectorState" },
  RefundResponse: { "error": "ErrorInfo", "responseHeaders": "ResponseHeadersEntry", "refundAmount": "Money", "state": "ConnectorState" },
  PaymentServiceDisputeRequest: { "state": "ConnectorState" },
  DisputeResponse: { "error": "ErrorInfo", "responseHeaders": "ResponseHeadersEntry", "disputeAmount": "Money", "evidenceDocuments": "EvidenceDocument" },
  PaymentServiceSetupRecurringRequest: { "amount": "Money", "paymentMethod": "PaymentMethod", "customer": "Customer", "address": "PaymentAddress", "authenticationData": "AuthenticationData", "customerAcceptance": "CustomerAcceptance", "browserInfo": "BrowserInformation", "billingDescriptor": "BillingDescriptor", "state": "ConnectorState", "l2L3Data": "L2L3Data", "setupMandateDetails": "SetupMandateDetails" },
  PaymentServiceSetupRecurringResponse: { "error": "ErrorInfo", "responseHeaders": "ResponseHeadersEntry", "mandateReference": "MandateReference", "redirectionData": "RedirectForm", "connectorResponse": "ConnectorResponseData", "state": "ConnectorState" },
  RecurringPaymentServiceChargeRequest: { "connectorRecurringPaymentId": "MandateReference", "amount": "Money", "paymentMethod": "PaymentMethod", "address": "PaymentAddress", "browserInfo": "BrowserInformation", "state": "ConnectorState", "originalPaymentAuthorizedAmount": "Money", "billingDescriptor": "BillingDescriptor", "authenticationData": "AuthenticationData", "customer": "Customer", "l2L3Data": "L2L3Data" },
  RecurringPaymentServiceChargeResponse: { "error": "ErrorInfo", "responseHeaders": "ResponseHeadersEntry", "mandateReference": "MandateReference", "state": "ConnectorState", "connectorResponse": "ConnectorResponseData" },
  RecurringPaymentServiceRevokeResponse: { "error": "ErrorInfo", "responseHeaders": "ResponseHeadersEntry" },
  PaymentMethodAuthenticationServicePreAuthenticateRequest: { "amount": "Money", "paymentMethod": "PaymentMethod", "customer": "Customer", "address": "PaymentAddress", "browserInfo": "BrowserInformation", "state": "ConnectorState" },
  PaymentMethodAuthenticationServicePreAuthenticateResponse: { "error": "ErrorInfo", "responseHeaders": "ResponseHeadersEntry", "redirectionData": "RedirectForm", "state": "ConnectorState", "authenticationData": "AuthenticationData" },
  PaymentMethodAuthenticationServiceAuthenticateRequest: { "amount": "Money", "paymentMethod": "PaymentMethod", "customer": "Customer", "address": "PaymentAddress", "authenticationData": "AuthenticationData", "browserInfo": "BrowserInformation", "state": "ConnectorState", "redirectionResponse": "RedirectionResponse" },
  PaymentMethodAuthenticationServiceAuthenticateResponse: { "error": "ErrorInfo", "responseHeaders": "ResponseHeadersEntry", "redirectionData": "RedirectForm", "authenticationData": "AuthenticationData", "state": "ConnectorState" },
  PaymentMethodAuthenticationServicePostAuthenticateRequest: { "amount": "Money", "paymentMethod": "PaymentMethod", "customer": "Customer", "address": "PaymentAddress", "authenticationData": "AuthenticationData", "browserInfo": "BrowserInformation", "state": "ConnectorState", "redirectionResponse": "RedirectionResponse" },
  PaymentMethodAuthenticationServicePostAuthenticateResponse: { "error": "ErrorInfo", "responseHeaders": "ResponseHeadersEntry", "redirectionData": "RedirectForm", "authenticationData": "AuthenticationData", "state": "ConnectorState" },
  PaymentServiceIncrementalAuthorizationRequest: { "amount": "Money", "state": "ConnectorState" },
  PaymentServiceIncrementalAuthorizationResponse: { "error": "ErrorInfo", "responseHeaders": "ResponseHeadersEntry", "state": "ConnectorState" },
  EventServiceHandleRequest: { "requestDetails": "RequestDetails", "webhookSecrets": "WebhookSecrets", "state": "ConnectorState" },
  PaymentServiceVerifyRedirectResponseRequest: { "requestDetails": "RequestDetails", "redirectResponseSecrets": "RedirectResponseSecrets" },
  PaymentServiceVerifyRedirectResponseResponse: { "responseAmount": "Money", "error": "ErrorInfo" },
  RefundServiceGetRequest: { "browserInfo": "BrowserInformation", "state": "ConnectorState" },
  DisputeServiceSubmitEvidenceRequest: { "evidenceDocuments": "EvidenceDocument" },
  DisputeServiceSubmitEvidenceResponse: { "error": "ErrorInfo", "responseHeaders": "ResponseHeadersEntry" },
  DisputeServiceDefendResponse: { "error": "ErrorInfo", "responseHeaders": "ResponseHeadersEntry" },
  DisputeServiceAcceptResponse: { "error": "ErrorInfo", "responseHeaders": "ResponseHeadersEntry" },
  PaymentMethodServiceTokenizeRequest: { "amount": "Money", "paymentMethod": "PaymentMethod", "customer": "Customer", "address": "PaymentAddress" },
  PaymentMethodServiceTokenizeResponse: { "error": "ErrorInfo", "responseHeaders": "ResponseHeadersEntry", "state": "ConnectorState" },
  PaymentMethodServiceGetRequest: { "state": "ConnectorState" },
  PaymentMethodServiceGetResponse: { "paymentMethod": "PaymentMethod", "customer": "Customer", "error": "ErrorInfo", "responseHeaders": "ResponseHeadersEntry" },
  CustomerServiceCreateRequest: { "address": "PaymentAddress" },
  CustomerServiceCreateResponse: { "error": "ErrorInfo", "responseHeaders": "ResponseHeadersEntry" },
  CustomerServiceGetResponse: { "customer": "Customer", "error": "ErrorInfo", "responseHeaders": "ResponseHeadersEntry" },
  CustomerServiceUpdateRequest: { "address": "PaymentAddress" },
  CustomerServiceUpdateResponse: { "customer": "Customer", "error": "ErrorInfo", "responseHeaders": "ResponseHeadersEntry" },
  CustomerServiceDeleteResponse: { "error": "ErrorInfo", "responseHeaders": "ResponseHeadersEntry" },
  PaysafePaymentMethodDetails: { "card": "CardEntry", "ach": "AchEntry" },
  CardEntry: { "value": "PaysafeCardAccountId" },
  AchEntry: { "value": "PaysafeAchAccountId" },
  PaysafeConfig: { "accountId": "PaysafePaymentMethodDetails" },
  CashtocodeConfig: { "authKeyMap": "AuthKeyMapEntry" },
  AuthKeyMapEntry: { "value": "PayloadCurrencyAuthData" },
  PayloadConfig: { "authKeyMap": "AuthKeyMapEntry" },
  ConnectorSpecificConfig: { "adyen": "AdyenConfig", "airwallex": "AirwallexConfig", "bambora": "BamboraConfig", "bankofamerica": "BankOfAmericaConfig", "billwerk": "BillwerkConfig", "bluesnap": "BluesnapConfig", "braintree": "BraintreeConfig", "cashtocode": "CashtocodeConfig", "cryptopay": "CryptopayConfig", "cybersource": "CybersourceConfig", "datatrans": "DatatransConfig", "dlocal": "DlocalConfig", "elavon": "ElavonConfig", "fiserv": "FiservConfig", "fiservemea": "FiservemeaConfig", "forte": "ForteConfig", "getnet": "GetnetConfig", "globalpay": "GlobalpayConfig", "hipay": "HipayConfig", "helcim": "HelcimConfig", "iatapay": "IatapayConfig", "jpmorgan": "JpmorganConfig", "mifinity": "MifinityConfig", "mollie": "MollieConfig", "multisafepay": "MultisafepayConfig", "nexinets": "NexinetsConfig", "nexixpay": "NexixpayConfig", "nmi": "NmiConfig", "noon": "NoonConfig", "novalnet": "NovalnetConfig", "nuvei": "NuveiConfig", "paybox": "PayboxConfig", "payme": "PaymeConfig", "payu": "PayuConfig", "powertranz": "PowertranzConfig", "rapyd": "RapydConfig", "redsys": "RedsysConfig", "shift4": "Shift4Config", "stax": "StaxConfig", "stripe": "StripeConfig", "trustpay": "TrustpayConfig", "tsys": "TsysConfig", "volt": "VoltConfig", "wellsfargo": "WellsfargoConfig", "worldpay": "WorldpayConfig", "worldpayvantiv": "WorldpayvantivConfig", "xendit": "XenditConfig", "phonepe": "PhonepeConfig", "cashfree": "CashfreeConfig", "paytm": "PaytmConfig", "calida": "CalidaConfig", "payload": "PayloadConfig", "authipay": "AuthipayConfig", "silverflow": "SilverflowConfig", "celero": "CeleroConfig", "trustpayments": "TrustpaymentsConfig", "paysafe": "PaysafeConfig", "barclaycard": "BarclaycardConfig", "worldpayxml": "WorldpayxmlConfig", "revolut": "RevolutConfig", "loonio": "LoonioConfig", "gigadat": "GigadatConfig", "hyperpg": "HyperpgConfig", "zift": "ZiftConfig", "screenstream": "ScreenstreamConfig", "ebanx": "EbanxConfig", "fiuu": "FiuuConfig", "globepay": "GlobepayConfig", "coinbase": "CoinbaseConfig", "coingate": "CoingateConfig", "revolv3": "Revolv3Config", "authorizedotnet": "AuthorizedotnetConfig", "peachpayments": "PeachpaymentsConfig", "paypal": "PaypalConfig", "truelayer": "TruelayerConfig", "fiservcommercehub": "FiservcommercehubConfig" },
  ConnectorConfig: { "connectorConfig": "ConnectorSpecificConfig", "options": "SdkOptions" },
  RequestConfig: { "http": "HttpConfig", "vault": "VaultOptions" },
  HttpConfig: { "proxy": "ProxyOptions", "caCert": "CaCert" },
  FfiOptions: { "connectorConfig": "ConnectorSpecificConfig" },
  FfiConnectorHttpRequest: { "headers": "HeadersEntry" },
  FfiConnectorHttpResponse: { "headers": "HeadersEntry" },
  FfiResult: { "httpRequest": "FfiConnectorHttpRequest", "httpResponse": "FfiConnectorHttpResponse", "integrationError": "IntegrationError", "connectorResponseTransformationError": "ConnectorResponseTransformationError" },
};

/**
 * Recursively pre-process a proto request object so that SecretString fields
 * (which the field-probe serialises as plain strings) are wrapped as {value: "..."}
 * before being handed to protobufjs fromObject.
 *
 * @param obj      - The (potentially plain-string) value to process.
 * @param msgName  - The protobuf message name for this level (e.g. "Ach").
 */
function _wrapSecretStrings(obj: unknown, msgName: string): unknown {
  if (typeof obj !== "object" || obj === null) return obj;
  if (Array.isArray(obj)) return (obj as unknown[]).map(item => _wrapSecretStrings(item, msgName));

  const secretSet = new Set(_SECRET_STRING_FIELDS[msgName] ?? []);
  const fieldTypes = _MSG_FIELD_TYPES[msgName] ?? {};
  const result: Record<string, unknown> = {};

  for (const [k, v] of Object.entries(obj as Record<string, unknown>)) {
    if (secretSet.has(k) && typeof v === "string") {
      // SecretString / CardNumberType / NetworkTokenType field stored as plain string → wrap
      result[k] = { value: v };
    } else if (typeof v === "string") {
      const nestedType = fieldTypes[k];
      if (nestedType) {
        // A message-type field received a plain string: this happens when
        // flatten_oneof_wrappers collapses e.g. {token: {token: SecretString}} → {token: "..."}
        // Re-expand: find the sole SecretString field of the nested message and wrap.
        const nestedSecrets = _SECRET_STRING_FIELDS[nestedType];
        if (nestedSecrets && nestedSecrets.length === 1) {
          result[k] = { [nestedSecrets[0]]: { value: v } };
        } else {
          result[k] = v;
        }
      } else {
        result[k] = v;
      }
    } else if (typeof v === "object" && v !== null && !Array.isArray(v)) {
      const nestedType = fieldTypes[k];
      result[k] = nestedType ? _wrapSecretStrings(v, nestedType) : v;
    } else {
      result[k] = v;
    }
  }
  return result;
}

// ── Dispatch helper ───────────────────────────────────────────────────────────

// eslint-disable-next-line @typescript-eslint/no-explicit-any
function callGrpc(ffi: GrpcFfi, config: GrpcConfig, method: string, req: any, ReqType: any, ResType: any): unknown {
  const configBuf = Buffer.from(JSON.stringify(config));
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const normalized = _wrapSecretStrings(req, (ReqType as any).name as string);
  const reqBuf    = Buffer.from(ReqType.encode(ReqType.fromObject(normalized)).finish());
  const outLen    = [0];

  const ptr = ffi.call(method, configBuf, configBuf.length, reqBuf, reqBuf.length, outLen);
  const len = outLen[0];
  const rawBytes = Buffer.from(koffi.decode(ptr, "uint8", len) as Uint8Array);
  ffi.free(ptr, len);

  if (rawBytes[0] === 1) {
    throw new Error(`gRPC error (${method}): ${rawBytes.slice(1).toString("utf-8")}`);
  }

  return ResType.decode(rawBytes.slice(1));
}

// ── Sub-clients (one per proto service) ──────────────────────────────────────

// CustomerService
export class GrpcCustomerClient {
  constructor(private ffi: GrpcFfi, private config: GrpcConfig) {}

  /** CustomerService.Create — Create customer record in the payment processor system. Stores customer details for future payment operations without re-sending personal information. */
  async create(req: unknown): Promise<unknown> {
    return callGrpc(this.ffi, this.config, "customer/create",
      req, types.CustomerServiceCreateRequest, types.CustomerServiceCreateResponse);
  }

}

// DisputeService
export class GrpcDisputeClient {
  constructor(private ffi: GrpcFfi, private config: GrpcConfig) {}

  /** DisputeService.SubmitEvidence — Upload evidence to dispute customer chargeback. Provides documentation like receipts and delivery proof to contest fraudulent transaction claims. */
  async submitEvidence(req: unknown): Promise<unknown> {
    return callGrpc(this.ffi, this.config, "dispute/submit_evidence",
      req, types.DisputeServiceSubmitEvidenceRequest, types.DisputeServiceSubmitEvidenceResponse);
  }

  /** DisputeService.Defend — Submit defense with reason code for dispute. Presents formal argument against customer's chargeback claim with supporting documentation. */
  async defend(req: unknown): Promise<unknown> {
    return callGrpc(this.ffi, this.config, "dispute/defend",
      req, types.DisputeServiceDefendRequest, types.DisputeServiceDefendResponse);
  }

  /** DisputeService.Accept — Concede dispute and accepts chargeback loss. Acknowledges liability and stops dispute defense process when evidence is insufficient. */
  async accept(req: unknown): Promise<unknown> {
    return callGrpc(this.ffi, this.config, "dispute/accept",
      req, types.DisputeServiceAcceptRequest, types.DisputeServiceAcceptResponse);
  }

}

// EventService
export class GrpcEventClient {
  constructor(private ffi: GrpcFfi, private config: GrpcConfig) {}

  /** EventService.HandleEvent — Process webhook notifications from connectors. Translates connector events into standardized responses for asynchronous payment state updates. */
  async handleEvent(req: unknown): Promise<unknown> {
    return callGrpc(this.ffi, this.config, "event/handle_event",
      req, types.EventServiceHandleRequest, types.EventServiceHandleResponse);
  }

}

// MerchantAuthenticationService
export class GrpcMerchantAuthenticationClient {
  constructor(private ffi: GrpcFfi, private config: GrpcConfig) {}

  /** MerchantAuthenticationService.CreateAccessToken — Generate short-lived connector authentication token. Provides secure credentials for connector API access without storing secrets client-side. */
  async createAccessToken(req: unknown): Promise<unknown> {
    return callGrpc(this.ffi, this.config, "merchant_authentication/create_access_token",
      req, types.MerchantAuthenticationServiceCreateAccessTokenRequest, types.MerchantAuthenticationServiceCreateAccessTokenResponse);
  }

  /** MerchantAuthenticationService.CreateSessionToken — Create session token for payment processing. Maintains session state across multiple payment operations for improved security and tracking. */
  async createSessionToken(req: unknown): Promise<unknown> {
    return callGrpc(this.ffi, this.config, "merchant_authentication/create_session_token",
      req, types.MerchantAuthenticationServiceCreateSessionTokenRequest, types.MerchantAuthenticationServiceCreateSessionTokenResponse);
  }

  /** MerchantAuthenticationService.CreateSdkSessionToken — Initialize wallet payment sessions for Apple Pay, Google Pay, etc. Sets up secure context for tokenized wallet payments with device verification. */
  async createSdkSessionToken(req: unknown): Promise<unknown> {
    return callGrpc(this.ffi, this.config, "merchant_authentication/create_sdk_session_token",
      req, types.MerchantAuthenticationServiceCreateSdkSessionTokenRequest, types.MerchantAuthenticationServiceCreateSdkSessionTokenResponse);
  }

}

// PaymentMethodAuthenticationService
export class GrpcPaymentMethodAuthenticationClient {
  constructor(private ffi: GrpcFfi, private config: GrpcConfig) {}

  /** PaymentMethodAuthenticationService.PreAuthenticate — Initiate 3DS flow before payment authorization. Collects device data and prepares authentication context for frictionless or challenge-based verification. */
  async preAuthenticate(req: unknown): Promise<unknown> {
    return callGrpc(this.ffi, this.config, "payment_method_authentication/pre_authenticate",
      req, types.PaymentMethodAuthenticationServicePreAuthenticateRequest, types.PaymentMethodAuthenticationServicePreAuthenticateResponse);
  }

  /** PaymentMethodAuthenticationService.Authenticate — Execute 3DS challenge or frictionless verification. Authenticates customer via bank challenge or behind-the-scenes verification for fraud prevention. */
  async authenticate(req: unknown): Promise<unknown> {
    return callGrpc(this.ffi, this.config, "payment_method_authentication/authenticate",
      req, types.PaymentMethodAuthenticationServiceAuthenticateRequest, types.PaymentMethodAuthenticationServiceAuthenticateResponse);
  }

  /** PaymentMethodAuthenticationService.PostAuthenticate — Validate authentication results with the issuing bank. Processes bank's authentication decision to determine if payment can proceed. */
  async postAuthenticate(req: unknown): Promise<unknown> {
    return callGrpc(this.ffi, this.config, "payment_method_authentication/post_authenticate",
      req, types.PaymentMethodAuthenticationServicePostAuthenticateRequest, types.PaymentMethodAuthenticationServicePostAuthenticateResponse);
  }

}

// PaymentMethodService
export class GrpcPaymentMethodClient {
  constructor(private ffi: GrpcFfi, private config: GrpcConfig) {}

  /** PaymentMethodService.Tokenize — Tokenize payment method for secure storage. Replaces raw card details with secure token for one-click payments and recurring billing. */
  async tokenize(req: unknown): Promise<unknown> {
    return callGrpc(this.ffi, this.config, "payment_method/tokenize",
      req, types.PaymentMethodServiceTokenizeRequest, types.PaymentMethodServiceTokenizeResponse);
  }

  /** PaymentMethodService.Eligibility — Check if the payout method is eligible for the transaction */
  async eligibility(req: unknown): Promise<unknown> {
    return callGrpc(this.ffi, this.config, "payment_method/eligibility",
      req, types.PayoutMethodEligibilityRequest, types.PayoutMethodEligibilityResponse);
  }

}

// PaymentService
export class GrpcPaymentClient {
  constructor(private ffi: GrpcFfi, private config: GrpcConfig) {}

  /** PaymentService.Authorize — Authorize a payment amount on a payment method. This reserves funds without capturing them, essential for verifying availability before finalizing. */
  async authorize(req: unknown): Promise<unknown> {
    return callGrpc(this.ffi, this.config, "payment/authorize",
      req, types.PaymentServiceAuthorizeRequest, types.PaymentServiceAuthorizeResponse);
  }

  /** PaymentService.Get — Retrieve current payment status from the payment processor. Enables synchronization between your system and payment processors for accurate state tracking. */
  async get(req: unknown): Promise<unknown> {
    return callGrpc(this.ffi, this.config, "payment/get",
      req, types.PaymentServiceGetRequest, types.PaymentServiceGetResponse);
  }

  /** PaymentService.Void — Cancel an authorized payment before capture. Releases held funds back to customer, typically used when orders are cancelled or abandoned. */
  async void(req: unknown): Promise<unknown> {
    return callGrpc(this.ffi, this.config, "payment/void",
      req, types.PaymentServiceVoidRequest, types.PaymentServiceVoidResponse);
  }

  /** PaymentService.Reverse — Reverse a captured payment before settlement. Recovers funds after capture but before bank settlement, used for corrections or cancellations. */
  async reverse(req: unknown): Promise<unknown> {
    return callGrpc(this.ffi, this.config, "payment/reverse",
      req, types.PaymentServiceReverseRequest, types.PaymentServiceReverseResponse);
  }

  /** PaymentService.Capture — Finalize an authorized payment transaction. Transfers reserved funds from customer to merchant account, completing the payment lifecycle. */
  async capture(req: unknown): Promise<unknown> {
    return callGrpc(this.ffi, this.config, "payment/capture",
      req, types.PaymentServiceCaptureRequest, types.PaymentServiceCaptureResponse);
  }

  /** PaymentService.CreateOrder — Initialize an order in the payment processor system. Sets up payment context before customer enters card details for improved authorization rates. */
  async createOrder(req: unknown): Promise<unknown> {
    return callGrpc(this.ffi, this.config, "payment/create_order",
      req, types.PaymentServiceCreateOrderRequest, types.PaymentServiceCreateOrderResponse);
  }

  /** PaymentService.Refund — Initiate a refund to customer's payment method. Returns funds for returns, cancellations, or service adjustments after original payment. */
  async refund(req: unknown): Promise<unknown> {
    return callGrpc(this.ffi, this.config, "payment/refund",
      req, types.PaymentServiceRefundRequest, types.RefundResponse);
  }

  /** PaymentService.IncrementalAuthorization — Increase authorized amount if still in authorized state. Allows adding charges to existing authorization for hospitality, tips, or incremental services. */
  async incrementalAuthorization(req: unknown): Promise<unknown> {
    return callGrpc(this.ffi, this.config, "payment/incremental_authorization",
      req, types.PaymentServiceIncrementalAuthorizationRequest, types.PaymentServiceIncrementalAuthorizationResponse);
  }

  /** PaymentService.VerifyRedirectResponse — Validate redirect-based payment responses. Confirms authenticity of redirect-based payment completions to prevent fraud and tampering. */
  async verifyRedirectResponse(req: unknown): Promise<unknown> {
    return callGrpc(this.ffi, this.config, "payment/verify_redirect_response",
      req, types.PaymentServiceVerifyRedirectResponseRequest, types.PaymentServiceVerifyRedirectResponseResponse);
  }

  /** PaymentService.SetupRecurring — Setup a recurring payment instruction for future payments/ debits. This could be for SaaS subscriptions, monthly bill payments, insurance payments and similar use cases. */
  async setupRecurring(req: unknown): Promise<unknown> {
    return callGrpc(this.ffi, this.config, "payment/setup_recurring",
      req, types.PaymentServiceSetupRecurringRequest, types.PaymentServiceSetupRecurringResponse);
  }

}

// PayoutService
export class GrpcPayoutClient {
  constructor(private ffi: GrpcFfi, private config: GrpcConfig) {}

  /** PayoutService.Transfer — Creates a payout fund transfer. */
  async transfer(req: unknown): Promise<unknown> {
    return callGrpc(this.ffi, this.config, "payout/transfer",
      req, types.PayoutServiceTransferRequest, types.PayoutServiceTransferResponse);
  }

  /** PayoutService.Stage — Stage the payout. */
  async stage(req: unknown): Promise<unknown> {
    return callGrpc(this.ffi, this.config, "payout/stage",
      req, types.PayoutServiceStageRequest, types.PayoutServiceStageResponse);
  }

  /** PayoutService.CreateLink — Creates a link between the recipient and the payout. */
  async createLink(req: unknown): Promise<unknown> {
    return callGrpc(this.ffi, this.config, "payout/create_link",
      req, types.PayoutServiceCreateLinkRequest, types.PayoutServiceCreateLinkResponse);
  }

  /** PayoutService.CreateRecipient — Create payout recipient. */
  async createRecipient(req: unknown): Promise<unknown> {
    return callGrpc(this.ffi, this.config, "payout/create_recipient",
      req, types.PayoutServiceCreateRecipientRequest, types.PayoutServiceCreateRecipientResponse);
  }

  /** PayoutService.EnrollDisburseAccount — Enroll disburse account. */
  async enrollDisburseAccount(req: unknown): Promise<unknown> {
    return callGrpc(this.ffi, this.config, "payout/enroll_disburse_account",
      req, types.PayoutServiceEnrollDisburseAccountRequest, types.PayoutServiceEnrollDisburseAccountResponse);
  }

}

// RecurringPaymentService
export class GrpcRecurringPaymentClient {
  constructor(private ffi: GrpcFfi, private config: GrpcConfig) {}

  /** RecurringPaymentService.Charge — Charge using an existing stored recurring payment instruction. Processes repeat payments for subscriptions or recurring billing without collecting payment details. */
  async charge(req: unknown): Promise<unknown> {
    return callGrpc(this.ffi, this.config, "recurring_payment/charge",
      req, types.RecurringPaymentServiceChargeRequest, types.RecurringPaymentServiceChargeResponse);
  }

  /** RecurringPaymentService.Revoke — Cancel an existing recurring payment mandate. Stops future automatic charges on customer's stored consent for subscription cancellations. */
  async revoke(req: unknown): Promise<unknown> {
    return callGrpc(this.ffi, this.config, "recurring_payment/revoke",
      req, types.RecurringPaymentServiceRevokeRequest, types.RecurringPaymentServiceRevokeResponse);
  }

}

// ── Top-level GrpcClient ──────────────────────────────────────────────────────

/**
 * Top-level gRPC client for the connector-service.
 *
 * Each sub-client corresponds to one proto service.  Auth headers from
 * `GrpcConfig` are injected automatically on every call via the Rust FFI layer.
 *
 * Example:
 * ```ts
 * const client = new GrpcClient({
 *   endpoint: "http://localhost:8000",
 *   connector: "stripe",
 *   connector_config: {"config": {"Stripe": {"api_key": "sk_test_..."}}},
 * });
 * const res = await client.customer.create({ ... });
 * const res = await client.dispute.submitEvidence({ ... });
 * const res = await client.event.handleEvent({ ... });
 * const res = await client.merchantAuthentication.createAccessToken({ ... });
 * ```
 */
export class GrpcClient {
  public customer: GrpcCustomerClient;
  public dispute: GrpcDisputeClient;
  public event: GrpcEventClient;
  public merchantAuthentication: GrpcMerchantAuthenticationClient;
  public paymentMethodAuthentication: GrpcPaymentMethodAuthenticationClient;
  public paymentMethod: GrpcPaymentMethodClient;
  public payment: GrpcPaymentClient;
  public payout: GrpcPayoutClient;
  public recurringPayment: GrpcRecurringPaymentClient;

  constructor(config: GrpcConfig, libPath?: string) {
    const ffi = loadGrpcFfi(libPath);
    this.customer = new GrpcCustomerClient(ffi, config);
    this.dispute = new GrpcDisputeClient(ffi, config);
    this.event = new GrpcEventClient(ffi, config);
    this.merchantAuthentication = new GrpcMerchantAuthenticationClient(ffi, config);
    this.paymentMethodAuthentication = new GrpcPaymentMethodAuthenticationClient(ffi, config);
    this.paymentMethod = new GrpcPaymentMethodClient(ffi, config);
    this.payment = new GrpcPaymentClient(ffi, config);
    this.payout = new GrpcPayoutClient(ffi, config);
    this.recurringPayment = new GrpcRecurringPaymentClient(ffi, config);
  }
}
