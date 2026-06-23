jest.mock("hyperswitch-prism", () => {
  const mockProto = {
    Environment: { SANDBOX: 1, PRODUCTION: 2 },
    Currency: { USD: 840, EUR: 978, GBP: 826, CURRENCY_UNSPECIFIED: 0 },
    PaymentMethodType: { PAY_PAL: 98, CARD: 21, GOOGLE_PAY: 38, APPLE_PAY: 9 },
    CountryAlpha2: { US: 1, GB: 77 },
    PaymentStatus: {
      AUTHORIZED: 1,
      CHARGED: 2,
      PENDING: 0,
      AUTHENTICATION_PENDING: 3,
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
    createServerAuthenticationToken = jest.fn()
    createServerSessionAuthenticationToken = jest.fn()
  }

  class MockEventClient {
    handleEvent = jest.fn()
  }

  class MockConnectorError extends Error {
    httpStatusCode?: number
    errorCode?: string
    constructor(message: string, httpStatusCode?: number) {
      super(message)
      this.name = "ConnectorError"
      this.httpStatusCode = httpStatusCode
      this.errorCode = "CONNECTOR_ERROR"
    }
  }

  class MockIntegrationError extends Error {
    errorCode?: string
    constructor(message: string, errorCode?: string) {
      super(message)
      this.name = "IntegrationError"
      this.errorCode = errorCode ?? "INTEGRATION_ERROR"
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
import {
  PaymentClient,
  MerchantAuthenticationClient,
  ConnectorError,
  IntegrationError,
} from "hyperswitch-prism"

describe("PrismService — initiatePayment (unit)", () => {
  let authClientMock: jest.Mocked<InstanceType<typeof MerchantAuthenticationClient>>

  const buildService = (connector: string, connectorConfig: any) => {
    return new PrismService({
      connector: connector as any,
      connectorConfig,
      environment: "SANDBOX",
    })
  }

  const buildInput = (
    currency = "USD",
    amount = 1000,
    extra: { payment_method_type?: string; country_alpha2_code?: string } = {}
  ) =>
    ({
      currency_code: currency,
      amount,
      data: extra,
      context: { return_url: "https://example.com/return" },
    } as any)

  beforeEach(() => {
    jest.clearAllMocks()
    authClientMock = new MerchantAuthenticationClient({} as any) as jest.Mocked<
      InstanceType<typeof MerchantAuthenticationClient>
    >
  })

  describe("stripe", () => {
    it("parses client_secret and derives transaction id", async () => {
      const service = buildService("stripe", { apiKey: { value: "sk_test" } })
      const mockRes = {
        sessionData: {
          connectorSpecific: {
            stripe: {
              clientSecret: { value: "pi_test_secret_abc123" },
            },
          },
        },
      }
      authClientMock.createClientAuthenticationToken.mockResolvedValue(mockRes as any)
      ;(service as any).authClient_ = authClientMock

      const result = await service.initiatePayment(buildInput())

      expect(result.id).toBe("pi_test")
      expect(result.status).toBe("pending")
      expect(result.data!.connector).toBe("stripe")
      expect(result.data!.client_secret).toBe("pi_test_secret_abc123")
      expect(result.data!.currency).toBe("USD")
      expect(result.data!.minorAmount).toBe(100000)
    })

    it("falls back to merchantClientSessionId when no client_secret", async () => {
      const service = buildService("stripe", { apiKey: { value: "sk_test" } })
      const mockRes = {
        sessionData: {
          connectorSpecific: {},
        },
      }
      authClientMock.createClientAuthenticationToken.mockResolvedValue(mockRes as any)
      ;(service as any).authClient_ = authClientMock

      const result = await service.initiatePayment(buildInput())

      expect(result.id).toMatch(/^hs_/)
      expect(result.data!.client_secret).toBeNull()
    })

    it("extracts client_secret from snake_case field", async () => {
      const service = buildService("stripe", { apiKey: { value: "sk_test" } })
      const mockRes = {
        sessionData: {
          connectorSpecific: {
            stripe: {
              client_secret: { value: "pi_snake_secret_abc123" },
            },
          },
        },
      }
      authClientMock.createClientAuthenticationToken.mockResolvedValue(mockRes as any)
      ;(service as any).authClient_ = authClientMock

      const result = await service.initiatePayment(buildInput())

      expect(result.data!.client_secret).toBe("pi_snake_secret_abc123")
      expect(result.id).toBe("pi_snake")
    })

    it("handles plain string client_secret without value wrapper", async () => {
      const service = buildService("stripe", { apiKey: { value: "sk_test" } })
      const mockRes = {
        sessionData: {
          connectorSpecific: {
            stripe: {
              clientSecret: "pi_plain_secret_abc123",
            },
          },
        },
      }
      authClientMock.createClientAuthenticationToken.mockResolvedValue(mockRes as any)
      ;(service as any).authClient_ = authClientMock

      const result = await service.initiatePayment(buildInput())

      expect(result.data!.client_secret).toBe("pi_plain_secret_abc123")
      expect(result.id).toBe("pi_plain")
    })

    it("falls back to connectorSpecific when no nested stripe key", async () => {
      const service = buildService("stripe", { apiKey: { value: "sk_test" } })
      const mockRes = {
        sessionData: {
          connectorSpecific: {
            clientSecret: { value: "pi_fallback_secret_abc123" },
          },
        },
      }
      authClientMock.createClientAuthenticationToken.mockResolvedValue(mockRes as any)
      ;(service as any).authClient_ = authClientMock

      const result = await service.initiatePayment(buildInput())

      expect(result.data!.client_secret).toBe("pi_fallback_secret_abc123")
      expect(result.id).toBe("pi_fallback")
    })
  })

  describe("payment request structure", () => {
    it("passes paymentMethodType and countryAlpha2Code when provided", async () => {
      const service = buildService("stripe", { apiKey: { value: "sk_test" } })
      authClientMock.createClientAuthenticationToken.mockResolvedValue({
        sessionData: {},
      } as any)
      ;(service as any).authClient_ = authClientMock

      await service.initiatePayment(
        buildInput("USD", 1000, {
          payment_method_type: "PAY_PAL",
          country_alpha2_code: "US",
        })
      )

      expect(
        authClientMock.createClientAuthenticationToken
      ).toHaveBeenCalledWith(
        expect.objectContaining({
          payment: expect.objectContaining({
            paymentMethodType: 98,
            countryAlpha2Code: 1,
          }),
        })
      )
    })

    it("omits optional fields when not provided", async () => {
      const service = buildService("stripe", { apiKey: { value: "sk_test" } })
      authClientMock.createClientAuthenticationToken.mockResolvedValue({
        sessionData: {},
      } as any)
      ;(service as any).authClient_ = authClientMock

      await service.initiatePayment(buildInput())

      const callArg =
        authClientMock.createClientAuthenticationToken.mock.calls[0][0]
      expect(callArg.payment).not.toHaveProperty("paymentMethodType")
      expect(callArg.payment).not.toHaveProperty("countryAlpha2Code")
    })
  })

  describe("adyen", () => {
    it("parses clientToken and publishableKey", async () => {
      const service = buildService("adyen", {
        apiKey: { value: "adyen_key" },
        merchantAccount: { value: "TestMerchant" },
      })
      const mockRes = {
        sessionData: {
          connectorSpecific: {
            adyen: {
              clientToken: { value: "adyen_client_token_xyz" },
              publishableKey: { value: "adyen_pub_key" },
            },
          },
        },
      }
      authClientMock.createClientAuthenticationToken.mockResolvedValue(mockRes as any)
      ;(service as any).authClient_ = authClientMock

      const result = await service.initiatePayment(buildInput())

      expect(result.id).toBe("adyen_client_token_xyz")
      expect(result.status).toBe("pending")
      expect(result.data!.connector).toBe("adyen")
      expect(result.data!.clientToken).toBe("adyen_client_token_xyz")
      expect(result.data!.publishableKey).toBe("adyen_pub_key")
    })

    it("falls back to session_id when no clientToken", async () => {
      const service = buildService("adyen", {
        apiKey: { value: "adyen_key" },
        merchantAccount: { value: "TestMerchant" },
      })
      const mockRes = {
        sessionData: {
          connectorSpecific: {},
        },
      }
      authClientMock.createClientAuthenticationToken.mockResolvedValue(mockRes as any)
      ;(service as any).authClient_ = authClientMock

      const result = await service.initiatePayment(buildInput())

      expect(result.id).toMatch(/^hs_/)
      expect(result.data!.clientToken).toBeNull()
      expect(result.data!.publishableKey).toBeNull()
    })

    it("extracts clientToken from snake_case client_token", async () => {
      const service = buildService("adyen", {
        apiKey: { value: "adyen_key" },
        merchantAccount: { value: "TestMerchant" },
      })
      const mockRes = {
        sessionData: {
          connectorSpecific: {
            adyen: {
              client_token: { value: "adyen_snake_token" },
            },
          },
        },
      }
      authClientMock.createClientAuthenticationToken.mockResolvedValue(mockRes as any)
      ;(service as any).authClient_ = authClientMock

      const result = await service.initiatePayment(buildInput())

      expect(result.data!.clientToken).toBe("adyen_snake_token")
      expect(result.id).toBe("adyen_snake_token")
    })

    it("extracts clientToken from sessionId", async () => {
      const service = buildService("adyen", {
        apiKey: { value: "adyen_key" },
        merchantAccount: { value: "TestMerchant" },
      })
      const mockRes = {
        sessionData: {
          connectorSpecific: {
            adyen: {
              sessionId: { value: "adyen_session_id_xyz" },
            },
          },
        },
      }
      authClientMock.createClientAuthenticationToken.mockResolvedValue(mockRes as any)
      ;(service as any).authClient_ = authClientMock

      const result = await service.initiatePayment(buildInput())

      expect(result.data!.clientToken).toBe("adyen_session_id_xyz")
      expect(result.id).toBe("adyen_session_id_xyz")
    })

    it("extracts clientToken from snake_case session_id", async () => {
      const service = buildService("adyen", {
        apiKey: { value: "adyen_key" },
        merchantAccount: { value: "TestMerchant" },
      })
      const mockRes = {
        sessionData: {
          connectorSpecific: {
            adyen: {
              session_id: { value: "adyen_snake_session" },
            },
          },
        },
      }
      authClientMock.createClientAuthenticationToken.mockResolvedValue(mockRes as any)
      ;(service as any).authClient_ = authClientMock

      const result = await service.initiatePayment(buildInput())

      expect(result.data!.clientToken).toBe("adyen_snake_session")
      expect(result.id).toBe("adyen_snake_session")
    })

    it("extracts publishableKey from snake_case publishable_key", async () => {
      const service = buildService("adyen", {
        apiKey: { value: "adyen_key" },
        merchantAccount: { value: "TestMerchant" },
      })
      const mockRes = {
        sessionData: {
          connectorSpecific: {
            adyen: {
              clientToken: { value: "adyen_client_token" },
              publishable_key: { value: "adyen_pub_snake" },
            },
          },
        },
      }
      authClientMock.createClientAuthenticationToken.mockResolvedValue(mockRes as any)
      ;(service as any).authClient_ = authClientMock

      const result = await service.initiatePayment(buildInput())

      expect(result.data!.publishableKey).toBe("adyen_pub_snake")
    })

    it("handles plain string values without value wrapper", async () => {
      const service = buildService("adyen", {
        apiKey: { value: "adyen_key" },
        merchantAccount: { value: "TestMerchant" },
      })
      const mockRes = {
        sessionData: {
          connectorSpecific: {
            adyen: {
              clientToken: "adyen_plain_token",
              publishableKey: "adyen_plain_pub",
            },
          },
        },
      }
      authClientMock.createClientAuthenticationToken.mockResolvedValue(mockRes as any)
      ;(service as any).authClient_ = authClientMock

      const result = await service.initiatePayment(buildInput())

      expect(result.data!.clientToken).toBe("adyen_plain_token")
      expect(result.data!.publishableKey).toBe("adyen_plain_pub")
      expect(result.id).toBe("adyen_plain_token")
    })

    it("falls back to connectorSpecific when no nested adyen key", async () => {
      const service = buildService("adyen", {
        apiKey: { value: "adyen_key" },
        merchantAccount: { value: "TestMerchant" },
      })
      const mockRes = {
        sessionData: {
          connectorSpecific: {
            clientToken: { value: "adyen_fallback_token" },
            publishableKey: { value: "adyen_fallback_pub" },
          },
        },
      }
      authClientMock.createClientAuthenticationToken.mockResolvedValue(mockRes as any)
      ;(service as any).authClient_ = authClientMock

      const result = await service.initiatePayment(buildInput())

      expect(result.data!.clientToken).toBe("adyen_fallback_token")
      expect(result.data!.publishableKey).toBe("adyen_fallback_pub")
      expect(result.id).toBe("adyen_fallback_token")
    })
  })

  describe("paypal", () => {
    it("parses clientToken and sessionToken from sessionData.paypal", async () => {
      const service = buildService("paypal", {
        clientId: { value: "paypal_client_id" },
        clientSecret: { value: "paypal_secret" },
      })
      const mockRes = {
        sessionData: {
          paypal: {
            clientToken: "paypal_client_token_123",
            sessionToken: "paypal_session_token_456",
          },
        },
      }
      authClientMock.createClientAuthenticationToken.mockResolvedValue(mockRes as any)
      ;(service as any).authClient_ = authClientMock

      const result = await service.initiatePayment(buildInput())

      expect(result.id).toMatch(/^hs_/)
      expect(result.status).toBe("pending")
      expect(result.data!.connector).toBe("paypal")
      expect(result.data!.clientToken).toBe("paypal_client_token_123")
      expect(result.data!.sessionToken).toBe("paypal_session_token_456")
      expect(result.data!.currency).toBe("USD")
    })

    it("falls back to connectorSpecific.paypal", async () => {
      const service = buildService("paypal", {
        clientId: { value: "paypal_client_id" },
        clientSecret: { value: "paypal_secret" },
      })
      const mockRes = {
        sessionData: {
          connectorSpecific: {
            paypal: {
              client_token: "paypal_ct_789",
            },
          },
        },
      }
      authClientMock.createClientAuthenticationToken.mockResolvedValue(mockRes as any)
      ;(service as any).authClient_ = authClientMock

      const result = await service.initiatePayment(buildInput())

      expect(result.data!.clientToken).toBe("paypal_ct_789")
    })
  })

  describe("braintree", () => {
    it("passes through raw sessionData", async () => {
      const service = buildService("braintree", {
        publicKey: { value: "bt_pubkey" },
        privateKey: { value: "bt_privkey" },
        paypalClientId: "paypal_client_123",
      })
      const mockRes = {
        sessionData: {
          applePay: {
            sessionResponse: { value: "bt_apple_session" },
          },
        },
      }
      authClientMock.createClientAuthenticationToken.mockResolvedValue(mockRes as any)
      ;(service as any).authClient_ = authClientMock

      const result = await service.initiatePayment(buildInput())

      expect(result.id).toMatch(/^hs_/)
      expect(result.status).toBe("pending")
      expect(result.data!.connector).toBe("braintree")
      expect(result.data!.sessionData).toEqual(mockRes.sessionData)
      expect(result.data!.currency).toBe("USD")
    })

    it("returns paypal session data when paypalClientId is configured", async () => {
      const service = buildService("braintree", {
        publicKey: { value: "bt_pubkey" },
        privateKey: { value: "bt_privkey" },
        paypalClientId: "paypal_client_123",
      })
      const mockRes = {
        sessionData: {
          connectorSpecific: {
            braintree: {
              paypal: {
                sessionToken: "paypal_client_123",
                clientToken: "bt_client_token_456",
              },
            },
          },
        },
      }
      authClientMock.createClientAuthenticationToken.mockResolvedValue(mockRes as any)
      ;(service as any).authClient_ = authClientMock

      const result = await service.initiatePayment(buildInput())

      expect(result.id).toMatch(/^hs_/)
      expect(result.status).toBe("pending")
      expect(result.data!.connector).toBe("braintree")
      expect(result.data!.sessionData).toEqual(mockRes.sessionData)
      expect(result.data!.currency).toBe("USD")
    })
  })

  describe("globalpay", () => {
    it("parses accessToken, tokenType, and expiresIn", async () => {
      const service = buildService("globalpay", {
        appId: { value: "gp_app_id" },
        appKey: { value: "gp_app_key" },
      })
      const mockRes = {
        sessionData: {
          connectorSpecific: {
            globalpay: {
              accessToken: { value: "gp_access_token_123" },
              tokenType: "Bearer",
              expiresIn: 3600,
            },
          },
        },
      }
      authClientMock.createClientAuthenticationToken.mockResolvedValue(mockRes as any)
      ;(service as any).authClient_ = authClientMock

      const result = await service.initiatePayment(buildInput())

      expect(result.id).toMatch(/^hs_/)
      expect(result.status).toBe("pending")
      expect(result.data!.connector).toBe("globalpay")
      expect(result.data!.accessToken).toBe("gp_access_token_123")
      expect(result.data!.tokenType).toBe("Bearer")
      expect(result.data!.expiresIn).toBe(3600)
    })
  })

  describe("cybersource", () => {
    it("parses captureContext, clientLibrary, and clientLibraryIntegrity", async () => {
      const service = buildService("cybersource", {
        apiKey: { value: "cs_api_key" },
        merchantAccount: { value: "cs_merchant" },
        apiSecret: { value: "cs_secret" },
      })
      const mockRes = {
        sessionData: {
          connectorSpecific: {
            cybersource: {
              captureContext: "eyJhbGciOiJIUzI1NiJ9.test",
              clientLibrary: "https://flex.cybersource.com/flex.js",
              clientLibraryIntegrity: "sha384-abc123",
            },
          },
        },
      }
      authClientMock.createClientAuthenticationToken.mockResolvedValue(mockRes as any)
      ;(service as any).authClient_ = authClientMock

      const result = await service.initiatePayment(buildInput())

      expect(result.id).toMatch(/^hs_/)
      expect(result.status).toBe("pending")
      expect(result.data!.connector).toBe("cybersource")
      expect(result.data!.captureContext).toBe("eyJhbGciOiJIUzI1NiJ9.test")
      expect(result.data!.clientLibrary).toBe("https://flex.cybersource.com/flex.js")
      expect(result.data!.clientLibraryIntegrity).toBe("sha384-abc123")
    })
  })

  describe("mollie", () => {
    it("parses paymentId and checkoutUrl", async () => {
      const service = buildService("mollie", {
        apiKey: { value: "mollie_key" },
      })
      const mockRes = {
        sessionData: {
          connectorSpecific: {
            mollie: {
              paymentId: "tr_test_123",
              checkoutUrl: { value: "https://www.mollie.com/checkout/test" },
            },
          },
        },
      }
      authClientMock.createClientAuthenticationToken.mockResolvedValue(mockRes as any)
      ;(service as any).authClient_ = authClientMock

      const result = await service.initiatePayment(buildInput())

      expect(result.id).toBe("tr_test_123")
      expect(result.status).toBe("pending")
      expect(result.data!.connector).toBe("mollie")
      expect(result.data!.paymentId).toBe("tr_test_123")
      expect(result.data!.checkoutUrl).toBe("https://www.mollie.com/checkout/test")
    })

    it("falls back to merchantClientSessionId when no paymentId", async () => {
      const service = buildService("mollie", {
        apiKey: { value: "mollie_key" },
      })
      const mockRes = {
        sessionData: {
          connectorSpecific: {},
        },
      }
      authClientMock.createClientAuthenticationToken.mockResolvedValue(mockRes as any)
      ;(service as any).authClient_ = authClientMock

      const result = await service.initiatePayment(buildInput())

      expect(result.id).toMatch(/^hs_/)
      expect(result.data!.paymentId).toBeUndefined()
      expect(result.data!.checkoutUrl).toBeNull()
    })
  })

  describe("error handling", () => {
    it("wraps ConnectorError with descriptive message", async () => {
      const service = buildService("stripe", { apiKey: { value: "sk_test" } })
      authClientMock.createClientAuthenticationToken.mockRejectedValue(
        new (ConnectorError as any)("API error", 500)
      )
      ;(service as any).authClient_ = authClientMock

      await expect(service.initiatePayment(buildInput())).rejects.toThrow(
        "[Hyperswitch Prism] An error occurred in initiatePayment: API error (http: 500)"
      )
    })

    it("wraps IntegrationError with descriptive message", async () => {
      const service = buildService("stripe", { apiKey: { value: "sk_test" } })
      authClientMock.createClientAuthenticationToken.mockRejectedValue(
        new (IntegrationError as any)("Missing required field", "MISSING_REQUIRED_FIELD")
      )
      ;(service as any).authClient_ = authClientMock

      await expect(service.initiatePayment(buildInput())).rejects.toThrow(
        "[Hyperswitch Prism] An error occurred in initiatePayment: Missing required field (code: MISSING_REQUIRED_FIELD)"
      )
    })

    it("wraps generic Error with descriptive message", async () => {
      const service = buildService("stripe", { apiKey: { value: "sk_test" } })
      authClientMock.createClientAuthenticationToken.mockRejectedValue(
        new Error("Network failure")
      )
      ;(service as any).authClient_ = authClientMock

      await expect(service.initiatePayment(buildInput())).rejects.toThrow(
        "[Hyperswitch Prism] An error occurred in initiatePayment: Network failure"
      )
    })
  })

  describe("fallback for unknown connector", () => {
    it("passes through raw session data", async () => {
      const service = buildService("shift4", {
        apiKey: { value: "shift4_key" },
      })
      const mockRes = {
        sessionData: {
          connectorSpecific: {
            shift4: { token: "shift4_token" },
          },
        },
      }
      authClientMock.createClientAuthenticationToken.mockResolvedValue(mockRes as any)
      ;(service as any).authClient_ = authClientMock

      const result = await service.initiatePayment(buildInput())

      expect(result.id).toMatch(/^hs_/)
      expect(result.status).toBe("pending")
      expect(result.data!.connector).toBe("shift4")
      expect(result.data!.sessionData).toEqual(mockRes.sessionData)
    })
  })

  describe("returnUrl propagation", () => {
    it("passes returnUrl to createClientAuthenticationToken when provided", async () => {
      const service = buildService("stripe", { apiKey: { value: "sk_test" } })
      authClientMock.createClientAuthenticationToken.mockResolvedValue({
        sessionData: {},
      } as any)
      ;(service as any).authClient_ = authClientMock

      await service.initiatePayment(buildInput())

      expect(authClientMock.createClientAuthenticationToken).toHaveBeenCalledWith(
        expect.objectContaining({
          payment: expect.objectContaining({
            returnUrl: "https://example.com/return",
          }),
        })
      )
    })

    it("omits returnUrl when not provided", async () => {
      const service = buildService("stripe", { apiKey: { value: "sk_test" } })
      authClientMock.createClientAuthenticationToken.mockResolvedValue({
        sessionData: {},
      } as any)
      ;(service as any).authClient_ = authClientMock

      await service.initiatePayment({
        currency_code: "USD",
        amount: 1000,
        data: {},
        context: {},
      } as any)

      const callArg = authClientMock.createClientAuthenticationToken.mock.calls[0][0]
      expect(callArg.payment).not.toHaveProperty("returnUrl")
    })
  })
})
