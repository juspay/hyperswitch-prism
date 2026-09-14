import HyperswitchPrismService from "../../medusa-custom-payments/src/providers/service"

describe("HyperswitchPrismService", () => {
  const baseOptions = {
    connector: "stripe" as const,
    connectorConfig: { apiKey: { value: "sk_test" } },
    environment: "SANDBOX" as const,
  }

  const createMockPrismService = () => ({
    initiatePayment: jest.fn(),
    authorizePayment: jest.fn(),
    getPaymentStatus: jest.fn(),
    capture: jest.fn(),
    refund: jest.fn(),
    cancel: jest.fn(),
    retrieve: jest.fn(),
    updatePayment: jest.fn(),
    handleWebhook: jest.fn(),
    deletePayment: jest.fn(),
  })

  const buildService = () => {
    const service = new HyperswitchPrismService({}, baseOptions)
    return { service, mockPrism: createMockPrismService() }
  }

  describe("validateOptions", () => {
    it("does not throw when all required options are present", () => {
      expect(() =>
        HyperswitchPrismService.validateOptions(baseOptions)
      ).not.toThrow()
    })

    it("throws when connector is missing", () => {
      expect(() =>
        HyperswitchPrismService.validateOptions({
          ...baseOptions,
          connector: undefined as any,
        })
      ).toThrow("Required option `connector` is missing")
    })

    it("throws when connectorConfig is missing", () => {
      expect(() =>
        HyperswitchPrismService.validateOptions({
          ...baseOptions,
          connectorConfig: undefined as any,
        })
      ).toThrow("Required option `connectorConfig` is missing")
    })
  })

  describe("identifier", () => {
    it("has the correct static identifier", () => {
      expect(HyperswitchPrismService.identifier).toBe("hyperswitch-prism")
    })
  })

  describe("method delegation to PrismService", () => {
    it("delegates initiatePayment to prismService", async () => {
      const { service, mockPrism } = buildService()
      ;(service as any).prismService_ = mockPrism
      const expected = { id: "test", status: "pending", data: {} }
      mockPrism.initiatePayment.mockResolvedValue(expected)

      const input = { currency_code: "USD", amount: 1000, data: {}, context: {} }
      const result = await service.initiatePayment(input as any)

      expect(mockPrism.initiatePayment).toHaveBeenCalledWith(input)
      expect(result).toBe(expected)
    })

    it("delegates authorizePayment to prismService", async () => {
      const { service, mockPrism } = buildService()
      ;(service as any).prismService_ = mockPrism
      const expected = { data: {}, status: "authorized" }
      mockPrism.authorizePayment.mockResolvedValue(expected)

      const input = { data: { id: "txn" } }
      const result = await service.authorizePayment(input as any)

      expect(mockPrism.authorizePayment).toHaveBeenCalledWith(input)
      expect(result).toBe(expected)
    })

    it("delegates getPaymentStatus to prismService", async () => {
      const { service, mockPrism } = buildService()
      ;(service as any).prismService_ = mockPrism
      const expected = { data: {}, status: "pending" }
      mockPrism.getPaymentStatus.mockResolvedValue(expected)

      const input = { data: { id: "txn" } }
      const result = await service.getPaymentStatus(input as any)

      expect(mockPrism.getPaymentStatus).toHaveBeenCalledWith(input)
      expect(result).toBe(expected)
    })

    it("delegates capturePayment to prismService.capture", async () => {
      const { service, mockPrism } = buildService()
      ;(service as any).prismService_ = mockPrism
      const expected = { data: {} }
      mockPrism.capture.mockResolvedValue(expected)

      const input = { data: { id: "txn" }, context: {} }
      const result = await service.capturePayment(input as any)

      expect(mockPrism.capture).toHaveBeenCalledWith(input)
      expect(result).toBe(expected)
    })

    it("delegates refundPayment to prismService.refund", async () => {
      const { service, mockPrism } = buildService()
      ;(service as any).prismService_ = mockPrism
      const expected = { data: {} }
      mockPrism.refund.mockResolvedValue(expected)

      const input = { data: { id: "txn" }, amount: 100, context: {} }
      const result = await service.refundPayment(input as any)

      expect(mockPrism.refund).toHaveBeenCalledWith(input)
      expect(result).toBe(expected)
    })

    it("delegates cancelPayment to prismService.cancel", async () => {
      const { service, mockPrism } = buildService()
      ;(service as any).prismService_ = mockPrism
      const expected = { data: {} }
      mockPrism.cancel.mockResolvedValue(expected)

      const input = { data: { id: "txn" }, context: {} }
      const result = await service.cancelPayment(input as any)

      expect(mockPrism.cancel).toHaveBeenCalledWith(input)
      expect(result).toBe(expected)
    })

    it("delegates deletePayment to prismService.cancel", async () => {
      const { service, mockPrism } = buildService()
      ;(service as any).prismService_ = mockPrism
      const expected = { data: {} }
      mockPrism.cancel.mockResolvedValue(expected)

      const input = { data: { id: "txn" } }
      const result = await service.deletePayment(input as any)

      expect(mockPrism.cancel).toHaveBeenCalledWith(input)
      expect(result).toBe(expected)
    })

    it("delegates retrievePayment to prismService.retrieve", async () => {
      const { service, mockPrism } = buildService()
      ;(service as any).prismService_ = mockPrism
      const expected = { data: {} }
      mockPrism.retrieve.mockResolvedValue(expected)

      const input = { data: { id: "txn" } }
      const result = await service.retrievePayment(input as any)

      expect(mockPrism.retrieve).toHaveBeenCalledWith(input)
      expect(result).toBe(expected)
    })

    it("delegates updatePayment to cancel then initiate", async () => {
      const { service, mockPrism } = buildService()
      ;(service as any).prismService_ = mockPrism
      const cancelResult = { data: {} }
      const initiateResult = { id: "new", status: "pending", data: {} }
      mockPrism.cancel.mockResolvedValue(cancelResult)
      mockPrism.initiatePayment.mockResolvedValue(initiateResult)

      const input = { currency_code: "USD", amount: 1000, data: {}, context: {} }
      const result = await service.updatePayment(input as any)

      expect(mockPrism.cancel).toHaveBeenCalledWith(input)
      expect(mockPrism.initiatePayment).toHaveBeenCalledWith(input)
      expect(result).toBe(initiateResult)
    })

    it("delegates getWebhookActionAndData to prismService.handleWebhook", async () => {
      const { service, mockPrism } = buildService()
      ;(service as any).prismService_ = mockPrism
      const expected = { action: "authorized", data: {} }
      mockPrism.handleWebhook.mockResolvedValue(expected)

      const input = { rawData: "{}", headers: {} }
      const result = await service.getWebhookActionAndData(input as any)

      expect(mockPrism.handleWebhook).toHaveBeenCalledWith(input)
      expect(result).toBe(expected)
    })
  })
})
