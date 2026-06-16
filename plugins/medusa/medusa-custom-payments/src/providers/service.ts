import {
  AbstractPaymentProvider,
  isDefined,
} from "@medusajs/framework/utils"
import {
  AuthorizePaymentInput,
  AuthorizePaymentOutput,
  CancelPaymentInput,
  CancelPaymentOutput,
  CapturePaymentInput,
  CapturePaymentOutput,
  DeletePaymentInput,
  DeletePaymentOutput,
  GetPaymentStatusInput,
  GetPaymentStatusOutput,
  InitiatePaymentInput,
  InitiatePaymentOutput,
  ProviderWebhookPayload,
  RefundPaymentInput,
  RefundPaymentOutput,
  RetrievePaymentInput,
  RetrievePaymentOutput,
  UpdatePaymentInput,
  UpdatePaymentOutput,
  WebhookActionResult,
} from "@medusajs/framework/types"
import { HyperswitchPrismOptions } from "./types"
import PrismService from "./prism"

class HyperswitchPrismService extends AbstractPaymentProvider<HyperswitchPrismOptions> {
  static identifier = "hyperswitch-prism"

  protected options_: HyperswitchPrismOptions
  protected prismService_: PrismService

  constructor(
    cradle: Record<string, unknown>,
    options: HyperswitchPrismOptions
  ) {
    // @ts-ignore
    super(...arguments)
    this.options_ = options
    this.prismService_ = new PrismService(options, cradle)
  }

  static validateOptions(options: HyperswitchPrismOptions): void {
    if (!isDefined(options.connector)) {
      throw new Error(
        "Required option `connector` is missing in Hyperswitch Prism provider"
      )
    }
    if (!isDefined(options.connectorConfig)) {
      throw new Error(
        "Required option `connectorConfig` is missing in Hyperswitch Prism provider"
      )
    }
  }

  async initiatePayment(
    input: InitiatePaymentInput
  ): Promise<InitiatePaymentOutput> {
    return this.prismService_.initiatePayment(input)
  }

  async authorizePayment(
    input: AuthorizePaymentInput
  ): Promise<AuthorizePaymentOutput> {
    return this.prismService_.authorizePayment(input)
  }

  async getPaymentStatus(
    input: GetPaymentStatusInput
  ): Promise<GetPaymentStatusOutput> {
    return this.prismService_.getPaymentStatus(input)
  }

  async capturePayment(
    input: CapturePaymentInput
  ): Promise<CapturePaymentOutput> {
    return this.prismService_.capture(input)
  }

  async refundPayment(input: RefundPaymentInput): Promise<RefundPaymentOutput> {
    return this.prismService_.refund(input)
  }

  async cancelPayment(input: CancelPaymentInput): Promise<CancelPaymentOutput> {
    return this.prismService_.cancel(input)
  }

  async deletePayment(
    input: DeletePaymentInput
  ): Promise<DeletePaymentOutput> {
    return this.cancelPayment(input)
  }

  async retrievePayment(
    input: RetrievePaymentInput
  ): Promise<RetrievePaymentOutput> {
    return this.prismService_.retrieve(input)
  }

  async updatePayment(input: UpdatePaymentInput): Promise<UpdatePaymentOutput> {
    await this.cancelPayment(input)
    return this.initiatePayment(input) as Promise<UpdatePaymentOutput>
  }

  async getWebhookActionAndData(
    webhookData: ProviderWebhookPayload["payload"]
  ): Promise<WebhookActionResult> {
    return this.prismService_.handleWebhook(webhookData)
  }
}

export default HyperswitchPrismService
