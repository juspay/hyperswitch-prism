/**
 * Code scaffolder. Produces a set of { path, contents } files for a given
 * framework x connector x flows x language. Returns code only — the calling agent
 * writes the files. All SDK symbols, request shapes, and numeric status checks are
 * grounded in payment.proto / the real SDK surface (no invented fields).
 */
import type { Connector, ConnectorField } from "../data/connectors.js";
import { envVarName, requiredFields } from "../data/connectors.js";
import type { Framework, Flow, Language, CaptureMethodName } from "../constants.js";

export interface ScaffoldFile {
  path: string;
  contents: string;
}

export interface ScaffoldCtx {
  connector: Connector;
  framework: Framework;
  language: Language;
  flows: Flow[];
  captureMethod: CaptureMethodName;
}

const CODE_FLOWS: Flow[] = ["authorize", "capture", "void", "refund", "sync"];

function ext(lang: Language): string {
  return lang === "ts" ? "ts" : "js";
}

/** Build the connectorConfig literal body (env-driven), e.g. `apiKey: { value: process.env.STRIPE_API_KEY ?? "" }`. */
function connectorConfigBody(connector: Connector, indent: string): string {
  const lines = requiredFields(connector).map((f: ConnectorField) => {
    const env = `process.env.${envVarName(connector.connector, f)} ?? ""`;
    return f.secret ? `${indent}${f.name}: { value: ${env} },` : `${indent}${f.name}: ${env},`;
  });
  return lines.join("\n");
}

// ---------------------------------------------------------------------------
// Client module
// ---------------------------------------------------------------------------
function clientModule(ctx: ScaffoldCtx): ScaffoldFile {
  const { connector, language } = ctx;
  const e = ext(language);
  const envSetup = language === "ts" ? "" : "";
  const contents =
    `// Prism payment client for ${connector.displayName} (${connector.connector}).\n` +
    `// Credentials are read from environment variables — never hard-code secrets.\n` +
    `import { PaymentClient, types } from "hyperswitch-prism";\n\n` +
    `${envSetup}export const paymentClient = new PaymentClient({\n` +
    `  connectorConfig: {\n` +
    `    ${connector.connector}: {\n` +
    connectorConfigBody(connector, "      ") +
    `\n    },\n` +
    `  },\n` +
    `  options: { environment: types.Environment.SANDBOX },\n` +
    `});\n\n` +
    `export { types };\n`;
  return { path: `src/payments/prismClient.${e}`, contents };
}

// ---------------------------------------------------------------------------
// Flow handlers (framework-agnostic business logic)
// ---------------------------------------------------------------------------
function authorizeFn(ctx: ScaffoldCtx): string {
  const ts = ctx.language === "ts";
  const sig = ts ? "(input: AuthorizeInput)" : "(input)";
  const successStatus =
    ctx.captureMethod === "AUTOMATIC" ? "types.PaymentStatus.CHARGED /* 8 */" : "types.PaymentStatus.AUTHORIZED /* 6 */";
  const inputType = ts
    ? `export interface AuthorizeInput {\n` +
      `  merchantTransactionId: string;\n` +
      `  minorAmount: number; // e.g. 1000 = $10.00\n` +
      `  currency: string;    // ISO 4217, e.g. "USD"\n` +
      `  card: { number: string; expMonth: string; expYear: string; cvc: string; holderName?: string };\n` +
      `}\n\n`
    : "";
  return (
    inputType +
    `export async function authorizePayment${sig} {\n` +
    `  const res = await paymentClient.authorize({\n` +
    `    merchantTransactionId: input.merchantTransactionId, // your idempotency reference\n` +
    `    amount: { minorAmount: input.minorAmount, currency: toCurrency(input.currency) },\n` +
    `    captureMethod: types.CaptureMethod.${ctx.captureMethod}, // ${ctx.captureMethod === "AUTOMATIC" ? "charge immediately" : "authorize then capture later"}\n` +
    `    paymentMethod: {\n` +
    `      card: {\n` +
    `        cardNumber: { value: input.card.number },\n` +
    `        cardExpMonth: { value: input.card.expMonth },\n` +
    `        cardExpYear: { value: input.card.expYear },\n` +
    `        cardCvc: { value: input.card.cvc },\n` +
    `        cardHolderName: { value: input.card.holderName ?? "" },\n` +
    `      },\n` +
    `    },\n` +
    `    address: { billingAddress: {} },\n` +
    `    authType: types.AuthenticationType.NO_THREE_DS,\n` +
    `    orderDetails: [],\n` +
    `    testMode: true, // sandbox\n` +
    `  });\n\n` +
    `  // response.status is a NUMBER. Soft declines come back as FAILURE (21) here (no throw).\n` +
    `  const success = res.status === ${successStatus};\n` +
    `  if (res.status === types.PaymentStatus.FAILURE /* 21 */) {\n` +
    `    return { success: false, status: res.status, declined: true, transactionId: res.connectorTransactionId, raw: res };\n` +
    `  }\n` +
    `  return { success, status: res.status, transactionId: res.connectorTransactionId, raw: res };\n` +
    `}\n`
  );
}

function captureFn(ctx: ScaffoldCtx): string {
  const ts = ctx.language === "ts";
  const sig = ts ? "(input: CaptureInput)" : "(input)";
  const inputType = ts
    ? `export interface CaptureInput {\n  connectorTransactionId: string;\n  minorAmount: number;\n  currency: string;\n  merchantCaptureId?: string;\n}\n\n`
    : "";
  return (
    inputType +
    `export async function capturePayment${sig} {\n` +
    `  const res = await paymentClient.capture({\n` +
    `    merchantCaptureId: input.merchantCaptureId ?? \`cap_\${input.connectorTransactionId}\`,\n` +
    `    connectorTransactionId: input.connectorTransactionId,\n` +
    `    amountToCapture: { minorAmount: input.minorAmount, currency: toCurrency(input.currency) },\n` +
    `    testMode: true,\n` +
    `  });\n` +
    `  return { success: res.status === types.PaymentStatus.CHARGED /* 8 */, status: res.status, raw: res };\n` +
    `}\n`
  );
}

function voidFn(ctx: ScaffoldCtx): string {
  const ts = ctx.language === "ts";
  const sig = ts ? "(input: VoidInput)" : "(input)";
  const inputType = ts
    ? `export interface VoidInput {\n  connectorTransactionId: string;\n  cancellationReason?: string;\n  merchantVoidId?: string;\n}\n\n`
    : "";
  return (
    inputType +
    `export async function voidPayment${sig} {\n` +
    `  const res = await paymentClient.void({\n` +
    `    merchantVoidId: input.merchantVoidId ?? \`void_\${input.connectorTransactionId}\`,\n` +
    `    connectorTransactionId: input.connectorTransactionId,\n` +
    `    cancellationReason: input.cancellationReason ?? "requested_by_customer",\n` +
    `    testMode: true,\n` +
    `  });\n` +
    `  return { success: res.status === types.PaymentStatus.VOIDED /* 11 */, status: res.status, raw: res };\n` +
    `}\n`
  );
}

function refundFn(ctx: ScaffoldCtx): string {
  const ts = ctx.language === "ts";
  const sig = ts ? "(input: RefundInput)" : "(input)";
  const inputType = ts
    ? `export interface RefundInput {\n  connectorTransactionId: string;\n  paymentMinorAmount: number; // original captured amount\n  refundMinorAmount: number;  // amount to refund\n  currency: string;\n  reason?: string;\n  merchantRefundId?: string;\n}\n\n`
    : "";
  return (
    inputType +
    `export async function refundPayment${sig} {\n` +
    `  const res = await paymentClient.refund({\n` +
    `    merchantRefundId: input.merchantRefundId ?? \`ref_\${input.connectorTransactionId}\`,\n` +
    `    connectorTransactionId: input.connectorTransactionId,\n` +
    `    paymentAmount: input.paymentMinorAmount, // original payment amount (minor units)\n` +
    `    refundAmount: { minorAmount: input.refundMinorAmount, currency: toCurrency(input.currency) },\n` +
    `    reason: input.reason ?? "requested_by_customer",\n` +
    `    testMode: true,\n` +
    `  });\n` +
    `  // NOTE: refunds use RefundStatus (not PaymentStatus). Success = REFUND_SUCCESS (4).\n` +
    `  return { success: res.status === types.RefundStatus.REFUND_SUCCESS /* 4 */, status: res.status, raw: res };\n` +
    `}\n`
  );
}

function syncFn(ctx: ScaffoldCtx): string {
  const ts = ctx.language === "ts";
  const sig = ts ? "(input: SyncInput)" : "(input)";
  const inputType = ts
    ? `export interface SyncInput {\n  connectorTransactionId: string;\n  merchantTransactionId?: string;\n}\n\n`
    : "";
  return (
    inputType +
    `export async function syncPayment${sig} {\n` +
    `  const res = await paymentClient.get({\n` +
    `    connectorTransactionId: input.connectorTransactionId,\n` +
    `    merchantTransactionId: input.merchantTransactionId,\n` +
    `    testMode: true,\n` +
    `  });\n` +
    `  return { status: res.status, raw: res };\n` +
    `}\n`
  );
}

const FLOW_FN: Record<string, (ctx: ScaffoldCtx) => string> = {
  authorize: authorizeFn,
  capture: captureFn,
  void: voidFn,
  refund: refundFn,
  sync: syncFn,
};

function handlersModule(ctx: ScaffoldCtx): ScaffoldFile {
  const e = ext(ctx.language);
  const selected = ctx.flows.filter((f) => CODE_FLOWS.includes(f));
  const flows = selected.length ? selected : (["authorize"] as Flow[]);
  const helper =
    `// Map an ISO 4217 code string to the SDK's numeric Currency enum.\n` +
    `function toCurrency(code${ctx.language === "ts" ? ": string" : ""})${ctx.language === "ts" ? ": number" : ""} {\n` +
    `  const c = (types.Currency${ctx.language === "ts" ? " as unknown as Record<string, number>" : ""})[code.toUpperCase()];\n` +
    `  if (c === undefined) throw new Error(\`Unsupported currency: \${code}\`);\n` +
    `  return c;\n` +
    `}\n\n`;
  const body = flows.map((f) => FLOW_FN[f]!(ctx)).join("\n");
  const contents =
    `// Payment flow handlers for ${ctx.connector.displayName}.\n` +
    `import { paymentClient, types } from "./prismClient.js";\n\n` +
    helper +
    body;
  return { path: `src/payments/handlers.${e}`, contents };
}

// ---------------------------------------------------------------------------
// Framework wiring
// ---------------------------------------------------------------------------
function frameworkRoutes(ctx: ScaffoldCtx): ScaffoldFile {
  const e = ext(ctx.language);
  const flows = ctx.flows.filter((f) => CODE_FLOWS.includes(f));
  const has = (f: Flow) => flows.includes(f);

  if (ctx.framework === "express" || ctx.framework === "node") {
    const routes: string[] = [];
    if (has("authorize"))
      routes.push(
        `router.post("/payments/authorize", async (req, res) => {\n` +
          `  try { res.json(await authorizePayment(req.body)); }\n` +
          `  catch (err) { res.status(502).json({ error: String(err) }); }\n});`,
      );
    if (has("capture"))
      routes.push(
        `router.post("/payments/capture", async (req, res) => {\n  try { res.json(await capturePayment(req.body)); }\n  catch (err) { res.status(502).json({ error: String(err) }); }\n});`,
      );
    if (has("void"))
      routes.push(
        `router.post("/payments/void", async (req, res) => {\n  try { res.json(await voidPayment(req.body)); }\n  catch (err) { res.status(502).json({ error: String(err) }); }\n});`,
      );
    if (has("refund"))
      routes.push(
        `router.post("/payments/refund", async (req, res) => {\n  try { res.json(await refundPayment(req.body)); }\n  catch (err) { res.status(502).json({ error: String(err) }); }\n});`,
      );
    if (has("sync"))
      routes.push(
        `router.get("/payments/:id", async (req, res) => {\n  try { res.json(await syncPayment({ connectorTransactionId: req.params.id })); }\n  catch (err) { res.status(502).json({ error: String(err) }); }\n});`,
      );
    const imports = handlerImportList(flows);
    const contents =
      `import { Router } from "express";\n` +
      `import { ${imports} } from "./payments/handlers.js";\n\n` +
      `export const paymentsRouter = Router();\nconst router = paymentsRouter;\n\n` +
      routes.join("\n\n") +
      `\n\n// Mount in your app: app.use(express.json()); app.use(paymentsRouter);\n`;
    return { path: `src/routes.${e}`, contents };
  }

  if (ctx.framework === "fastify") {
    const imports = handlerImportList(flows);
    const regs: string[] = [];
    if (has("authorize")) regs.push(`  fastify.post("/payments/authorize", async (req) => authorizePayment(req.body));`);
    if (has("capture")) regs.push(`  fastify.post("/payments/capture", async (req) => capturePayment(req.body));`);
    if (has("void")) regs.push(`  fastify.post("/payments/void", async (req) => voidPayment(req.body));`);
    if (has("refund")) regs.push(`  fastify.post("/payments/refund", async (req) => refundPayment(req.body));`);
    if (has("sync"))
      regs.push(`  fastify.get("/payments/:id", async (req) => syncPayment({ connectorTransactionId: (req.params${ctx.language === "ts" ? " as { id: string }" : ""}).id }));`);
    const fastifyTypeImport = ctx.language === "ts" ? `import type { FastifyInstance } from "fastify";\n` : "";
    const contents =
      fastifyTypeImport +
      `import { ${imports} } from "./payments/handlers.js";\n\n` +
      `export async function paymentRoutes(fastify${ctx.language === "ts" ? ": FastifyInstance" : ""}) {\n` +
      regs.join("\n") +
      `\n}\n`;
    return { path: `src/routes.${e}`, contents };
  }

  if (ctx.framework === "nestjs") {
    const contents =
      `import { Controller, Post, Get, Body, Param } from "@nestjs/common";\n` +
      `import { ${handlerImportList(flows)} } from "./payments/handlers.js";\n\n` +
      `@Controller("payments")\nexport class PaymentsController {\n` +
      (has("authorize") ? `  @Post("authorize") authorize(@Body() body${ctx.language === "ts" ? ": any" : ""}) { return authorizePayment(body); }\n` : "") +
      (has("capture") ? `  @Post("capture") capture(@Body() body${ctx.language === "ts" ? ": any" : ""}) { return capturePayment(body); }\n` : "") +
      (has("void") ? `  @Post("void") void_(@Body() body${ctx.language === "ts" ? ": any" : ""}) { return voidPayment(body); }\n` : "") +
      (has("refund") ? `  @Post("refund") refund(@Body() body${ctx.language === "ts" ? ": any" : ""}) { return refundPayment(body); }\n` : "") +
      (has("sync") ? `  @Get(":id") sync(@Param("id") id${ctx.language === "ts" ? ": string" : ""}) { return syncPayment({ connectorTransactionId: id }); }\n` : "") +
      `}\n`;
    return { path: `src/payments.controller.${e}`, contents };
  }

  // next: route handlers per flow under app/api
  const handlers: string[] = [];
  const mk = (flow: string, fn: string, method: "POST" | "GET") =>
    `// app/api/payments/${flow}/route.${ext(ctx.language)}\n` +
    `import { ${fn} } from "@/payments/handlers";\n` +
    `import { NextResponse } from "next/server";\n\n` +
    `export async function ${method}(req${ctx.language === "ts" ? ": Request" : ""}) {\n` +
    `  const body = await req.json();\n` +
    `  return NextResponse.json(await ${fn}(body));\n}\n`;
  if (has("authorize")) handlers.push(mk("authorize", "authorizePayment", "POST"));
  if (has("capture")) handlers.push(mk("capture", "capturePayment", "POST"));
  if (has("void")) handlers.push(mk("void", "voidPayment", "POST"));
  if (has("refund")) handlers.push(mk("refund", "refundPayment", "POST"));
  return {
    path: `app/api/payments/route-handlers.${ext(ctx.language)}.txt`,
    contents:
      `// Next.js: place each block at its own app/api/payments/<flow>/route.${ext(ctx.language)} file.\n\n` +
      handlers.join("\n// ---\n\n"),
  };
}

function handlerImportList(flows: Flow[]): string {
  const names: string[] = [];
  if (flows.includes("authorize")) names.push("authorizePayment");
  if (flows.includes("capture")) names.push("capturePayment");
  if (flows.includes("void")) names.push("voidPayment");
  if (flows.includes("refund")) names.push("refundPayment");
  if (flows.includes("sync")) names.push("syncPayment");
  return names.length ? names.join(", ") : "authorizePayment";
}

// ---------------------------------------------------------------------------
// .env.example
// ---------------------------------------------------------------------------
function envExample(ctx: ScaffoldCtx): ScaffoldFile {
  const lines = requiredFields(ctx.connector).map(
    (f) => `${envVarName(ctx.connector.connector, f)}=${ctx.connector.fieldDocs[f.name]?.example ?? (f.secret ? "your_secret_here" : "value")}`,
  );
  return {
    path: ".env.example",
    contents: `# ${ctx.connector.displayName} sandbox credentials\n${lines.join("\n")}\n`,
  };
}

// ---------------------------------------------------------------------------
// Entry
// ---------------------------------------------------------------------------
export function scaffold(ctx: ScaffoldCtx): ScaffoldFile[] {
  return [
    clientModule(ctx),
    handlersModule(ctx),
    frameworkRoutes(ctx),
    envExample(ctx),
  ];
}
