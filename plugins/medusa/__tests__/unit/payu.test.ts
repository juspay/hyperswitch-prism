// Self-contained unit tests for the PayU connector wiring in PrismService.
// The hyperswitch-prism SDK ships a native FFI lib (GLIBC 2.38) that cannot be
// loaded in every environment, so the SDK is fully mocked here — these tests
// exercise the plugin's PayU request-building / response-mapping logic without
// any network call.
jest.mock("hyperswitch-prism", () => {
  const mockProto = {
    Environment: { SANDBOX: 1, PRODUCTION: 2 },
    Currency: { USD: 840, EUR: 978, INR: 356, CURRENCY_UNSPECIFIED: 0 },
    PaymentMethodType: { PAY_PAL: 98, CARD: 21, UPI_COLLECT: 0 },
    CountryAlpha2: { US: 1, GB: 77, IN: 99 },
    CaptureMethod: { AUTOMATIC: 1, MANUAL: 2 },
    PaymentStatus: {
      AUTHORIZED: 1,
      CHARGED: 3,
      AUTHENTICATION_PENDING: 6,
      FAILURE: 17,
      PAYMENT_STATUS_UNSPECIFIED: 0,
    },
    RefundStatus: { REFUND_FAILURE: 1, REFUND_TRANSACTION_FAILURE: 2 },
    WebhookEventType: {},
    HttpMethod: { HTTP_METHOD_POST: 1 },
  }

  class MockPaymentClient {
    get = jest.fn()
    capture = jest.fn()
    refund = jest.fn()
    void = jest.fn()
    authorize = jest.fn()
  }
  class MockAuthClient {
    createClientAuthenticationToken = jest.fn()
  }
  class MockEventClient {
    handleEvent = jest.fn()
    parseEvent = jest.fn()
  }
  class MockConnectorError extends Error {}
  class MockIntegrationError extends Error {}

  return {
    PaymentClient: MockPaymentClient,
    MerchantAuthenticationClient: MockAuthClient,
    EventClient: MockEventClient,
    ConnectorError: MockConnectorError,
    IntegrationError: MockIntegrationError,
    types: mockProto,
  }
})

import PrismService from "../../medusa-custom-payments/src/providers/prism"

describe("PrismService — PayU (unit)", () => {
  let paymentClientMock: any
  let authClientMock: any

  const buildService = () =>
    new PrismService({
      connector: "payu" as any,
      connectorConfig: {
        apiKey: { value: "merchant_key" },
        apiSecret: { value: "merchant_salt" },
        returnUrl: "http://localhost:3000/return",
      },
      environment: "SANDBOX",
    })

  const withMocks = (service: PrismService) => {
    ;(service as any).paymentClient_ = paymentClientMock
    ;(service as any).authClient_ = authClientMock
    return service
  }

  beforeEach(() => {
    jest.clearAllMocks()
    const mocked = jest.requireMock("hyperswitch-prism")
    paymentClientMock = new mocked.PaymentClient()
    authClientMock = new mocked.MerchantAuthenticationClient()
  })

  describe("initiatePayment", () => {
    it("surfaces the PayU SDK session token from connectorSpecific.payu", async () => {
      const service = withMocks(buildService())
      authClientMock.createClientAuthenticationToken.mockResolvedValue({
        statusCode: 200,
        sessionData: {
          connectorSpecific: { payu: { sessionToken: { value: "oauth_abc" } } },
        },
      })

      const result = await service.initiatePayment({
        currency_code: "INR",
        amount: 10000,
        data: {},
        context: {},
      } as any)

      expect(result.status).toBe("pending")
      expect(result.data!.connector).toBe("payu")
      expect(result.data!.sessionToken).toBe("oauth_abc")
      expect(result.data!.currency).toBe("INR")
    })

    it("re-initiation persists the chosen VPA without a network call", async () => {
      const service = withMocks(buildService())

      const result = await service.initiatePayment({
        currency_code: "INR",
        amount: 10000,
        data: { id: "payu_ref_1", vpa: "success@upi", paymentMethodType: "upi_collect" },
        context: {},
      } as any)

      expect(
        authClientMock.createClientAuthenticationToken
      ).not.toHaveBeenCalled()
      expect(result.data!.vpa).toBe("success@upi")
      expect(result.data!.connector).toBe("payu")
    })
  })

  describe("authorizePayment", () => {
    const authInput = (extra: Record<string, any> = {}) =>
      ({
        data: {
          id: "payu_ref_1",
          connector: "payu",
          currency: "INR",
          minorAmount: 10000,
          billing: { firstName: "Asha", email: "asha@example.com" },
          paymentMethodType: "upi_collect",
          vpa: "success@upi",
          ...extra,
        },
      } as any)

    it("builds a UPI Collect authorize and surfaces redirectUrl", async () => {
      const service = withMocks(buildService())
      paymentClientMock.authorize.mockResolvedValue({
        status: 6, // AUTHENTICATION_PENDING
        connectorTransactionId: "payu_txn_99",
        redirectionData: { uri: { uri: "upi://pay?pa=success@upi" } },
      })

      const result = await service.authorizePayment(authInput())

      expect(paymentClientMock.authorize).toHaveBeenCalledTimes(1)
      const req = paymentClientMock.authorize.mock.calls[0][0]
      expect(req.paymentMethod.upiCollect.vpaId.value).toBe("success@upi")
      expect(req.amount.minorAmount).toBe(10000)
      expect(result.data!.redirectUrl).toBe("upi://pay?pa=success@upi")
      // adopts the real PayU reference as data.id for the post-redirect PSync
      expect(result.data!.id).toBe("payu_txn_99")
      expect(result.status).toBe("requires_more")
    })

    it("returns ERROR when billing is missing", async () => {
      const service = withMocks(buildService())

      const result = await service.authorizePayment(
        authInput({ billing: undefined })
      )

      expect(paymentClientMock.authorize).not.toHaveBeenCalled()
      expect(result.status).toBe("error")
    })

    it("status-syncs (does not re-authorize) on a repeat call", async () => {
      const service = withMocks(buildService())
      paymentClientMock.get.mockResolvedValue({ status: 3 }) // CHARGED

      const result = await service.authorizePayment(
        authInput({ connectorTransactionId: "payu_txn_99" })
      )

      expect(paymentClientMock.authorize).not.toHaveBeenCalled()
      expect(paymentClientMock.get).toHaveBeenCalledTimes(1)
      expect(result.status).toBe("captured")
    })
  })

  describe("cancel — shouldSkipVoid", () => {
    it("skips the void when the session was never authorized", async () => {
      const service = withMocks(buildService())

      // id === merchantClientSessionId ⇒ no real PayU reference yet
      await service.cancel({
        data: {
          id: "hs_session_1",
          merchantClientSessionId: "hs_session_1",
          connector: "payu",
        },
      } as any)

      expect(paymentClientMock.void).not.toHaveBeenCalled()
    })

    it("voids when a real PayU reference was adopted", async () => {
      const service = withMocks(buildService())
      paymentClientMock.void.mockResolvedValue({})

      await service.cancel({
        data: {
          id: "payu_txn_99",
          merchantClientSessionId: "hs_session_1",
          connector: "payu",
        },
      } as any)

      expect(paymentClientMock.void).toHaveBeenCalledTimes(1)
    })
  })
})
