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
  }

  class MockPaymentClient {
    get = jest.fn()
    capture = jest.fn()
    refund = jest.fn()
    void = jest.fn()
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

import PrismService from "../../medusa-unified-payment/src/providers/prism"
import { recordWebhookOutcome } from "../../medusa-unified-payment/src/providers/connector/webhook-common"
import { ConnectorError, IntegrationError } from "hyperswitch-prism"

describe("PrismService — remaining methods (unit)", () => {
  let paymentClientMock: any
  let eventClientMock: any

  const buildService = (
    connector: string,
    connectorConfig: any,
    extraOpts: { webhookSecret?: string } = {}
  ) => {
    return new PrismService({
      connector: connector as any,
      connectorConfig,
      environment: "SANDBOX",
      ...extraOpts,
    })
  }

  const baseData = {
    id: "test_txn_id",
    currency: "USD",
    minorAmount: 100000,
    connector: "stripe",
  }

  beforeEach(() => {
    jest.clearAllMocks()
    const mocked = jest.requireMock("hyperswitch-prism")
    paymentClientMock = new mocked.PaymentClient()
    eventClientMock = new mocked.EventClient()
  })

  describe("getPaymentStatus", () => {
    it("maps AUTHORIZED status correctly", async () => {
      const service = buildService("stripe", { apiKey: { value: "sk" } })
      paymentClientMock.get.mockResolvedValue({ status: 1 } as any)
      ;(service as any).paymentClient_ = paymentClientMock

      const result = await service.getPaymentStatus({ data: baseData } as any)

      expect(result.status).toBe("authorized")
      expect(result.data).toMatchObject({ prismStatus: 1 })
    })

    it("maps CHARGED status correctly", async () => {
      const service = buildService("stripe", { apiKey: { value: "sk" } })
      paymentClientMock.get.mockResolvedValue({ status: 3 } as any)
      ;(service as any).paymentClient_ = paymentClientMock

      const result = await service.getPaymentStatus({ data: baseData } as any)

      expect(result.status).toBe("captured")
    })

    it("maps AUTHENTICATION_PENDING to REQUIRES_MORE", async () => {
      const service = buildService("stripe", { apiKey: { value: "sk" } })
      paymentClientMock.get.mockResolvedValue({ status: 6 } as any)
      ;(service as any).paymentClient_ = paymentClientMock

      const result = await service.getPaymentStatus({ data: baseData } as any)

      expect(result.status).toBe("requires_more")
    })

    it("maps VOIDED to CANCELED", async () => {
      const service = buildService("stripe", { apiKey: { value: "sk" } })
      paymentClientMock.get.mockResolvedValue({ status: 10 } as any)
      ;(service as any).paymentClient_ = paymentClientMock

      const result = await service.getPaymentStatus({ data: baseData } as any)

      expect(result.status).toBe("canceled")
    })

    it("maps FAILURE to ERROR", async () => {
      const service = buildService("stripe", { apiKey: { value: "sk" } })
      paymentClientMock.get.mockResolvedValue({ status: 17 } as any)
      ;(service as any).paymentClient_ = paymentClientMock

      const result = await service.getPaymentStatus({ data: baseData } as any)

      expect(result.status).toBe("error")
    })

    it("defaults unknown status to PENDING", async () => {
      const service = buildService("stripe", { apiKey: { value: "sk" } })
      paymentClientMock.get.mockResolvedValue({ status: 999 } as any)
      ;(service as any).paymentClient_ = paymentClientMock

      const result = await service.getPaymentStatus({ data: baseData } as any)

      expect(result.status).toBe("pending")
    })

    it("returns PENDING when no transaction id", async () => {
      const service = buildService("stripe", { apiKey: { value: "sk" } })

      const result = await service.getPaymentStatus({ data: {} } as any)

      expect(result.status).toBe("pending")
      expect(paymentClientMock.get).not.toHaveBeenCalled()
    })

    it("passes amount when available", async () => {
      const service = buildService("stripe", { apiKey: { value: "sk" } })
      paymentClientMock.get.mockResolvedValue({ status: 1 } as any)
      ;(service as any).paymentClient_ = paymentClientMock

      await service.getPaymentStatus({ data: baseData } as any)

      expect(paymentClientMock.get).toHaveBeenCalledWith(
        expect.objectContaining({
          connectorTransactionId: "test_txn_id",
          amount: { minorAmount: 100000, currency: 840 },
        })
      )
    })

    it("handles adyen connector errors gracefully", async () => {
      const service = buildService("adyen", {
        apiKey: { value: "adyen_key" },
        merchantAccount: { value: "merchant" },
      })
      paymentClientMock.get.mockRejectedValue(
        new (ConnectorError as any)({ message: "Not found", httpStatusCode: 404 })
      )
      ;(service as any).paymentClient_ = paymentClientMock

      const result = await service.getPaymentStatus({
        data: { ...baseData, connector: "adyen" },
      } as any)

      expect(result.status).toBe("pending")
    })

    it("re-throws non-adyen connector errors", async () => {
      const service = buildService("stripe", { apiKey: { value: "sk" } })
      paymentClientMock.get.mockRejectedValue(
        new (ConnectorError as any)({ message: "Not found", httpStatusCode: 404 })
      )
      ;(service as any).paymentClient_ = paymentClientMock

      await expect(
        service.getPaymentStatus({ data: baseData } as any)
      ).rejects.toThrow("[Hyperswitch Prism] An error occurred in getPaymentStatus")
    })
  })

  describe("authorizePayment", () => {
    // Make the adyen retry loop's sleeps instant — the loop covers Medusa's
    // 5s webhook subscriber delay in production and would slow unit runs.
    let setTimeoutSpy: jest.SpyInstance
    beforeEach(() => {
      setTimeoutSpy = jest
        .spyOn(global, "setTimeout")
        .mockImplementation(((fn: () => void) => {
          fn()
          return 0
        }) as any)
    })
    afterEach(() => {
      setTimeoutSpy.mockRestore()
    })

    it("returns PENDING for adyen when status stays pending and no webhook arrived", async () => {
      const service = buildService("adyen", {
        apiKey: { value: "adyen_key" },
        merchantAccount: { value: "merchant" },
      })
      paymentClientMock.get.mockResolvedValue({ status: 0 } as any)
      ;(service as any).paymentClient_ = paymentClientMock

      const result = await service.authorizePayment({
        data: { ...baseData, connector: "adyen" },
      } as any)

      expect(result.status).toBe("pending")
    })

    it("returns actual status for adyen when not PENDING", async () => {
      const service = buildService("adyen", {
        apiKey: { value: "adyen_key" },
        merchantAccount: { value: "merchant" },
      })
      paymentClientMock.get.mockResolvedValue({ status: 1 } as any)
      ;(service as any).paymentClient_ = paymentClientMock

      const result = await service.authorizePayment({
        data: { ...baseData, connector: "adyen" },
      } as any)

      expect(result.status).toBe("authorized")
    })

    it("returns the verified webhook outcome for adyen and adopts the pspReference", async () => {
      const service = buildService("adyen", {
        apiKey: { value: "adyen_key" },
        merchantAccount: { value: "merchant" },
      })
      // UCS cannot PSync adyen sessions-flow payments — the get call always fails.
      paymentClientMock.get.mockRejectedValue(
        new (IntegrationError as any)({ message: "encoded_data missing" })
      )
      ;(service as any).paymentClient_ = paymentClientMock

      recordWebhookOutcome({
        action: "authorized",
        data: {
          session_id: "payses_webhook_test",
          amount: 1500,
          connector_transaction_id: "PSP_REF_123",
        } as any,
      } as any)

      const result = await service.authorizePayment({
        data: {
          ...baseData,
          connector: "adyen",
          merchantClientSessionId: "payses_webhook_test",
        },
      } as any)

      expect(result.status).toBe("authorized")
      expect(result.data).toMatchObject({
        id: "PSP_REF_123",
        connectorTransactionId: "PSP_REF_123",
      })
    })

    it("returns PENDING for adyen on persistent status errors", async () => {
      const service = buildService("adyen", {
        apiKey: { value: "adyen_key" },
        merchantAccount: { value: "merchant" },
      })
      paymentClientMock.get.mockRejectedValue(new Error("fail"))
      ;(service as any).paymentClient_ = paymentClientMock

      const result = await service.authorizePayment({
        data: { ...baseData, connector: "adyen" },
      } as any)

      expect(result.status).toBe("pending")
    })

    it("delegates to getPaymentStatus for non-adyen connectors", async () => {
      const service = buildService("stripe", { apiKey: { value: "sk" } })
      paymentClientMock.get.mockResolvedValue({ status: 1 } as any)
      ;(service as any).paymentClient_ = paymentClientMock

      const result = await service.authorizePayment({ data: baseData } as any)

      expect(result.status).toBe("authorized")
    })
  })

  describe("capture", () => {
    it("calls paymentClient.capture with correct params", async () => {
      const service = buildService("stripe", { apiKey: { value: "sk" } })
      paymentClientMock.capture.mockResolvedValue({ status: 3 } as any)
      ;(service as any).paymentClient_ = paymentClientMock

      const result = await service.capture({
        data: baseData,
        context: { idempotency_key: "idem_123" },
      } as any)

      expect(paymentClientMock.capture).toHaveBeenCalledWith(
        expect.objectContaining({
          merchantCaptureId: "idem_123",
          connectorTransactionId: "test_txn_id",
          // UCS rejects captures without amount_to_capture
          amountToCapture: {
            minorAmount: 100000,
            currency: 840,
          },
        })
      )
      expect(result.data!.captureStatus).toBe(3)
    })

    it("generates fallback capture id when no idempotency key", async () => {
      const service = buildService("stripe", { apiKey: { value: "sk" } })
      paymentClientMock.capture.mockResolvedValue({ status: 3 } as any)
      ;(service as any).paymentClient_ = paymentClientMock

      await service.capture({ data: baseData, context: {} } as any)

      const call = paymentClientMock.capture.mock.calls[0][0]
      expect(call.merchantCaptureId).toMatch(/^capt_/)
    })
  })

  describe("refund", () => {
    it("processes refund successfully", async () => {
      const service = buildService("stripe", { apiKey: { value: "sk" } })
      paymentClientMock.refund.mockResolvedValue({ status: 0 } as any)
      ;(service as any).paymentClient_ = paymentClientMock

      const result = await service.refund({
        data: baseData,
        amount: 500,
        context: { idempotency_key: "ref_123" },
      } as any)

      expect(paymentClientMock.refund).toHaveBeenCalledWith(
        expect.objectContaining({
          merchantRefundId: "ref_123",
          connectorTransactionId: "test_txn_id",
          refundAmount: { minorAmount: 50000, currency: 840 },
        })
      )
      expect(result.data!.refundStatus).toBe(0)
    })

    it("throws on refund failure", async () => {
      const service = buildService("stripe", { apiKey: { value: "sk" } })
      paymentClientMock.refund.mockResolvedValue({
        status: 1,
        error: { message: "Insufficient funds" },
      } as any)
      ;(service as any).paymentClient_ = paymentClientMock

      await expect(
        service.refund({ data: baseData, amount: 500, context: {} } as any)
      ).rejects.toThrow(
        "[Hyperswitch Prism] An error occurred in refundPayment: Insufficient funds"
      )
    })

    it("throws on refund transaction failure", async () => {
      const service = buildService("stripe", { apiKey: { value: "sk" } })
      paymentClientMock.refund.mockResolvedValue({ status: 2 } as any)
      ;(service as any).paymentClient_ = paymentClientMock

      await expect(
        service.refund({ data: baseData, amount: 500, context: {} } as any)
      ).rejects.toThrow("Refund failed at connector")
    })
  })

  describe("cancel", () => {
    it("calls paymentClient.void and returns data", async () => {
      const service = buildService("stripe", { apiKey: { value: "sk" } })
      paymentClientMock.void.mockResolvedValue({} as any)
      ;(service as any).paymentClient_ = paymentClientMock

      const result = await service.cancel({
        data: baseData,
        context: { idempotency_key: "void_123" },
      } as any)

      expect(paymentClientMock.void).toHaveBeenCalledWith(
        expect.objectContaining({
          merchantVoidId: "void_123",
          connectorTransactionId: "test_txn_id",
        })
      )
      expect(result.data).toEqual(baseData)
    })

    it("returns data when no transaction id", async () => {
      const service = buildService("stripe", { apiKey: { value: "sk" } })

      const result = await service.cancel({ data: {}, context: {} } as any)

      expect(result.data).toEqual({})
      expect(paymentClientMock.void).not.toHaveBeenCalled()
    })

    it("swallows ConnectorError and returns data", async () => {
      const service = buildService("stripe", { apiKey: { value: "sk" } })
      paymentClientMock.void.mockRejectedValue(
        new (ConnectorError as any)({
          message: "Already voided",
          httpStatusCode: 400,
        })
      )
      ;(service as any).paymentClient_ = paymentClientMock

      const result = await service.cancel({
        data: baseData,
        context: {},
      } as any)

      expect(result.data).toEqual(baseData)
    })

    it("re-throws non-ConnectorError", async () => {
      const service = buildService("stripe", { apiKey: { value: "sk" } })
      paymentClientMock.void.mockRejectedValue(new Error("Network error"))
      ;(service as any).paymentClient_ = paymentClientMock

      await expect(
        service.cancel({ data: baseData, context: {} } as any)
      ).rejects.toThrow("[Hyperswitch Prism] An error occurred in cancelPayment")
    })
  })

  describe("retrieve", () => {
    it("returns raw response with converted amount", async () => {
      const service = buildService("stripe", { apiKey: { value: "sk" } })
      paymentClientMock.get.mockResolvedValue({
        status: 3,
        amount: { minorAmount: 100000, currency: "usd" },
      } as any)
      ;(service as any).paymentClient_ = paymentClientMock

      const result = await service.retrieve({ data: baseData } as any)

      expect(result.data!.raw).toMatchObject({ status: 3, amount: 1000 })
    })

    it("does not convert amount when missing", async () => {
      const service = buildService("stripe", { apiKey: { value: "sk" } })
      paymentClientMock.get.mockResolvedValue({ status: 3 } as any)
      ;(service as any).paymentClient_ = paymentClientMock

      const result = await service.retrieve({ data: baseData } as any)

      expect(result.data!.raw).toEqual({ status: 3 })
    })
  })

  describe("handleWebhook", () => {
    // Webhooks only flow through UCS for adyen/paypal, and a webhook secret
    // is mandatory — stripe & co. short-circuit to NOT_SUPPORTED.
    const adyenConfig = {
      apiKey: { value: "ak" },
      merchantAccount: { value: "ma" },
    }
    const buildWebhookService = (
      connector = "adyen",
      extraOpts: { webhookSecret?: string } = { webhookSecret: "ABCDEF01" }
    ) => {
      const config =
        connector === "paypal"
          ? { clientId: { value: "ci" }, clientSecret: { value: "cs" } }
          : adyenConfig
      const service = buildService(connector, config, extraOpts)
      ;(service as any).eventClient_ = eventClientMock
      return service
    }

    it("returns AUTHORIZED for PAYMENT_INTENT_AUTHORIZATION_SUCCESS", async () => {
      const service = buildWebhookService()
      eventClientMock.handleEvent.mockResolvedValue({
        eventType: 1,
        sourceVerified: true,
        eventContent: {
          paymentsResponse: {
            merchantTransactionId: "txn_123",
            amount: { minorAmount: 100000, currency: "usd" },
          },
        },
      } as any)

      const result = await service.handleWebhook({
        rawData: "{}",
        headers: { "Content-Type": "application/json" },
      } as any)

      expect(result.action).toBe("authorized")
      expect(result.data).toMatchObject({ session_id: "txn_123", amount: 1000 })
    })

    it("returns SUCCESSFUL (captured) for PAYMENT_INTENT_SUCCESS", async () => {
      const service = buildWebhookService()
      eventClientMock.handleEvent.mockResolvedValue({
        eventType: 2,
        sourceVerified: true,
        eventContent: {
          paymentsResponse: {
            connectorTransactionId: "conn_txn_123",
            amount: { minorAmount: 50000, currency: "usd" },
          },
        },
      } as any)

      const result = await service.handleWebhook({
        rawData: Buffer.from("{}"),
        headers: {},
      } as any)

      expect(result.action).toBe("captured")
      expect(result.data).toMatchObject({
        session_id: "conn_txn_123",
        amount: 500,
      })
    })

    it("returns REQUIRES_MORE for PAYMENT_ACTION_REQUIRED", async () => {
      const service = buildWebhookService()
      eventClientMock.handleEvent.mockResolvedValue({ eventType: 4 } as any)

      const result = await service.handleWebhook({
        rawData: "{}",
        headers: {},
      } as any)

      expect(result.action).toBe("requires_more")
    })

    it("returns PENDING for PAYMENT_INTENT_PROCESSING", async () => {
      const service = buildWebhookService()
      eventClientMock.handleEvent.mockResolvedValue({ eventType: 5 } as any)

      const result = await service.handleWebhook({
        rawData: "{}",
        headers: {},
      } as any)

      expect(result.action).toBe("pending")
    })

    it("returns CANCELED for PAYMENT_INTENT_CANCELLED", async () => {
      const service = buildWebhookService()
      eventClientMock.handleEvent.mockResolvedValue({ eventType: 6 } as any)

      const result = await service.handleWebhook({
        rawData: "{}",
        headers: {},
      } as any)

      expect(result.action).toBe("canceled")
    })

    it("returns FAILED for PAYMENT_INTENT_FAILURE", async () => {
      const service = buildWebhookService()
      eventClientMock.handleEvent.mockResolvedValue({ eventType: 7 } as any)

      const result = await service.handleWebhook({
        rawData: "{}",
        headers: {},
      } as any)

      expect(result.action).toBe("failed")
    })

    it("returns NOT_SUPPORTED for unknown event type", async () => {
      const service = buildWebhookService()
      eventClientMock.handleEvent.mockResolvedValue({ eventType: 999 } as any)

      const result = await service.handleWebhook({
        rawData: "{}",
        headers: {},
      } as any)

      expect(result.action).toBe("not_supported")
    })

    it("returns NOT_SUPPORTED when eventType is missing", async () => {
      const service = buildWebhookService()
      eventClientMock.handleEvent.mockResolvedValue({} as any)

      const result = await service.handleWebhook({
        rawData: "{}",
        headers: {},
      } as any)

      expect(result.action).toBe("not_supported")
    })

    it("lowercases headers before passing to eventClient", async () => {
      const service = buildWebhookService("adyen", { webhookSecret: "secret" })
      eventClientMock.handleEvent.mockResolvedValue({ eventType: 1 } as any)

      await service.handleWebhook({
        rawData: "{}",
        headers: { "Content-Type": "application/json", "X-Signature": "sig" },
      } as any)

      expect(eventClientMock.handleEvent).toHaveBeenCalledWith(
        expect.objectContaining({
          requestDetails: expect.objectContaining({
            headers: expect.objectContaining({
              "content-type": "application/json",
              "x-signature": "sig",
            }),
          }),
          webhookSecrets: { secret: "secret" },
        })
      )
    })

    it("prefers ParseEvent's merchant reference as session_id", async () => {
      const service = buildWebhookService()
      eventClientMock.handleEvent.mockResolvedValue({
        eventType: 1,
        sourceVerified: true,
        eventContent: {
          paymentsResponse: { connectorTransactionId: "psp_ref_1" },
        },
      } as any)
      eventClientMock.parseEvent.mockResolvedValue({
        reference: {
          payment: {
            merchantTransactionId: "medusa_session_1",
            connectorTransactionId: "psp_ref_1",
          },
        },
      } as any)

      const result = await service.handleWebhook({
        rawData: "{}",
        headers: {},
      } as any)

      expect(result.action).toBe("authorized")
      expect(result.data).toMatchObject({
        session_id: "medusa_session_1",
        connector_transaction_id: "psp_ref_1",
      })
    })

    it("falls back to HandleEvent ids when ParseEvent fails", async () => {
      const service = buildWebhookService()
      eventClientMock.handleEvent.mockResolvedValue({
        eventType: 1,
        sourceVerified: true,
        eventContent: {
          paymentsResponse: { connectorTransactionId: "psp_ref_2" },
        },
      } as any)
      eventClientMock.parseEvent.mockRejectedValue(new Error("parse boom"))

      const result = await service.handleWebhook({
        rawData: "{}",
        headers: {},
      } as any)

      expect(result.action).toBe("authorized")
      expect(result.data).toMatchObject({ session_id: "psp_ref_2" })
    })

    it("rejects events that fail source verification", async () => {
      const service = buildWebhookService()
      eventClientMock.handleEvent.mockResolvedValue({
        eventType: 1,
        sourceVerified: false,
        eventContent: {
          paymentsResponse: { merchantTransactionId: "txn_123" },
        },
      } as any)

      const result = await service.handleWebhook({
        rawData: "{}",
        headers: {},
      } as any)

      expect(result.action).toBe("not_supported")
      expect(result.data).toBeUndefined()
    })

    it("rejects events without calling UCS when webhookSecret is missing (adyen)", async () => {
      const service = buildWebhookService("adyen", {})

      const result = await service.handleWebhook({
        rawData: "{}",
        headers: {},
      } as any)

      expect(result.action).toBe("not_supported")
      expect(eventClientMock.handleEvent).not.toHaveBeenCalled()
    })

    it("rejects events without calling UCS when webhookSecret is missing (paypal)", async () => {
      const service = buildWebhookService("paypal", {})

      const result = await service.handleWebhook({
        rawData: "{}",
        headers: {},
      } as any)

      expect(result.action).toBe("not_supported")
      expect(eventClientMock.handleEvent).not.toHaveBeenCalled()
    })

    it("acknowledges refund webhooks as NOT_SUPPORTED without throwing", async () => {
      const service = buildWebhookService()
      eventClientMock.handleEvent.mockResolvedValue({
        eventType: 16,
        sourceVerified: true,
        eventContent: {
          refundsResponse: {
            connectorRefundId: "ref_123",
            merchantRefundId: "mref_123",
            status: 4,
          },
        },
      } as any)

      const result = await service.handleWebhook({
        rawData: "{}",
        headers: {},
      } as any)

      expect(result.action).toBe("not_supported")
    })

    it("acknowledges dispute webhooks as NOT_SUPPORTED without throwing (paypal)", async () => {
      const service = buildWebhookService("paypal", {
        webhookSecret: "WH-123",
      })
      eventClientMock.handleEvent.mockResolvedValue({
        eventType: 17,
        sourceVerified: true,
        eventContent: {
          disputesResponse: { connectorDisputeId: "disp_1" },
        },
      } as any)

      const result = await service.handleWebhook({
        rawData: "{}",
        headers: {},
      } as any)

      expect(result.action).toBe("not_supported")
    })

    it("acknowledges webhooks for connectors without UCS webhook support", async () => {
      const service = buildService("stripe", { apiKey: { value: "sk" } })
      ;(service as any).eventClient_ = eventClientMock

      const result = await service.handleWebhook({
        rawData: "{}",
        headers: {},
      } as any)

      expect(result.action).toBe("not_supported")
      expect(eventClientMock.handleEvent).not.toHaveBeenCalled()
    })

    it("throws on unexpected eventClient errors", async () => {
      const service = buildWebhookService()
      eventClientMock.handleEvent.mockRejectedValue(new Error("grpc down"))

      await expect(
        service.handleWebhook({ rawData: "{}", headers: {} } as any)
      ).rejects.toThrow("An error occurred in getWebhookActionAndData")
    })
  })

  describe("updatePayment", () => {
    it("returns input data unchanged", async () => {
      const service = buildService("stripe", { apiKey: { value: "sk" } })

      const result = await service.updatePayment({ data: baseData } as any)

      expect(result.data).toEqual(baseData)
    })
  })

  describe("deletePayment", () => {
    it("returns input data unchanged", async () => {
      const service = buildService("stripe", { apiKey: { value: "sk" } })

      const result = await service.deletePayment({ data: baseData } as any)

      expect(result.data).toEqual(baseData)
    })
  })

  describe("zero-decimal currencies", () => {
    it("converts JPY correctly (no decimal places)", async () => {
      const service = buildService("stripe", { apiKey: { value: "sk" } })
      const mockRes = {
        sessionData: {
          connectorSpecific: {
            stripe: { clientSecret: { value: "pi_jpy_secret_123" } },
          },
        },
      }
      const mocked = jest.requireMock("hyperswitch-prism")
      const authClientMock = new mocked.MerchantAuthenticationClient()
      authClientMock.createClientAuthenticationToken = jest
        .fn()
        .mockResolvedValue(mockRes)
      ;(service as any).authClient_ = authClientMock

      const result = await service.initiatePayment({
        currency_code: "JPY",
        amount: 1000,
        data: {},
        context: {},
      } as any)

      expect(result.data!.minorAmount).toBe(1000)
      expect(authClientMock.createClientAuthenticationToken).toHaveBeenCalledWith(
        expect.objectContaining({
          payment: expect.objectContaining({
            amount: { minorAmount: 1000, currency: 392 },
          }),
        })
      )
    })
  })

  describe("permissions", () => {
    it("passes default permissions for globalpay", async () => {
      const service = buildService("globalpay", {
        appId: { value: "gp_app" },
        appKey: { value: "gp_key" },
      })
      const mocked = jest.requireMock("hyperswitch-prism")
      const authClientMock = new mocked.MerchantAuthenticationClient()
      authClientMock.createClientAuthenticationToken = jest.fn().mockResolvedValue({
        sessionData: {},
      })
      ;(service as any).authClient_ = authClientMock

      await service.initiatePayment({
        currency_code: "USD",
        amount: 1000,
        data: {},
        context: {},
      } as any)

      expect(authClientMock.createClientAuthenticationToken).toHaveBeenCalledWith(
        expect.objectContaining({
          permissions: { values: ["PMT_POST_Create_Single"] },
        })
      )
    })

    it("uses custom permissions from connectorConfig", async () => {
      const service = buildService("globalpay", {
        appId: { value: "gp_app" },
        appKey: { value: "gp_key" },
        permissions: ["PMT_POST_Create_Multiple", "PMT_GET_Single"],
      })
      const mocked = jest.requireMock("hyperswitch-prism")
      const authClientMock = new mocked.MerchantAuthenticationClient()
      authClientMock.createClientAuthenticationToken = jest.fn().mockResolvedValue({
        sessionData: {},
      })
      ;(service as any).authClient_ = authClientMock

      await service.initiatePayment({
        currency_code: "USD",
        amount: 1000,
        data: {},
        context: {},
      } as any)

      expect(authClientMock.createClientAuthenticationToken).toHaveBeenCalledWith(
        expect.objectContaining({
          permissions: { values: ["PMT_POST_Create_Multiple", "PMT_GET_Single"] },
        })
      )
    })

    it("does not pass permissions for stripe by default", async () => {
      const service = buildService("stripe", { apiKey: { value: "sk" } })
      const mocked = jest.requireMock("hyperswitch-prism")
      const authClientMock = new mocked.MerchantAuthenticationClient()
      authClientMock.createClientAuthenticationToken = jest.fn().mockResolvedValue({
        sessionData: {},
      })
      ;(service as any).authClient_ = authClientMock

      await service.initiatePayment({
        currency_code: "USD",
        amount: 1000,
        data: {},
        context: {},
      } as any)

      const callArg = authClientMock.createClientAuthenticationToken.mock.calls[0][0]
      expect(callArg).not.toHaveProperty("permissions")
    })
  })
})
