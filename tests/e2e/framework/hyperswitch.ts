import { request, test, type APIRequestContext } from "@playwright/test";
import pg from "pg";
import { configuration } from "./configuration.js";

export type Json = Record<string, any>;

function stepName(method: string, path: string): string {
  const route = path.split("?")[0];
  if (method === "POST" && route === "/payments") return "Create payment";
  if (method === "POST" && route.endsWith("/confirm"))
    return "Confirm payment";
  if (method === "GET" && route.startsWith("/payments/"))
    return "Retrieve payment";
  return `${method} ${route}`;
}

export class Hyperswitch {
  private constructor(private readonly api: APIRequestContext) {}

  static async create(): Promise<Hyperswitch> {
    return new Hyperswitch(
      await request.newContext({
        baseURL: configuration.hyperswitchBaseUrl,
        timeout: 60_000,
      }),
    );
  }

  async call(
    method: "GET" | "POST",
    path: string,
    apiKey: string,
    data?: Json,
  ): Promise<Json> {
    return test.step(stepName(method, path), async () => {
      let response;
      try {
        response = await this.api.fetch(path, {
          method,
          headers: { "api-key": apiKey },
          data,
        });
      } catch {
        throw new Error(`${method} ${path}: Hyperswitch transport failed`);
      }

      if (!response.ok()) {
        const requestId = response.headers()["x-request-id"] ?? "unknown";
        // Response bodies may contain processor details, so failures expose only
        // the status and correlation identifier.
        throw new Error(
          `${method} ${path}: HTTP ${response.status()}, request-id=${requestId}`,
        );
      }
      return response.json();
    });
  }

  async enableUcs(): Promise<void> {
    const response = await this.api.get("/configs/ucs_enabled", {
      headers: { "api-key": configuration.adminApiKey },
    });
    if (response.ok()) {
      const config = await response.json();
      if (config.value !== "true") {
        throw new Error("The local Hyperswitch stack has UCS disabled");
      }
      return;
    }
    if (response.status() !== 404) {
      throw new Error(
        `Cannot read Hyperswitch UCS configuration: HTTP ${response.status()}`,
      );
    }
    await this.call("POST", "/configs/", configuration.adminApiKey, {
      key: "ucs_enabled",
      value: "true",
    });
  }

  async dispose(): Promise<void> {
    await this.api.dispose();
  }
}

export async function readPaymentRoute(
  merchantId: string,
  paymentId: string,
  connector: string,
): Promise<{ gateway: string; processorReferenceRecorded: boolean }> {
  const client = new pg.Client({
    connectionString: configuration.databaseUrl,
    connectionTimeoutMillis: 10_000,
  });
  await client.connect();
  try {
    const result = await client.query(
      `SELECT p.feature_metadata->>'gateway_system' AS gateway,
        EXISTS (
          SELECT 1 FROM payment_attempt a
          WHERE a.merchant_id = p.merchant_id
            AND a.payment_id = p.payment_id
            AND a.connector = $3
            AND a.status::text = 'charged'
            AND a.connector_transaction_id IS NOT NULL
            AND a.connector_transaction_id <> ''
        ) AS processor_reference_recorded
       FROM payment_intent p
       WHERE p.merchant_id = $1 AND p.payment_id = $2`,
      [merchantId, paymentId, connector],
    );
    return {
      gateway: result.rows[0]?.gateway,
      processorReferenceRecorded:
        result.rows[0]?.processor_reference_recorded === true,
    };
  } finally {
    await client.end();
  }
}
