jest.mock("hyperswitch-prism", () => {
  const mockProto = {
    Environment: { SANDBOX: 1, PRODUCTION: 2 },
    Currency: { USD: 840, EUR: 978, JPY: 392, CURRENCY_UNSPECIFIED: 0 },
    PaymentMethodType: { PAY_PAL: 98, CARD: 21, GOOGLE_PAY: 38, APPLE_PAY: 9 },
    CountryAlpha2: { US: 1, GB: 77 },
    PaymentStatus: {
      AUTHORIZED: 1,
      PARTIALLY_AUTHORIZED: 2,
      CHARGED: 3,
      PARTIAL_CHARGED: 4,
      PARTIAL_CHARGED_AND_CHARGEABLE: 5,
      AUTHENTICATION_PENDING: 6,
      CONFIRMATION_AWAITED: 7,
      PAYMENT_METHOD_AWAITED: 8,
      DEVICE_DATA_COLLECTION_PENDING: 9,
      VOIDED: 10,
      VOID_INITIATED: 11,
      VOIDED_POST_CAPTURE: 12,
      AUTHORIZATION_FAILED: 13,
      AUTHENTICATION_FAILED: 14,
      CAPTURE_FAILED: 15,
      VOID_FAILED: 16,
      FAILURE: 17,
      ROUTER_DECLINED: 18,
      PAYMENT_STATUS_UNSPECIFIED: 0,
    },
    RefundStatus: { REFUND_FAILURE: 1, REFUND_TRANSACTION_FAILURE: 2 },
    WebhookEventType: {
      PAYMENT_INTENT_AUTHORIZATION_SUCCESS: 1,
      PAYMENT_INTENT_SUCCESS: 2,
      PAYMENT_INTENT_CAPTURE_SUCCESS: 3,
      PAYMENT_ACTION_REQUIRED: 4,
      PAYMENT_INTENT_PROCESSING: 5,
      PAYMENT_INTENT_CANCELLED: 6,
      PAYMENT_INTENT_FAILURE: 7,
      PAYMENT_INTENT_AUTHORIZATION_FAILURE: 8,
      PAYMENT_INTENT_CAPTURE_FAILURE: 9,
      WEBHOOK_REFUND_FAILURE: 15,
      WEBHOOK_REFUND_SUCCESS: 16,
      WEBHOOK_DISPUTE_OPENED: 17,
      WEBHOOK_DISPUTE_LOST: 23,
    },
    HttpMethod: { HTTP_METHOD_POST: 1 },
    CaptureMethod: { AUTOMATIC: 1 },
  }

  class MockPaymentClient {
    get = jest.fn()
    capture = jest.fn()
    refund = jest.fn()
    void = jest.fn()
    authorize = jest.fn()
    tokenAuthorize = jest.fn()
  }

  class MockAuthClient {
    createClientAuthenticationToken = jest.fn()
  }

  class MockEventClient {
    handleEvent = jest.fn()
    parseEvent = jest.fn()
  }

  class MockConnectorError extends Error {
    proto: any
    constructor(proto: any) {
      super(typeof proto === "string" ? proto : proto?.message || "Connector error")
      this.name = "ConnectorError"
      this.proto = proto
    }
    get errorCode() {
      return this.proto?.errorCode || "CONNECTOR_ERROR"
    }
    get httpStatusCode() {
      return this.proto?.httpStatusCode
    }
  }

  class MockIntegrationError extends Error {
    proto: any
    constructor(proto: any) {
      super(typeof proto === "string" ? proto : proto?.message || "Integration error")
      this.name = "IntegrationError"
      this.proto = proto
    }
    get errorCode() {
      return this.proto?.errorCode || "INTEGRATION_ERROR"
    }
    get suggestedAction() {
      return this.proto?.suggestedAction
    }
    get docUrl() {
      return this.proto?.docUrl
    }
  }

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
import HyperswitchPrismService from "../../medusa-custom-payments/src/providers/service"
import { mapPrismStatus } from "../../medusa-custom-payments/src/providers/utils"

// Session-data handling: re-initiation stores only expected fields (ids and
// connector state come from the plugin), the Authorize.Net card flow is
// sandbox-only and keeps only the last four digits after authorize,
// `sourceVerified` must be exactly `true`, and partial payment outcomes do not
// complete a cart.
describe("session data handling (unit)", () => {
  let paymentClientMock: any
  let eventClientMock: any

  const buildService = (
    connector: string,
    connectorConfig: any = {},
    extraOpts: Record<string, unknown> = {}
  ) => {
    const service = new PrismService({
      connector: connector as any,
      connectorConfig,
      environment: "SANDBOX",
      ...extraOpts,
    } as any)
    ;(service as any).paymentClient_ = paymentClientMock
    ;(service as any).eventClient_ = eventClientMock
    return service
  }

  const reinit = (service: PrismService, data: Record<string, unknown>) =>
    service.initiatePayment({
      currency_code: "usd",
      amount: 1000,
      data,
      context: { idempotency_key: "payses_server" },
    } as any)

  // Plugin-owned fields a storefront payload should not be able to set.
  const clientOverrides = {
    id: "tr_other",
    connectorTransactionId: "tr_other",
    merchantClientSessionId: "client_session",
    connector: "stripe",
    minorAmount: 1,
    currency: "jpy",
    raw: { status: 3 },
  }

  beforeEach(() => {
    jest.clearAllMocks()
    const mocked = jest.requireMock("hyperswitch-prism")
    paymentClientMock = new mocked.PaymentClient()
    eventClientMock = new mocked.EventClient()
  })

  describe("re-initiation ignores client-controlled ids and state", () => {
    const cases: Array<[string, Record<string, unknown>, string[]]> = [
      ["mollie", { cardToken: "tkn_1", returnUrl: "https://shop/return" }, ["cardToken", "returnUrl"]],
      ["globalpay", { paymentReference: "PMT_1" }, ["paymentReference"]],
      ["cybersource", { transientToken: "jwt_1", email: "a@b.c" }, ["transientToken", "email"]],
      ["authorizedotnet", { cardNumber: "4111111111111111", cardExpMonth: "12", cardExpYear: "2030", cardCvc: "123" }, ["cardNumber", "cardCvc"]],
    ]

    it.each(cases)("%s keeps only allow-listed fields", async (connector, fields, kept) => {
      const result = await reinit(buildService(connector), { ...clientOverrides, ...fields })
      const data = result.data as Record<string, unknown>

      expect(result.id).toBe("payses_server")
      expect(data.id).toBe("payses_server")
      expect(data.merchantClientSessionId).toBe("payses_server")
      expect(data.connectorTransactionId).toBeUndefined()
      expect(data.raw).toBeUndefined()
      expect(data.connector).toBe(connector)
      expect(data.minorAmount).toBe(100000)
      expect(data.currency).toBe("usd")
      for (const key of kept) {
        expect(data[key]).toBe(fields[key])
      }
    })

    it("mollie keeps only string billing fields and re-surfaces the server profileId", async () => {
      const service = buildService("mollie", { profileToken: { value: "pfl_server" } })
      const result = await reinit(service, {
        paymentMethodType: "klarna",
        profileId: "pfl_client",
        billing: { firstName: "A", email: "a@b.c", line1: "1 St", evil: { nested: true } },
      })
      const data = result.data as any
      expect(data.profileId).toBe("pfl_server")
      expect(data.paymentMethodType).toBe("klarna")
      expect(data.billing).toEqual({ firstName: "A", email: "a@b.c", line1: "1 St" })
    })
  })

  describe("mollie authorize", () => {
    it("creates a new payment instead of syncing a client-supplied transaction id", async () => {
      const service = buildService("mollie", { profileToken: { value: "pfl" } })
      const session = await reinit(service, { ...clientOverrides, cardToken: "tkn_1" })
      paymentClientMock.authorize.mockResolvedValue({ status: 6, connectorTransactionId: "tr_new" })

      const result = await service.authorizePayment({ data: session.data } as any)

      expect(paymentClientMock.get).not.toHaveBeenCalled()
      expect(paymentClientMock.authorize).toHaveBeenCalledWith(
        expect.objectContaining({ amount: { minorAmount: 100000, currency: 840 } })
      )
      expect((result.data as any).connectorTransactionId).toBe("tr_new")
    })

    it("syncs against the server-recorded connectorTransactionId, not data.id", async () => {
      const service = buildService("mollie")
      paymentClientMock.get.mockResolvedValue({ status: 3 })

      await service.authorizePayment({
        data: { id: "tr_other", connectorTransactionId: "tr_real", connector: "mollie" },
      } as any)

      expect(paymentClientMock.get).toHaveBeenCalledWith(
        expect.objectContaining({ connectorTransactionId: "tr_real" })
      )
    })
  })

  describe("authorizedotnet raw-card flow", () => {
    it("refuses to boot with environment PRODUCTION", () => {
      expect(() =>
        HyperswitchPrismService.validateOptions({
          connector: "authorizedotnet",
          connectorConfig: {},
          environment: "PRODUCTION",
        } as any)
      ).toThrow(/cannot run with environment PRODUCTION/)
    })

    it("refuses to collect a card with environment PRODUCTION", async () => {
      const service = buildService("authorizedotnet", {}, { environment: "PRODUCTION" })
      await expect(reinit(service, { cardNumber: "4111111111111111" })).rejects.toThrow(
        /cannot run with environment PRODUCTION/
      )
    })

    it("drops the PAN and CVC from session data after authorize", async () => {
      const service = buildService("authorizedotnet")
      paymentClientMock.authorize.mockResolvedValue({ status: 3, connectorTransactionId: "anet_1" })

      const result = await service.authorizePayment({
        data: {
          connector: "authorizedotnet",
          id: "payses_server",
          minorAmount: 1000,
          currency: "usd",
          cardNumber: "4111111111111111",
          cardExpMonth: "12",
          cardExpYear: "2030",
          cardCvc: "123",
        },
      } as any)

      const data = result.data as any
      expect(data.cardNumber).toBeUndefined()
      expect(data.cardCvc).toBeUndefined()
      expect(data.cardLast4).toBe("1111")
      expect(JSON.stringify(data)).not.toContain("4111111111111111")
    })

    it("refunds using the stored last four digits", async () => {
      const service = buildService("authorizedotnet")
      paymentClientMock.refund.mockResolvedValue({ status: 4 })

      await service.refund({
        data: { connectorTransactionId: "anet_1", currency: "usd", cardLast4: "1111", cardExpMonth: "12", cardExpYear: "2030" },
        amount: 10,
      } as any)

      const featureData = JSON.parse(paymentClientMock.refund.mock.calls[0][0].connectorFeatureData.value)
      expect(featureData.creditCard).toEqual({ cardNumber: "1111", expirationDate: "2030-12" })
    })
  })

  describe("webhook sourceVerified handling", () => {
    it.each([
      ["missing", {}],
      ["undefined", { sourceVerified: undefined }],
      ["truthy non-boolean", { sourceVerified: "true" }],
    ])("rejects an event whose sourceVerified is %s", async (_label, verification) => {
      const service = buildService("adyen", {}, { webhookSecret: "ABCDEF01" })
      eventClientMock.handleEvent.mockResolvedValue({
        eventType: 1,
        ...verification,
        eventContent: { paymentsResponse: { merchantTransactionId: "txn_1" } },
      })

      const result = await service.handleWebhook({ rawData: "{}", headers: {} } as any)

      expect(result.action).toBe("not_supported")
      expect(eventClientMock.parseEvent).not.toHaveBeenCalled()
    })
  })

  describe("partial payment outcomes", () => {
    it.each([
      ["PARTIALLY_AUTHORIZED", 2],
      ["PARTIAL_CHARGED", 4],
      ["PARTIAL_CHARGED_AND_CHARGEABLE", 5],
    ])("%s maps to error, not authorized/captured", (_name, status) => {
      expect(mapPrismStatus(status)).toBe("error")
    })

    it("full outcomes still map to authorized/captured", () => {
      expect(mapPrismStatus(1)).toBe("authorized")
      expect(mapPrismStatus(3)).toBe("captured")
    })
  })
})
