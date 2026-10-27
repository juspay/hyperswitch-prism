import {
  request,
  test,
  type APIRequestContext,
  type APIResponse,
} from "@playwright/test";
import { configuration } from "./configuration.js";

export type Json = Record<string, any>;

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
    return test.step(`${method} ${path.split("?")[0]}`, async () => {
      const response = await this.fetch(method, path, apiKey, data);

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

  // Exposes HTTP status for setup APIs that can legitimately return 404.
  async fetch(
    method: "GET" | "POST",
    path: string,
    apiKey: string,
    data?: Json,
  ): Promise<APIResponse> {
    try {
      return await this.api.fetch(path, {
        method,
        headers: { "api-key": apiKey },
        data,
      });
    } catch {
      throw new Error(`${method} ${path}: Hyperswitch transport failed`);
    }
  }

  async dispose(): Promise<void> {
    await this.api.dispose();
  }
}
