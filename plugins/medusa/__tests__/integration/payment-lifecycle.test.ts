jest.mock("hyperswitch-prism", () => {
  const mockProto = {
    Environment: { SANDBOX: 1, PRODUCTION: 2 },
    Currency: { USD: 840, EUR: 978, GBP: 826, JPY: 392, CURRENCY_UNSPECIFIED: 0 },
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

import HyperswitchPrismService from "../../medusa-unified-payment/src/providers/service"
import { ConnectorError, IntegrationError } from "hyperswitch-prism"

describe("Payment Lifecycle Integration — Stripe & Adyen", () => {
  const buildStripeService = () =>
    new HyperswitchPrismService(
      {},
      {
        connector: "stripe",
        connectorConfig: { apiKey: { value: "sk_test" } },
        environment: "SANDBOX",
      }
    )

  const buildAdyenService = () =>
    new HyperswitchPrismService(
      {},
      {
        connector: "adyen",
        connectorConfig: {
          apiKey: { value: "adyen_key" },
          merchantAccount: { value: "TestMerchant" },
        },
        environment: "SANDBOX",
      }
    )

  const injectMocks = (service: HyperswitchPrismService) => {
    const mocked = jest.requireMock("hyperswitch-prism")
    const prism = (service as any).prismService_
    prism.paymentClient_ = new mocked.PaymentClient()
    prism.authClient_ = new mocked.MerchantAuthenticationClient()
    prism.eventClient_ = new mocked.EventClient()
    return {
      paymentClient: prism.paymentClient_,
      authClient: prism.authClient_,
      eventClient: prism.eventClient_,
    }
  }

  beforeEach(() => {
    jest.clearAllMocks()
  })

  describe("Stripe", () => {
    it("full lifecycle: initiate → authorize → capture → refund", async () => {
      const service = buildStripeService()
      const { authClient, paymentClient } = injectMocks(service)

      // Step 1: initiate
      authClient.createClientAuthenticationToken.mockResolvedValue({
        sessionData: {
          connectorSpecific: {
            stripe: {
              clientSecret: { value: "pi_lifecycle_secret_abc123" },
            },
          },
        },
      })

      const initiated = await service.initiatePayment({
        currency_code: "USD",
        amount: 1000,
        data: {},
        context: {},
      })
      expect(initiated.status).toBe("pending")
      expect(initiated.data!.client_secret).toBe("pi_lifecycle_secret_abc123")
      expect(initiated.id).toBe("pi_lifecycle")

      // Step 2: authorize
      paymentClient.get.mockResolvedValue({ status: 1 }) // AUTHORIZED

      const authorized = await service.authorizePayment({
        data: initiated.data,
      })
      expect(authorized.status).toBe("authorized")

      // Step 3: capture
      paymentClient.capture.mockResolvedValue({ status: 3 }) // CHARGED

      const captured = await service.capturePayment({
        data: authorized.data,
        context: { idempotency_key: "capt_lifecycle" },
      })
      expect(captured.data!.captureStatus).toBe(3)
      expect(paymentClient.capture).toHaveBeenCalledWith(
        expect.objectContaining({
          merchantCaptureId: "capt_lifecycle",
          connectorTransactionId: "pi_lifecycle",
        })
      )

      // Step 4: refund
      paymentClient.refund.mockResolvedValue({ status: 0 }) // success

      const refunded = await service.refundPayment({
        data: captured.data,
        amount: 500,
        context: { idempotency_key: "ref_lifecycle" },
      })
      expect(refunded.data!.refundStatus).toBe(0)
      expect(paymentClient.refund).toHaveBeenCalledWith(
        expect.objectContaining({
          merchantRefundId: "ref_lifecycle",
          connectorTransactionId: "pi_lifecycle",
          refundAmount: { minorAmount: 50000, currency: 840 },
        })
      )
    })

    it("updatePayment cancels then re-initiates", async () => {
      const service = buildStripeService()
      const { authClient, paymentClient } = injectMocks(service)

      // Original session
      const originalData = {
        id: "pi_update",
        currency: "USD",
        minorAmount: 100000,
        connector: "stripe",
      }

      // Cancel succeeds
      paymentClient.void.mockResolvedValue({})

      // Re-initiate returns new session
      authClient.createClientAuthenticationToken.mockResolvedValue({
        sessionData: {
          connectorSpecific: {
            stripe: {
              clientSecret: { value: "pi_new_secret_xyz" },
            },
          },
        },
      })

      const updated = await service.updatePayment({
        currency_code: "USD",
        amount: 2000,
        data: originalData,
        context: {},
      })

      expect(paymentClient.void).toHaveBeenCalledWith(
        expect.objectContaining({ connectorTransactionId: "pi_update" })
      )
      expect(updated.status).toBe("pending")
      expect(updated.data!.client_secret).toBe("pi_new_secret_xyz")
    })

    it("deletePayment delegates to cancel", async () => {
      const service = buildStripeService()
      const { paymentClient } = injectMocks(service)

      paymentClient.void.mockResolvedValue({})

      const data = {
        id: "pi_delete",
        currency: "USD",
        minorAmount: 100000,
        connector: "stripe",
      }

      const result = await service.deletePayment({ data } as any)

      expect(paymentClient.void).toHaveBeenCalledWith(
        expect.objectContaining({ connectorTransactionId: "pi_delete" })
      )
      expect(result.data).toEqual(data)
    })

    it("handles webhook events end-to-end — returns not_supported (Stripe has no UCS webhook support)", async () => {
      const service = buildStripeService()
      const { eventClient } = injectMocks(service)

      eventClient.handleEvent.mockResolvedValue({
        eventType: 1,
        eventContent: {
          paymentsResponse: {
            merchantTransactionId: "txn_webhook_123",
            amount: { minorAmount: 100000, currency: "usd" },
          },
        },
      })

      const result = await service.getWebhookActionAndData({
        rawData: "{}",
        headers: { "Content-Type": "application/json" },
      } as any)

      // Stripe webhooks are not supported by UCS — acknowledged and ignored
      expect(result.action).toBe("not_supported")
    })

    it("propagates Prism errors through the provider layer", async () => {
      const service = buildStripeService()
      const { authClient } = injectMocks(service)

      authClient.createClientAuthenticationToken.mockRejectedValue(
        new (ConnectorError as any)({
          message: "Invalid API key",
          httpStatusCode: 401,
        })
      )

      await expect(
        service.initiatePayment({
          currency_code: "USD",
          amount: 1000,
          data: {},
          context: {},
        })
      ).rejects.toThrow(
        "[Hyperswitch Prism] An error occurred in initiatePayment: Invalid API key (http: 401)"
      )
    })

    it("propagates IntegrationError through the provider layer", async () => {
      const service = buildStripeService()
      const { authClient } = injectMocks(service)

      authClient.createClientAuthenticationToken.mockRejectedValue(
        new (IntegrationError as any)({
          message: "Missing required field",
          errorCode: "MISSING_REQUIRED_FIELD",
        })
      )

      await expect(
        service.initiatePayment({
          currency_code: "USD",
          amount: 1000,
          data: {},
          context: {},
        })
      ).rejects.toThrow(
        "[Hyperswitch Prism] An error occurred in initiatePayment: Missing required field (code: MISSING_REQUIRED_FIELD)"
      )
    })
  })

  describe("Adyen", () => {
    it("full lifecycle: initiate → authorize (pending, no webhook) → capture → refund", async () => {
      const service = buildAdyenService()
      const { authClient, paymentClient } = injectMocks(service)

      // Step 1: initiate
      authClient.createClientAuthenticationToken.mockResolvedValue({
        sessionData: {
          connectorSpecific: {
            adyen: {
              clientToken: { value: "adyen_lifecycle_token" },
              publishableKey: { value: "adyen_lifecycle_pub" },
            },
          },
        },
      })

      const initiated = await service.initiatePayment({
        currency_code: "USD",
        amount: 1000,
        data: {},
        context: {},
      })
      expect(initiated.status).toBe("pending")
      expect(initiated.data!.clientToken).toBe("adyen_lifecycle_token")
      expect(initiated.id).toBe("adyen_lifecycle_token")

      // Step 2: authorize — no webhook outcome in store; PSync returns PENDING
      // (Adyen sessions flow: outcome arrives only via webhook, not via PSync)
      paymentClient.get.mockResolvedValue({ status: 0 }) // PAYMENT_STATUS_UNSPECIFIED → PENDING

      const authorized = await service.authorizePayment({
        data: { ...initiated.data, connector: "adyen" },
      })
      expect(authorized.status).toBe("pending")

      // Step 3: capture
      paymentClient.capture.mockResolvedValue({ status: 3 })

      const captured = await service.capturePayment({
        data: authorized.data,
        context: { idempotency_key: "capt_adyen" },
      })
      expect(captured.data!.captureStatus).toBe(3)

      // Step 4: refund
      paymentClient.refund.mockResolvedValue({ status: 0 })

      const refunded = await service.refundPayment({
        data: captured.data,
        amount: 500,
        context: { idempotency_key: "ref_adyen" },
      })
      expect(refunded.data!.refundStatus).toBe(0)
    })

    it("authorizePayment returns actual status when not PENDING", async () => {
      const service = buildAdyenService()
      const { paymentClient } = injectMocks(service)

      paymentClient.get.mockResolvedValue({ status: 1 }) // AUTHORIZED

      const result = await service.authorizePayment({
        data: {
          id: "adyen_auth",
          currency: "USD",
          minorAmount: 100000,
          connector: "adyen",
        },
      })

      expect(result.status).toBe("authorized")
    })

    it("authorizePayment returns PENDING on error (transient — webhook outcome is source of truth)", async () => {
      const service = buildAdyenService()
      const { paymentClient } = injectMocks(service)

      paymentClient.get.mockRejectedValue(new Error("network failure"))

      const result = await service.authorizePayment({
        data: {
          id: "adyen_err",
          currency: "USD",
          minorAmount: 100000,
          connector: "adyen",
        },
      })

      // Errors during PSync are treated as transient; Medusa retries via webhook
      expect(result.status).toBe("pending")
    })

    it("getPaymentStatus gracefully handles ConnectorError for adyen", async () => {
      const service = buildAdyenService()
      const { paymentClient } = injectMocks(service)

      paymentClient.get.mockRejectedValue(
        new (ConnectorError as any)({
          message: "Not found",
          httpStatusCode: 404,
        })
      )

      const result = await service.getPaymentStatus({
        data: {
          id: "adyen_missing",
          currency: "USD",
          minorAmount: 100000,
          connector: "adyen",
        },
      })

      expect(result.status).toBe("pending")
    })

    it("getPaymentStatus re-throws ConnectorError for non-adyen connectors", async () => {
      const service = buildStripeService()
      const { paymentClient } = injectMocks(service)

      paymentClient.get.mockRejectedValue(
        new (ConnectorError as any)({
          message: "Not found",
          httpStatusCode: 404,
        })
      )

      await expect(
        service.getPaymentStatus({
          data: {
            id: "stripe_missing",
            currency: "USD",
            minorAmount: 100000,
            connector: "stripe",
          },
        })
      ).rejects.toThrow("[Hyperswitch Prism] An error occurred in getPaymentStatus")
    })

    it("updatePayment cancels then re-initiates for adyen", async () => {
      const service = buildAdyenService()
      const { authClient, paymentClient } = injectMocks(service)

      const originalData = {
        id: "adyen_update",
        currency: "USD",
        minorAmount: 100000,
        connector: "adyen",
      }

      paymentClient.void.mockResolvedValue({})
      authClient.createClientAuthenticationToken.mockResolvedValue({
        sessionData: {
          connectorSpecific: {
            adyen: {
              clientToken: { value: "adyen_new_token" },
            },
          },
        },
      })

      const updated = await service.updatePayment({
        currency_code: "EUR",
        amount: 500,
        data: originalData,
        context: {},
      })

      expect(paymentClient.void).toHaveBeenCalledWith(
        expect.objectContaining({ connectorTransactionId: "adyen_update" })
      )
      expect(updated.status).toBe("pending")
      expect(updated.data!.clientToken).toBe("adyen_new_token")
    })
  })

  describe("Cross-connector shared behaviors", () => {
    it("retrievePayment converts minor amount back for stripe", async () => {
      const service = buildStripeService()
      const { paymentClient } = injectMocks(service)

      paymentClient.get.mockResolvedValue({
        status: 3,
        amount: { minorAmount: 100000, currency: "usd" },
      })

      const result = await service.retrievePayment({
        data: {
          id: "pi_retrieve",
          currency: "USD",
          minorAmount: 100000,
        },
      })

      expect(result.data!.raw).toMatchObject({ status: 3, amount: 1000 })
    })

    it("retrievePayment leaves amount unchanged when missing", async () => {
      const service = buildStripeService()
      const { paymentClient } = injectMocks(service)

      paymentClient.get.mockResolvedValue({ status: 3 })

      const result = await service.retrievePayment({
        data: {
          id: "pi_retrieve",
          currency: "USD",
          minorAmount: 100000,
        },
      })

      expect(result.data!.raw).toEqual({ status: 3 })
    })

    it("cancelPayment swallows ConnectorError and returns data", async () => {
      const service = buildStripeService()
      const { paymentClient } = injectMocks(service)

      paymentClient.void.mockRejectedValue(
        new (ConnectorError as any)({
          message: "Already voided",
          httpStatusCode: 400,
        })
      )

      const data = {
        id: "pi_cancel",
        currency: "USD",
        minorAmount: 100000,
        connector: "stripe",
      }
      const result = await service.cancelPayment({
        data,
        context: {},
      })

      expect(result.data).toEqual(data)
    })

    it("cancelPayment re-throws non-ConnectorError", async () => {
      const service = buildStripeService()
      const { paymentClient } = injectMocks(service)

      paymentClient.void.mockRejectedValue(new Error("Network timeout"))

      await expect(
        service.cancelPayment({
          data: {
            id: "pi_cancel_fail",
            currency: "USD",
            minorAmount: 100000,
            connector: "stripe",
          },
          context: {},
        })
      ).rejects.toThrow("[Hyperswitch Prism] An error occurred in cancelPayment")
    })

    it("handles all webhook event types — all return not_supported (Stripe has no UCS webhook support)", async () => {
      const service = buildStripeService()
      const { eventClient } = injectMocks(service)

      // Stripe webhooks are not supported by UCS regardless of event type —
      // state is driven by the synchronous getPaymentStatus (PSync) flow.
      const eventTypes = [1, 2, 3, 4, 5, 6, 7, 8, 9, 999]

      for (const eventType of eventTypes) {
        eventClient.handleEvent.mockResolvedValue({ eventType })

        const result = await service.getWebhookActionAndData({
          rawData: "{}",
          headers: {},
        } as any)

        expect(result.action).toBe("not_supported")
      }
    })
  })
})
