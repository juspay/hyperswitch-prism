/** Zod input schemas for every tool. All `.strict()` so unknown keys are rejected. */
import { z } from "zod";
import {
  SUPPORTED_FRAMEWORKS,
  SUPPORTED_FLOWS,
  SUPPORTED_LANGUAGES,
  CAPTURE_METHODS,
} from "./constants.js";

export const frameworkSchema = z.enum(SUPPORTED_FRAMEWORKS);
export const flowSchema = z.enum(SUPPORTED_FLOWS);
export const languageSchema = z.enum(SUPPORTED_LANGUAGES);
export const captureMethodSchema = z.enum(CAPTURE_METHODS);

export const integrationGuideShape = {
  framework: frameworkSchema.describe("Target web framework for the integration."),
  flows: z
    .array(flowSchema)
    .nonempty()
    .default(["authorize", "capture", "refund"])
    .describe("Payment flows to include in the plan, e.g. ['authorize','capture','refund','void','webhook']."),
  connector: z
    .string()
    .optional()
    .describe("Optional connector/processor machine name (e.g. 'stripe') to tailor the plan."),
} as const;
export const integrationGuideSchema = z.object(integrationGuideShape).strict();

export const listConnectorsShape = {
  search: z
    .string()
    .optional()
    .describe("Optional case-insensitive substring filter over connector name / display name."),
} as const;
export const listConnectorsSchema = z.object(listConnectorsShape).strict();

export const connectorRequirementsShape = {
  connector: z
    .string()
    .min(1)
    .describe("Connector/processor machine name, e.g. 'stripe', 'authorizedotnet', 'adyen'."),
} as const;
export const connectorRequirementsSchema = z.object(connectorRequirementsShape).strict();

export const scaffoldIntegrationShape = {
  framework: frameworkSchema.describe("Target web framework."),
  connector: z.string().min(1).describe("Connector machine name, e.g. 'stripe'."),
  flows: z
    .array(flowSchema)
    .nonempty()
    .default(["authorize"])
    .describe("Flows to scaffold: authorize, capture, void, refund, sync."),
  language: languageSchema.default("ts").describe("Output language: 'ts' or 'js'."),
  captureMethod: captureMethodSchema
    .default("AUTOMATIC")
    .describe("AUTOMATIC (charge immediately) or MANUAL (authorize then capture separately)."),
} as const;
export const scaffoldIntegrationSchema = z.object(scaffoldIntegrationShape).strict();

export const generateConfigShape = {
  connector: z.string().min(1).describe("Connector machine name."),
  values: z
    .record(z.string())
    .optional()
    .describe(
      "Optional map of camelCase field name -> value. Omitted fields become placeholders. Secrets are kept in .env, never inlined.",
    ),
  environment: z
    .enum(["sandbox", "production"])
    .default("sandbox")
    .describe("Target environment."),
} as const;
export const generateConfigSchema = z.object(generateConfigShape).strict();

export const validateConfigShape = {
  connector: z.string().min(1).describe("Connector machine name the config is for."),
  config: z
    .unknown()
    .describe(
      "Candidate config to validate. Accepts either the full { connectorConfig: { <name>: {...} } } shape or just the inner per-connector object.",
    ),
} as const;
export const validateConfigSchema = z.object(validateConfigShape).strict();

export const explainErrorShape = {
  errorClass: z
    .enum(["IntegrationError", "ConnectorError", "NetworkError"])
    .optional()
    .describe("The thrown error class name, if known."),
  code: z
    .string()
    .optional()
    .describe("Error code string, e.g. MISSING_REQUIRED_FIELD, CONNECT_TIMEOUT, INVALID_CONFIGURATION."),
  message: z.string().optional().describe("Raw error message text."),
  status: z
    .number()
    .int()
    .optional()
    .describe("Numeric payment/refund status from a response body (e.g. 21 for FAILURE)."),
  kind: z
    .enum(["payment", "refund"])
    .optional()
    .describe("Whether `status` is a payment or refund status."),
} as const;
export const explainErrorSchema = z.object(explainErrorShape).strict();

export const statusReferenceShape = {
  status_code: z.number().int().optional().describe("Optional specific numeric status code to explain."),
  kind: z.enum(["payment", "refund"]).optional().describe("Limit to payment or refund statuses."),
} as const;
export const statusReferenceSchema = z.object(statusReferenceShape).strict();

export const doctorShape = {} as const;
export const doctorSchema = z.object(doctorShape).strict();

export const testChargeShape = {
  connector: z.string().min(1).describe("Connector machine name to charge against in sandbox."),
  amount: z
    .number()
    .int()
    .positive()
    .describe("Amount in MINOR units (e.g. 1000 = $10.00)."),
  currency: z.string().min(3).max(3).default("USD").describe("ISO 4217 currency code, e.g. 'USD'."),
  card: z
    .object({
      number: z.string().min(8),
      expMonth: z.string(),
      expYear: z.string(),
      cvc: z.string(),
      holderName: z.string().optional(),
    })
    .strict()
    .optional()
    .describe("Sandbox test card. If omitted, the connector's default sandbox card is used."),
  captureMethod: captureMethodSchema.default("AUTOMATIC").describe("AUTOMATIC or MANUAL."),
  test_mode: z
    .boolean()
    .default(true)
    .describe("MUST be true. The tool hard-fails if false — it only ever runs sandbox charges."),
} as const;
export const testChargeSchema = z.object(testChargeShape).strict();
