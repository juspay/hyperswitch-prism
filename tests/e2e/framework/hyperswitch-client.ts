import {
  request,
  test,
  type APIRequestContext,
  type APIResponse,
} from "@playwright/test";
import { configuration } from "./configuration.js";
import type { Json } from "./types.js";

export type CompletedApiCall = {
  method: string;
  path: string;
  since: string;
  requestId: string | undefined;
};

export function stringField(response: Json, field: string): string {
  const value = response[field];
  if (typeof value !== "string" || !value) {
    throw new Error(`Response must contain a nonempty ${field}`);
  }
  return value;
}

export class HyperswitchClient {
  private constructor(
    private readonly api: APIRequestContext,
    private readonly onResponse?: (call: CompletedApiCall) => Promise<void>,
  ) {}

  static async create(
    onResponse?: (call: CompletedApiCall) => Promise<void>,
  ): Promise<HyperswitchClient> {
    return new HyperswitchClient(
      await request.newContext({
        baseURL: configuration.hyperswitchBaseUrl,
        timeout: 60_000,
        extraHTTPHeaders: { Accept: "application/json" },
      }),
      onResponse,
    );
  }

  async call(
    method: "GET" | "POST",
    path: string,
    apiKey: string,
    data?: Json,
  ): Promise<Json> {
    return test.step(`${method} ${path.split("?")[0]}`, async () => {
      const since = new Date().toISOString();
      const response = await this.fetch(method, path, apiKey, data);
      await this.onResponse?.({
        method,
        path,
        since,
        requestId: response.headers()["x-request-id"],
      });

      if (!response.ok()) {
        const requestId = response.headers()["x-request-id"] ?? "unknown";
        // Response bodies may contain processor details, so failures expose only
        // the status and correlation identifier.
        throw new Error(
          `${method} ${path}: HTTP ${response.status()}, request-id=${requestId}`,
        );
      }
      let body: unknown;
      try {
        body = await response.json();
      } catch {
        throw new Error(`${method} ${path}: expected a JSON response`);
      }
      if (body === null || typeof body !== "object" || Array.isArray(body)) {
        throw new Error(`${method} ${path}: expected a JSON object`);
      }
      return body as Json;
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
        maxRetries: 0,
        maxRedirects: 0,
      });
    } catch {
      throw new Error(`${method} ${path}: Hyperswitch transport failed`);
    }
  }

  async dispose(): Promise<void> {
    await this.api.dispose();
  }
}
