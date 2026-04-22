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
  ) => unknown;
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
      "str",
      koffi.pointer("uint8"),
      "uint32",
      koffi.pointer("uint8"),
      "uint32",
      koffi.out(koffi.pointer("uint32")),
    ],
  );

  const free = lib.func("hyperswitch_grpc_free", "void", [
    koffi.pointer("uint8"),
    "uint32",
  ]);

  return { call, free };
}

// ── SecretString field normalization ─────────────────────────────────────────

const _SECRET_STRING_FIELDS: Record<string, readonly string[]> = {
  TokenPaymentMethodType: ["token"],
  CardDetails: ["cardNumber", "cardExpMonth", "cardExpYear", "cardCvc", "cardHolderName"],
  ProxyCardDetails: ["cardNumber", "cardExpMonth", "cardExpYear", "cardCvc", "cardHolderName"],
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
  Eft: ["accountNumber", "branchCode", "bankAccountHolderName"],
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
  NmiData: ["publicKey"],
  CustomerInfo: ["customerName", "customerEmail", "customerPhoneNumber", "customerBankId", "customerBankName"],
  AdyenClientAuthenticationResponse: ["sessionData"],
  CheckoutClientAuthenticationResponse: ["paymentSessionToken", "paymentSessionSecret"],
  CybersourceClientAuthenticationResponse: ["captureContext"],
  NuveiClientAuthenticationResponse: ["sessionToken"],
  MollieClientAuthenticationResponse: ["checkoutUrl"],
  GlobalpayClientAuthenticationResponse: ["accessToken"],
  BluesnapClientAuthenticationResponse: ["pfToken"],
  Shift4ClientAuthenticationResponse: ["clientSecret"],
  BankOfAmericaClientAuthenticationResponse: ["captureContext"],
  WellsfargoClientAuthenticationResponse: ["captureContext"],
  FiservClientAuthenticationResponse: ["sessionId"],
  ElavonClientAuthenticationResponse: ["sessionToken"],
  NoonClientAuthenticationResponse: ["checkoutUrl"],
  PaysafeClientAuthenticationResponse: ["paymentHandleToken"],
  BamboraapacClientAuthenticationResponse: ["token"],
  DatatransClientAuthenticationResponse: ["transactionId"],
  BamboraClientAuthenticationResponse: ["token"],
  PayloadClientAuthenticationResponse: ["clientToken"],
  MultisafepayClientAuthenticationResponse: ["apiToken"],
  NexixpayClientAuthenticationResponse: ["securityToken"],
  StripeClientAuthenticationResponse: ["clientSecret"],
  GpayTokenParameters: ["publicKey"],
  SecretInfoToInitiateSdk: ["display", "payment"],
  PaymentServiceAuthorizeRequest: ["metadata", "connectorFeatureData"],
  PaymentServiceAuthorizeResponse: ["rawConnectorResponse", "rawConnectorRequest", "connectorFeatureData"],
  PaymentServiceGetRequest: ["metadata", "connectorFeatureData"],
  PaymentServiceGetResponse: ["metadata", "rawConnectorResponse", "rawConnectorRequest"],
  PaymentServiceVoidRequest: ["metadata", "connectorFeatureData"],
  PaymentServiceVoidResponse: ["rawConnectorRequest", "connectorFeatureData"],
  PaymentServiceReverseRequest: ["metadata", "connectorFeatureData"],
  MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest: ["metadata", "connectorFeatureData"],
  MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse: ["accessToken"],
  MerchantAuthenticationServiceCreateServerSessionAuthenticationTokenRequest: ["connectorFeatureData"],
  PaymentSessionContext: ["metadata"],
  MerchantAuthenticationServiceCreateClientAuthenticationTokenRequest: ["connectorFeatureData"],
  PaymentClientAuthenticationContext: ["metadata"],
  MerchantAuthenticationServiceCreateClientAuthenticationTokenResponse: ["rawConnectorResponse", "rawConnectorRequest"],
  PaymentServiceCaptureRequest: ["metadata", "connectorFeatureData"],
  PaymentServiceCaptureResponse: ["rawConnectorRequest", "connectorFeatureData"],
  PaymentServiceCreateOrderRequest: ["metadata", "connectorFeatureData"],
  PaymentServiceCreateOrderResponse: ["rawConnectorRequest", "rawConnectorResponse"],
  PaymentServiceRefundRequest: ["metadata", "refundMetadata", "connectorFeatureData"],
  RefundResponse: ["email", "metadata", "refundMetadata", "acquirerReferenceNumber", "rawConnectorResponse", "rawConnectorRequest"],
  DisputeResponse: ["rawConnectorRequest"],
  PaymentServiceSetupRecurringRequest: ["metadata", "connectorFeatureData", "connectorTestingData"],
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
  SanlamConfig: ["apiKey", "merchantId"],
  ItaubankConfig: ["clientSecret", "clientId"],
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
  PproConfig: ["apiKey", "merchantId"],
  PaypalConfig: ["clientId", "clientSecret", "payerId"],
  TrustlyConfig: ["username", "password", "privateKey"],
  EasebuzzConfig: ["apiKey", "apiSalt"],
  TruelayerConfig: ["clientId", "clientSecret", "merchantAccountId", "accountHolderName", "privateKey", "kid"],
  PinelabsOnlineConfig: ["clientId", "clientSecret"],
  PaymentServiceTokenAuthorizeRequest: ["connectorToken", "metadata", "connectorFeatureData"],
  PaymentServiceTokenSetupRecurringRequest: ["connectorToken", "metadata", "connectorFeatureData"],
  PaymentServiceProxyAuthorizeRequest: ["metadata", "connectorFeatureData"],
  PaymentServiceProxySetupRecurringRequest: ["metadata"],
  CardPayout: ["cardNumber", "cardExpMonth", "cardExpYear", "cardHolderName"],
  AchBankTransferPayout: ["bankAccountNumber", "bankRoutingNumber"],
  BacsBankTransferPayout: ["bankAccountNumber", "bankSortCode"],
  SepaBankTransferPayout: ["iban", "bic"],
  PixBankTransferPayout: ["bankAccountNumber", "pixKey", "taxId"],
  ApplePayDecrypt: ["dpan", "expiryMonth", "expiryYear", "cardHolderName"],
  Paypal: ["email", "telephoneNumber", "paypalId"],
  Venmo: ["telephoneNumber"],
  InteracPayout: ["email"],
  OpenBankingUkPayout: ["accountHolderName", "iban"],
  PayoutServiceCreateRequest: ["connectorFeatureData", "accessToken"],
  PayoutServiceTransferRequest: ["accessToken"],
  PayoutServiceStageRequest: ["accessToken"],
  PayoutServiceGetRequest: ["accessToken"],
  PayoutServiceVoidRequest: ["connectorFeatureData", "accessToken"],
  PayoutServiceCreateLinkRequest: ["connectorFeatureData", "accessToken"],
  PayoutServiceCreateRecipientRequest: ["accessToken"],
  PayoutServiceEnrollDisburseAccountRequest: ["accessToken"],
  PayoutMethodEligibilityRequest: ["connectorFeatureData", "accessToken"],
};

const _MSG_FIELD_TYPES: Record<string, Record<string, string>> = {
  PaymentMethod: { "card": "CardDetails", "cardRedirect": "CardRedirect", "cardProxy": "ProxyCardDetails", "token": "TokenPaymentMethodType", "applePay": "AppleWallet", "googlePay": "GoogleWallet", "applePayThirdPartySdk": "ApplePayThirdPartySdkWallet", "googlePayThirdPartySdk": "GooglePayThirdPartySdkWallet", "paypalSdk": "PaypalSdkWallet", "amazonPayRedirect": "AmazonPayRedirectWallet", "cashappQr": "CashappQrWallet", "paypalRedirect": "PaypalRedirectWallet", "weChatPayQr": "WeChatPayQrWallet", "aliPayRedirect": "AliPayRedirectWallet", "revolutPay": "RevolutPayWallet", "mifinity": "MifinityWallet", "bluecode": "Bluecode", "paze": "PazeWallet", "samsungPay": "SamsungWallet", "mbWay": "MBWay", "satispay": "Satispay", "wero": "Wero", "lazypayRedirect": "LazyPayRedirectWallet", "phonepeRedirect": "PhonePeRedirectWallet", "billdeskRedirect": "BillDeskRedirectWallet", "cashfreeRedirect": "CashfreeRedirectWallet", "payuRedirect": "PayURedirectWallet", "easebuzzRedirect": "EaseBuzzRedirectWallet", "kakaoPayRedirect": "KakaoPayRedirectWallet", "mbWayRedirect": "MbWayRedirectWallet", "momoRedirect": "MomoRedirectWallet", "touchNGoRedirect": "TouchNGoRedirectWallet", "twintRedirect": "TwintRedirectWallet", "vippsRedirect": "VippsRedirectWallet", "weChatPayRedirect": "WeChatPayRedirectWallet", "aliPayHk": "AliPayHKWallet", "danaRedirect": "DanaRedirectWallet", "gcashRedirect": "GcashRedirectWallet", "goPayRedirect": "GoPayRedirectWallet", "swishQr": "SwishQrWallet", "upiCollect": "UpiCollect", "upiIntent": "UpiIntent", "upiQr": "UpiQr", "onlineBankingThailand": "OnlineBankingThailand", "onlineBankingCzechRepublic": "OnlineBankingCzechRepublic", "onlineBankingFinland": "OnlineBankingFinland", "onlineBankingFpx": "OnlineBankingFPX", "onlineBankingPoland": "OnlineBankingPoland", "onlineBankingSlovakia": "OnlineBankingSlovakia", "openBankingUk": "OpenBankingUK", "openBankingPis": "OpenBankingPIS", "localBankRedirect": "LocalBankRedirect", "ideal": "Ideal", "sofort": "Sofort", "trustly": "Trustly", "giropay": "Giropay", "eps": "Eps", "przelewy24": "Przelewy24", "pse": "Pse", "bancontactCard": "BancontactCard", "blik": "Blik", "openBanking": "OpenBanking", "interac": "Interac", "bizum": "Bizum", "eftBankRedirect": "EftBankRedirect", "duitNow": "DuitNow", "crypto": "CryptoCurrency", "classicReward": "ClassicReward", "eVoucher": "EVoucher", "instantBankTransfer": "InstantBankTransfer", "achBankTransfer": "AchBankTransfer", "sepaBankTransfer": "SepaBankTransfer", "bacsBankTransfer": "BacsBankTransfer", "multibancoBankTransfer": "MultibancoBankTransfer", "instantBankTransferFinland": "InstantBankTransferFinland", "instantBankTransferPoland": "InstantBankTransferPoland", "pix": "PixPayment", "permataBankTransfer": "PermataBankTransfer", "bcaBankTransfer": "BCABankTransfer", "bniVaBankTransfer": "BNIVaBankTransfer", "briVaBankTransfer": "BRIVaBankTransfer", "cimbVaBankTransfer": "CIMBVaBankTransfer", "danamonVaBankTransfer": "DanamonVaBankTransfer", "mandiriVaBankTransfer": "MandiriVaBankTransfer", "localBankTransfer": "LocalBankTransfer", "indonesianBankTransfer": "IndonesianBankTransfer", "ach": "Ach", "sepa": "Sepa", "bacs": "Bacs", "becs": "Becs", "sepaGuaranteedDebit": "SepaGuaranteedDebit", "eft": "Eft", "affirm": "Affirm", "afterpayClearpay": "AfterpayClearpay", "klarna": "Klarna", "cardDetailsForNetworkTransactionId": "CardDetailsForNetworkTransactionId", "networkToken": "NetworkTokenData", "decryptedWalletTokenDetailsForNetworkTransactionId": "DecryptedWalletTokenDetailsForNetworkTransactionId", "givex": "Givex", "paySafeCard": "PaySafeCard", "boleto": "Boleto", "efecty": "Efecty", "pagoEfectivo": "PagoEfectivo", "redCompra": "RedCompra", "redPagos": "RedPagos", "alfamart": "Alfamart", "indomaret": "Indomaret", "oxxo": "Oxxo", "sevenEleven": "SevenEleven", "lawson": "Lawson", "miniStop": "MiniStop", "familyMart": "FamilyMart", "seicomart": "Seicomart", "payEasy": "PayEasy", "netbanking": "NetbankingPayment" },
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
  RedirectForm: { "form": "FormData", "html": "HtmlData", "uri": "UriData", "braintree": "BraintreeData", "mifinity": "MifinityData", "nmi": "NmiData" },
  FormData: { "formFields": "FormFieldsEntry" },
  NmiData: { "amount": "Money" },
  RequestDetails: { "headers": "HeadersEntry" },
  EventServiceHandleResponse: { "eventContent": "EventContent", "eventAckResponse": "EventAckResponse" },
  EventAckResponse: { "headers": "HeadersEntry" },
  EventContent: { "paymentsResponse": "PaymentServiceGetResponse", "refundsResponse": "RefundResponse", "disputesResponse": "DisputeResponse" },
  InteracCustomerInfo: { "customerInfo": "CustomerInfo" },
  BankRedirectConnectorResponse: { "interac": "InteracCustomerInfo" },
  AdditionalPaymentMethodConnectorResponse: { "card": "CardConnectorResponse", "upi": "UpiConnectorResponse", "googlePay": "GooglePayConnectorResponse", "applePay": "ApplePayConnectorResponse", "bankRedirect": "BankRedirectConnectorResponse" },
  ConnectorResponseData: { "additionalPaymentMethodData": "AdditionalPaymentMethodConnectorResponse", "extendedAuthorizationResponseData": "ExtendedAuthorizationResponseData" },
  PaymentMethodUpdate: { "card": "CardDetailUpdate" },
  ClientAuthenticationTokenData: { "googlePay": "GpayClientAuthenticationResponse", "paypal": "PaypalClientAuthenticationResponse", "applePay": "ApplepayClientAuthenticationResponse", "connectorSpecific": "ConnectorSpecificClientAuthenticationResponse" },
  ConnectorSpecificClientAuthenticationResponse: { "stripe": "StripeClientAuthenticationResponse", "adyen": "AdyenClientAuthenticationResponse", "checkout": "CheckoutClientAuthenticationResponse", "cybersource": "CybersourceClientAuthenticationResponse", "nuvei": "NuveiClientAuthenticationResponse", "mollie": "MollieClientAuthenticationResponse", "globalpay": "GlobalpayClientAuthenticationResponse", "bluesnap": "BluesnapClientAuthenticationResponse", "rapyd": "RapydClientAuthenticationResponse", "shift4": "Shift4ClientAuthenticationResponse", "bankOfAmerica": "BankOfAmericaClientAuthenticationResponse", "wellsfargo": "WellsfargoClientAuthenticationResponse", "fiserv": "FiservClientAuthenticationResponse", "elavon": "ElavonClientAuthenticationResponse", "noon": "NoonClientAuthenticationResponse", "paysafe": "PaysafeClientAuthenticationResponse", "bamboraapac": "BamboraapacClientAuthenticationResponse", "jpmorgan": "JpmorganClientAuthenticationResponse", "billwerk": "BillwerkClientAuthenticationResponse", "datatrans": "DatatransClientAuthenticationResponse", "bambora": "BamboraClientAuthenticationResponse", "payload": "PayloadClientAuthenticationResponse", "multisafepay": "MultisafepayClientAuthenticationResponse", "nexinets": "NexinetsClientAuthenticationResponse", "nexixpay": "NexixpayClientAuthenticationResponse" },
  GpayClientAuthenticationResponse: { "googlePaySession": "GooglePaySessionResponse" },
  GooglePaySessionResponse: { "merchantInfo": "GpayMerchantInfo", "shippingAddressParameters": "GpayShippingAddressParameters", "allowedPaymentMethods": "GpayAllowedPaymentMethods", "transactionInfo": "GpayTransactionInfo", "secrets": "SecretInfoToInitiateSdk" },
  GpayAllowedPaymentMethods: { "parameters": "GpayAllowedMethodsParameters", "tokenizationSpecification": "GpayTokenizationSpecification" },
  GpayAllowedMethodsParameters: { "billingAddressParameters": "GpayBillingAddressParameters" },
  GpayTokenizationSpecification: { "parameters": "GpayTokenParameters" },
  ApplepayClientAuthenticationResponse: { "sessionResponse": "ApplePaySessionResponse", "paymentRequestData": "ApplePayPaymentRequest" },
  ApplePaySessionResponse: { "thirdPartySdk": "ThirdPartySdkSessionResponse" },
  ThirdPartySdkSessionResponse: { "secrets": "SecretInfoToInitiateSdk" },
  ApplePayPaymentRequest: { "total": "AmountInfo" },
  ApplePayRecurringPaymentRequest: { "regularBilling": "ApplePayRegularBillingRequest" },
  PaypalClientAuthenticationResponse: { "transactionInfo": "PaypalTransactionInfo" },
  PaymentServiceAuthorizeRequest: { "amount": "Money", "paymentMethod": "PaymentMethod", "customer": "Customer", "address": "PaymentAddress", "authenticationData": "AuthenticationData", "customerAcceptance": "CustomerAcceptance", "browserInfo": "BrowserInformation", "setupMandateDetails": "SetupMandateDetails", "billingDescriptor": "BillingDescriptor", "state": "ConnectorState", "orderDetails": "OrderDetailsWithAmount", "redirectionResponse": "RedirectionResponse", "l2L3Data": "L2L3Data" },
  PaymentServiceAuthorizeResponse: { "error": "ErrorInfo", "responseHeaders": "ResponseHeadersEntry", "redirectionData": "RedirectForm", "state": "ConnectorState", "mandateReference": "MandateReference", "connectorResponse": "ConnectorResponseData" },
  PaymentServiceGetRequest: { "amount": "Money", "state": "ConnectorState" },
  PaymentServiceGetResponse: { "error": "ErrorInfo", "responseHeaders": "ResponseHeadersEntry", "mandateReference": "MandateReference", "amount": "Money", "connectorResponse": "ConnectorResponseData", "state": "ConnectorState", "redirectionData": "RedirectForm", "paymentMethodUpdate": "PaymentMethodUpdate" },
  PaymentServiceVoidRequest: { "browserInfo": "BrowserInformation", "amount": "Money", "state": "ConnectorState" },
  PaymentServiceVoidResponse: { "error": "ErrorInfo", "responseHeaders": "ResponseHeadersEntry", "state": "ConnectorState", "mandateReference": "MandateReference" },
  PaymentServiceReverseRequest: { "browserInfo": "BrowserInformation" },
  PaymentServiceReverseResponse: { "error": "ErrorInfo", "responseHeaders": "ResponseHeadersEntry" },
  MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse: { "error": "ErrorInfo" },
  MerchantAuthenticationServiceCreateServerSessionAuthenticationTokenRequest: { "state": "ConnectorState", "payment": "PaymentSessionContext" },
  PaymentSessionContext: { "amount": "Money", "browserInfo": "BrowserInformation" },
  MerchantAuthenticationServiceCreateServerSessionAuthenticationTokenResponse: { "error": "ErrorInfo" },
  MerchantAuthenticationServiceCreateClientAuthenticationTokenRequest: { "payment": "PaymentClientAuthenticationContext" },
  PaymentClientAuthenticationContext: { "amount": "Money", "customer": "Customer" },
  MerchantAuthenticationServiceCreateClientAuthenticationTokenResponse: { "sessionData": "ClientAuthenticationTokenData", "error": "ErrorInfo" },
  PaymentServiceCaptureRequest: { "amountToCapture": "Money", "multipleCaptureData": "MultipleCaptureRequestData", "browserInfo": "BrowserInformation", "state": "ConnectorState" },
  PaymentServiceCaptureResponse: { "error": "ErrorInfo", "responseHeaders": "ResponseHeadersEntry", "state": "ConnectorState", "mandateReference": "MandateReference" },
  PaymentServiceCreateOrderRequest: { "amount": "Money", "state": "ConnectorState" },
  PaymentServiceCreateOrderResponse: { "error": "ErrorInfo", "responseHeaders": "ResponseHeadersEntry", "sessionData": "ClientAuthenticationTokenData" },
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
  EventReference: { "payment": "PaymentEventReference", "refund": "RefundEventReference", "dispute": "DisputeEventReference", "mandate": "MandateEventReference", "payout": "PayoutEventReference" },
  EventServiceParseRequest: { "requestDetails": "RequestDetails" },
  EventServiceParseResponse: { "reference": "EventReference" },
  EventContext: { "payment": "PaymentEventContext", "refund": "RefundEventContext", "dispute": "DisputeEventContext", "mandate": "MandateEventContext", "payout": "PayoutEventContext" },
  EventServiceHandleRequest: { "requestDetails": "RequestDetails", "webhookSecrets": "WebhookSecrets", "accessToken": "AccessToken", "eventContext": "EventContext" },
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
  ConnectorSpecificConfig: { "adyen": "AdyenConfig", "airwallex": "AirwallexConfig", "bambora": "BamboraConfig", "bankofamerica": "BankOfAmericaConfig", "billwerk": "BillwerkConfig", "bluesnap": "BluesnapConfig", "braintree": "BraintreeConfig", "cashtocode": "CashtocodeConfig", "cryptopay": "CryptopayConfig", "cybersource": "CybersourceConfig", "datatrans": "DatatransConfig", "dlocal": "DlocalConfig", "elavon": "ElavonConfig", "fiserv": "FiservConfig", "fiservemea": "FiservemeaConfig", "forte": "ForteConfig", "getnet": "GetnetConfig", "globalpay": "GlobalpayConfig", "hipay": "HipayConfig", "helcim": "HelcimConfig", "iatapay": "IatapayConfig", "jpmorgan": "JpmorganConfig", "mifinity": "MifinityConfig", "mollie": "MollieConfig", "multisafepay": "MultisafepayConfig", "nexinets": "NexinetsConfig", "nexixpay": "NexixpayConfig", "nmi": "NmiConfig", "noon": "NoonConfig", "novalnet": "NovalnetConfig", "nuvei": "NuveiConfig", "paybox": "PayboxConfig", "payme": "PaymeConfig", "payu": "PayuConfig", "powertranz": "PowertranzConfig", "rapyd": "RapydConfig", "redsys": "RedsysConfig", "shift4": "Shift4Config", "stax": "StaxConfig", "stripe": "StripeConfig", "trustpay": "TrustpayConfig", "tsys": "TsysConfig", "volt": "VoltConfig", "wellsfargo": "WellsfargoConfig", "worldpay": "WorldpayConfig", "worldpayvantiv": "WorldpayvantivConfig", "xendit": "XenditConfig", "phonepe": "PhonepeConfig", "cashfree": "CashfreeConfig", "paytm": "PaytmConfig", "calida": "CalidaConfig", "payload": "PayloadConfig", "authipay": "AuthipayConfig", "silverflow": "SilverflowConfig", "celero": "CeleroConfig", "trustpayments": "TrustpaymentsConfig", "paysafe": "PaysafeConfig", "barclaycard": "BarclaycardConfig", "worldpayxml": "WorldpayxmlConfig", "revolut": "RevolutConfig", "loonio": "LoonioConfig", "gigadat": "GigadatConfig", "hyperpg": "HyperpgConfig", "zift": "ZiftConfig", "screenstream": "ScreenstreamConfig", "ebanx": "EbanxConfig", "fiuu": "FiuuConfig", "globepay": "GlobepayConfig", "coinbase": "CoinbaseConfig", "coingate": "CoingateConfig", "revolv3": "Revolv3Config", "authorizedotnet": "AuthorizedotnetConfig", "peachpayments": "PeachpaymentsConfig", "paypal": "PaypalConfig", "truelayer": "TruelayerConfig", "fiservcommercehub": "FiservcommercehubConfig", "itaubank": "ItaubankConfig", "ppro": "PproConfig", "trustly": "TrustlyConfig", "sanlam": "SanlamConfig", "pinelabsOnline": "PinelabsOnlineConfig", "easebuzz": "EasebuzzConfig" },
  PaymentServiceTokenAuthorizeRequest: { "amount": "Money", "customer": "Customer", "address": "PaymentAddress", "browserInfo": "BrowserInformation", "state": "ConnectorState", "billingDescriptor": "BillingDescriptor", "l2L3Data": "L2L3Data", "customerAcceptance": "CustomerAcceptance" },
  PaymentServiceTokenSetupRecurringRequest: { "amount": "Money", "customer": "Customer", "address": "PaymentAddress", "state": "ConnectorState", "customerAcceptance": "CustomerAcceptance", "setupMandateDetails": "SetupMandateDetails", "billingDescriptor": "BillingDescriptor" },
  PaymentServiceProxyAuthorizeRequest: { "amount": "Money", "cardProxy": "ProxyCardDetails", "customer": "Customer", "address": "PaymentAddress", "authenticationData": "AuthenticationData", "browserInfo": "BrowserInformation", "state": "ConnectorState", "setupMandateDetails": "SetupMandateDetails", "billingDescriptor": "BillingDescriptor", "redirectionResponse": "RedirectionResponse", "l2L3Data": "L2L3Data", "customerAcceptance": "CustomerAcceptance" },
  PaymentServiceProxySetupRecurringRequest: { "amount": "Money", "cardProxy": "ProxyCardDetails", "customer": "Customer", "address": "PaymentAddress", "state": "ConnectorState", "setupMandateDetails": "SetupMandateDetails", "customerAcceptance": "CustomerAcceptance", "authenticationData": "AuthenticationData", "browserInfo": "BrowserInformation" },
  PayoutAddress: { "shippingAddress": "Address", "billingAddress": "Address" },
  PayoutMethod: { "card": "CardPayout", "ach": "AchBankTransferPayout", "bacs": "BacsBankTransferPayout", "sepa": "SepaBankTransferPayout", "pix": "PixBankTransferPayout", "applePayDecrypt": "ApplePayDecrypt", "paypal": "Paypal", "venmo": "Venmo", "interac": "InteracPayout", "openBankingUk": "OpenBankingUkPayout", "passthrough": "Passthrough" },
  PayoutServiceCreateRequest: { "address": "PayoutAddress", "payoutMethodData": "PayoutMethod", "amount": "Money", "customer": "Customer", "browserInfo": "BrowserInformation" },
  PayoutServiceCreateResponse: { "error": "ErrorInfo" },
  PayoutServiceTransferRequest: { "address": "PayoutAddress", "payoutMethodData": "PayoutMethod", "amount": "Money", "customer": "Customer", "browserInfo": "BrowserInformation" },
  PayoutServiceTransferResponse: { "error": "ErrorInfo" },
  PayoutServiceStageRequest: { "address": "PayoutAddress", "amount": "Money", "customer": "Customer", "browserInfo": "BrowserInformation" },
  PayoutServiceStageResponse: { "error": "ErrorInfo" },
  PayoutServiceGetResponse: { "error": "ErrorInfo" },
  PayoutServiceVoidRequest: { "address": "PayoutAddress" },
  PayoutServiceVoidResponse: { "error": "ErrorInfo" },
  PayoutServiceCreateLinkRequest: { "address": "PayoutAddress", "payoutMethodData": "PayoutMethod", "amount": "Money", "customer": "Customer", "browserInfo": "BrowserInformation" },
  PayoutServiceCreateLinkResponse: { "error": "ErrorInfo" },
  PayoutServiceCreateRecipientRequest: { "address": "PayoutAddress", "payoutMethodData": "PayoutMethod", "amount": "Money", "customer": "Customer" },
  PayoutServiceCreateRecipientResponse: { "error": "ErrorInfo" },
  PayoutServiceEnrollDisburseAccountRequest: { "address": "PayoutAddress", "payoutMethodData": "PayoutMethod", "amount": "Money", "customer": "Customer" },
  PayoutServiceEnrollDisburseAccountResponse: { "error": "ErrorInfo" },
  PayoutMethodEligibilityRequest: { "payoutMethodData": "PayoutMethod", "amount": "Money" },
  PayoutMethodEligibilityResponse: { "error": "ErrorInfo" },
  ConnectorConfig: { "connectorConfig": "ConnectorSpecificConfig", "options": "SdkOptions" },
  RequestConfig: { "http": "HttpConfig", "vault": "VaultOptions" },
  HttpConfig: { "proxy": "ProxyOptions", "caCert": "CaCert" },
  FfiOptions: { "connectorConfig": "ConnectorSpecificConfig" },
  FfiConnectorHttpRequest: { "headers": "HeadersEntry" },
  FfiConnectorHttpResponse: { "headers": "HeadersEntry" },
  FfiResult: { "httpRequest": "FfiConnectorHttpRequest", "httpResponse": "FfiConnectorHttpResponse", "integrationError": "IntegrationError", "connectorError": "ConnectorError" },
};

function _wrapSecretStrings(obj: unknown, msgName: string): unknown {
  if (typeof obj !== "object" || obj === null) return obj;
  if (Array.isArray(obj)) return (obj as unknown[]).map(item => _wrapSecretStrings(item, msgName));

  const secretSet = new Set(_SECRET_STRING_FIELDS[msgName] ?? []);
  const fieldTypes = _MSG_FIELD_TYPES[msgName] ?? {};
  const result: Record<string, unknown> = {};

  for (const [k, v] of Object.entries(obj as Record<string, unknown>)) {
    if (secretSet.has(k) && typeof v === "string") {
      result[k] = { value: v };
    } else if (typeof v === "string") {
      const nestedType = fieldTypes[k];
      if (nestedType) {
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

function callGrpc(ffi: GrpcFfi, config: GrpcConfig, method: string, req: any, ReqType: any, ResType: any): unknown {
  const configBuf = Buffer.from(JSON.stringify(config));
  const normalized = _wrapSecretStrings(req, (ReqType as any).name as string);
  const reqBuf = Buffer.from(ReqType.encode(ReqType.create(normalized)).finish());
  const outLen = [0];

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
  /** DisputeService.Get — Retrieve dispute status and evidence submission state. Tracks dispute progress through bank review process for informed decision-making. */
  async disputeGet(req: unknown): Promise<unknown> {
    return callGrpc(this.ffi, this.config, "dispute/dispute_get",
      req, types.DisputeServiceGetRequest, types.DisputeResponse);
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

  /** EventService.ParseEvent — Parse a raw webhook payload without credentials. Returns resource reference and event type — sufficient to resolve secrets or early-exit. */
  async parseEvent(req: unknown): Promise<unknown> {
    return callGrpc(this.ffi, this.config, "event/parse_event",
      req, types.EventServiceParseRequest, types.EventServiceParseResponse);
  }
  /** EventService.HandleEvent — Verify webhook source and return a unified typed response. Response mirrors PaymentService.Get / RefundService.Get / DisputeService.Get. */
  async handleEvent(req: unknown): Promise<unknown> {
    return callGrpc(this.ffi, this.config, "event/handle_event",
      req, types.EventServiceHandleRequest, types.EventServiceHandleResponse);
  }
}

// MerchantAuthenticationService
export class GrpcMerchantAuthenticationClient {
  constructor(private ffi: GrpcFfi, private config: GrpcConfig) {}

  /** MerchantAuthenticationService.CreateServerAuthenticationToken — Generate short-lived connector authentication token. Provides secure credentials for connector API access without storing secrets client-side. */
  async createServerAuthenticationToken(req: unknown): Promise<unknown> {
    return callGrpc(this.ffi, this.config, "merchant_authentication/create_server_authentication_token",
      req, types.MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest, types.MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse);
  }
  /** MerchantAuthenticationService.CreateServerSessionAuthenticationToken — Create a server-side session with the connector. Establishes session state for multi-step operations like 3DS verification or wallet authorization. */
  async createServerSessionAuthenticationToken(req: unknown): Promise<unknown> {
    return callGrpc(this.ffi, this.config, "merchant_authentication/create_server_session_authentication_token",
      req, types.MerchantAuthenticationServiceCreateServerSessionAuthenticationTokenRequest, types.MerchantAuthenticationServiceCreateServerSessionAuthenticationTokenResponse);
  }
  /** MerchantAuthenticationService.CreateClientAuthenticationToken — Initialize client-facing SDK sessions for wallets, device fingerprinting, etc. Returns structured data the client SDK needs to render payment/verification UI. */
  async createClientAuthenticationToken(req: unknown): Promise<unknown> {
    return callGrpc(this.ffi, this.config, "merchant_authentication/create_client_authentication_token",
      req, types.MerchantAuthenticationServiceCreateClientAuthenticationTokenRequest, types.MerchantAuthenticationServiceCreateClientAuthenticationTokenResponse);
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
  /** PaymentService.Void — Cancel an authorized payment that has not been captured. Releases held funds back to the customer's payment method when a transaction cannot be completed. */
  async void(req: unknown): Promise<unknown> {
    return callGrpc(this.ffi, this.config, "payment/void",
      req, types.PaymentServiceVoidRequest, types.PaymentServiceVoidResponse);
  }
  /** PaymentService.Reverse — Reverse a captured payment in full. Initiates a complete refund when you need to cancel a settled transaction rather than just an authorization. */
  async reverse(req: unknown): Promise<unknown> {
    return callGrpc(this.ffi, this.config, "payment/reverse",
      req, types.PaymentServiceReverseRequest, types.PaymentServiceReverseResponse);
  }
  /** PaymentService.Capture — Finalize an authorized payment by transferring funds. Captures the authorized amount to complete the transaction and move funds to your merchant account. */
  async capture(req: unknown): Promise<unknown> {
    return callGrpc(this.ffi, this.config, "payment/capture",
      req, types.PaymentServiceCaptureRequest, types.PaymentServiceCaptureResponse);
  }
  /** PaymentService.CreateOrder — Create a payment order for later processing. Establishes a transaction context that can be authorized or captured in subsequent API calls. */
  async createOrder(req: unknown): Promise<unknown> {
    return callGrpc(this.ffi, this.config, "payment/create_order",
      req, types.PaymentServiceCreateOrderRequest, types.PaymentServiceCreateOrderResponse);
  }
  /** PaymentService.Refund — Process a partial or full refund for a captured payment. Returns funds to the customer when goods are returned or services are cancelled. */
  async refund(req: unknown): Promise<unknown> {
    return callGrpc(this.ffi, this.config, "payment/refund",
      req, types.PaymentServiceRefundRequest, types.RefundResponse);
  }
  /** PaymentService.IncrementalAuthorization — Increase the authorized amount for an existing payment. Enables you to capture additional funds when the transaction amount changes after initial authorization. */
  async incrementalAuthorization(req: unknown): Promise<unknown> {
    return callGrpc(this.ffi, this.config, "payment/incremental_authorization",
      req, types.PaymentServiceIncrementalAuthorizationRequest, types.PaymentServiceIncrementalAuthorizationResponse);
  }
  /** PaymentService.VerifyRedirectResponse — Verify and process redirect responses from 3D Secure or other external flows. Validates authentication results and updates payment state accordingly. */
  async verifyRedirectResponse(req: unknown): Promise<unknown> {
    return callGrpc(this.ffi, this.config, "payment/verify_redirect_response",
      req, types.PaymentServiceVerifyRedirectResponseRequest, types.PaymentServiceVerifyRedirectResponseResponse);
  }
  /** PaymentService.SetupRecurring — Configure a payment method for recurring billing. Sets up the mandate and payment details needed for future automated charges. */
  async setupRecurring(req: unknown): Promise<unknown> {
    return callGrpc(this.ffi, this.config, "payment/setup_recurring",
      req, types.PaymentServiceSetupRecurringRequest, types.PaymentServiceSetupRecurringResponse);
  }
  /** PaymentService.TokenAuthorize — Authorize using a connector-issued payment method token. */
  async tokenAuthorize(req: unknown): Promise<unknown> {
    return callGrpc(this.ffi, this.config, "payment/token_authorize",
      req, types.PaymentServiceTokenAuthorizeRequest, types.PaymentServiceAuthorizeResponse);
  }
  /** PaymentService.TokenSetupRecurring — Setup a recurring mandate using a connector token. */
  async tokenSetupRecurring(req: unknown): Promise<unknown> {
    return callGrpc(this.ffi, this.config, "payment/token_setup_recurring",
      req, types.PaymentServiceTokenSetupRecurringRequest, types.PaymentServiceSetupRecurringResponse);
  }
  /** PaymentService.ProxyAuthorize — Authorize using vault-aliased card data. Proxy substitutes before connector. */
  async proxyAuthorize(req: unknown): Promise<unknown> {
    return callGrpc(this.ffi, this.config, "payment/proxy_authorize",
      req, types.PaymentServiceProxyAuthorizeRequest, types.PaymentServiceAuthorizeResponse);
  }
  /** PaymentService.ProxySetupRecurring — Setup recurring mandate using vault-aliased card data. */
  async proxySetupRecurring(req: unknown): Promise<unknown> {
    return callGrpc(this.ffi, this.config, "payment/proxy_setup_recurring",
      req, types.PaymentServiceProxySetupRecurringRequest, types.PaymentServiceSetupRecurringResponse);
  }
}

// PayoutService
export class GrpcPayoutClient {
  constructor(private ffi: GrpcFfi, private config: GrpcConfig) {}

  /** PayoutService.Create — Creates a payout. */
  async payoutCreate(req: unknown): Promise<unknown> {
    return callGrpc(this.ffi, this.config, "payout/payout_create",
      req, types.PayoutServiceCreateRequest, types.PayoutServiceCreateResponse);
  }
  /** PayoutService.Transfer — Creates a payout fund transfer. */
  async transfer(req: unknown): Promise<unknown> {
    return callGrpc(this.ffi, this.config, "payout/transfer",
      req, types.PayoutServiceTransferRequest, types.PayoutServiceTransferResponse);
  }
  /** PayoutService.Get — Retrieve payout details. */
  async payoutGet(req: unknown): Promise<unknown> {
    return callGrpc(this.ffi, this.config, "payout/payout_get",
      req, types.PayoutServiceGetRequest, types.PayoutServiceGetResponse);
  }
  /** PayoutService.Void — Void a payout. */
  async payoutVoid(req: unknown): Promise<unknown> {
    return callGrpc(this.ffi, this.config, "payout/payout_void",
      req, types.PayoutServiceVoidRequest, types.PayoutServiceVoidResponse);
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

// RefundService
export class GrpcRefundClient {
  constructor(private ffi: GrpcFfi, private config: GrpcConfig) {}

  /** RefundService.Get — Retrieve refund status from the payment processor. Tracks refund progress through processor settlement for accurate customer communication. */
  async refundGet(req: unknown): Promise<unknown> {
    return callGrpc(this.ffi, this.config, "refund/refund_get",
      req, types.RefundServiceGetRequest, types.RefundResponse);
  }
}

// ── Top-level GrpcClient ──────────────────────────────────────────────────────

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
  public refund: GrpcRefundClient;

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
    this.refund = new GrpcRefundClient(ffi, config);
  }
}
