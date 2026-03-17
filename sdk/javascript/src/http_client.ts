import { ProxyAgent, Agent, Dispatcher } from "undici";
// @ts-ignore
import { types } from "./payments/generated/proto";

const Defaults = types.HttpDefault;

/**
 * Normalized HTTP Request structure for the Connector Service.
 */
export interface HttpRequest {
  url: string;
  method: string;
  headers?: Record<string, string>;
  body?: Uint8Array;
}

/**
 * Normalized HTTP Response structure.
 */
export interface HttpResponse {
  statusCode: number;
  headers: Record<string, string>;
  body: Uint8Array;
  latencyMs: number; // Flat field for cross-language parity
}

/** Network error codes from proto (single source of truth). */
export const NetworkErrorCode = types.NetworkErrorCode;

/**
 * Network error for HTTP transport failures (timeouts, connection errors, config).
 * Uses proto-generated NetworkErrorCode for cross-SDK parity with RequestError/ResponseError.
 */
export class NetworkError extends Error {
  constructor(
    message: string,
    public code: types.NetworkErrorCode = types.NetworkErrorCode.NETWORK_ERROR_CODE_UNSPECIFIED,
    public statusCode?: number,
    public body?: string,
    public headers?: Record<string, string>
  ) {
    super(message);
    this.name = "NetworkError";
  }

  /**
   * String error code for parity with RequestError/ResponseError (e.g. "CONNECT_TIMEOUT").
   * Use for logging, display, and simple comparisons.
   */
  get errorCode(): string {
    return types.NetworkErrorCode[this.code as number] ?? "NETWORK_ERROR_CODE_UNSPECIFIED";
  }
}

/**
 * Resolve proxy URL, honoring bypass rules.
 */
export function resolveProxyUrl(url: string, proxy?: types.IProxyOptions | null): string | null {
  if (!proxy) return null;
  const shouldBypass = Array.isArray(proxy.bypassUrls) && proxy.bypassUrls.includes(url);
  if (shouldBypass) return null;
  return proxy.httpsUrl || proxy.httpUrl || null;
}

/**
 * Creates a high-performance dispatcher with specialized fintech timeouts.
 * (The instance-level connection pool)
 */
export function createDispatcher(config: types.IHttpConfig): Dispatcher {
  let ca: string | Uint8Array | undefined;
  if (config.caCert) {
    if (config.caCert.pem) {
      ca = config.caCert.pem;
    } else if (config.caCert.der) {
      ca = config.caCert.der;
    }
  }

  const dispatcherOptions: any = {
    connect: {
      timeout: config.connectTimeoutMs ?? Defaults.CONNECT_TIMEOUT_MS,
      ca,
    },
    headersTimeout: config.responseTimeoutMs ?? Defaults.RESPONSE_TIMEOUT_MS,
    bodyTimeout: config.responseTimeoutMs ?? Defaults.RESPONSE_TIMEOUT_MS,
    keepAliveTimeout: config.keepAliveTimeoutMs ?? Defaults.KEEP_ALIVE_TIMEOUT_MS,
  };

  const proxyUrl = config.proxy?.httpsUrl || config.proxy?.httpUrl;
  try {
    return proxyUrl
      ? new ProxyAgent({ uri: proxyUrl, ...dispatcherOptions })
      : new Agent(dispatcherOptions);
  } catch (error: any) {
    const code = proxyUrl ? types.NetworkErrorCode.INVALID_PROXY_CONFIGURATION : types.NetworkErrorCode.CLIENT_INITIALIZATION_FAILURE;
    throw new NetworkError(`Internal HTTP setup failed: ${error.message}`, code, 500);
  }
}

/**
 * Standardized network execution engine for Unified Connector Service.
 */
export async function execute(
  request: HttpRequest,
  options: types.IHttpConfig = {},
  dispatcher?: Dispatcher // Pass the instance-owned pool here
): Promise<HttpResponse> {
  const { url, method, headers, body } = request;

  try {
    new URL(url);
  } catch {
    throw new NetworkError(`Invalid URL: ${url}`, types.NetworkErrorCode.URL_PARSING_FAILED);
  }

  const totalTimeout = options.totalTimeoutMs ?? Defaults.TOTAL_TIMEOUT_MS;
  const controller = new AbortController();
  const timeoutId = setTimeout(() => controller.abort(), totalTimeout);

  const startTime = Date.now();

  try {
    const response = await fetch(url, {
      method: method.toUpperCase(),
      headers: headers || {},
      body: body ? Buffer.from(body) : undefined,
      redirect: "manual",
      signal: controller.signal,
      // @ts-ignore
      dispatcher,
    });

    const responseHeaders: Record<string, string> = {};
    response.headers.forEach((v, k) => { responseHeaders[k.toLowerCase()] = v; });

    let responseBody: Uint8Array;
    try {
      responseBody = new Uint8Array(await response.arrayBuffer());
    } catch (e: any) {
      throw new NetworkError(`Failed to read response body: ${e?.message || e}`, types.NetworkErrorCode.RESPONSE_DECODING_FAILED, response.status);
    }

    return {
      statusCode: response.status,
      headers: responseHeaders,
      body: responseBody,
      latencyMs: Date.now() - startTime
    };
  } catch (error: any) {
    if (error instanceof NetworkError) throw error;
    if (error.name === 'AbortError') {
      throw new NetworkError(
        `Total Request Timeout: ${method} ${url} exceeded ${totalTimeout}ms`,
        types.NetworkErrorCode.TOTAL_TIMEOUT_EXCEEDED,
        504
      );
    }

    const cause = error.cause;
    if (cause) {
      if (cause.code === 'UND_ERR_CONNECT_TIMEOUT') {
        throw new NetworkError(
          `Connection Timeout: Failed to connect to ${url}`,
          types.NetworkErrorCode.CONNECT_TIMEOUT_EXCEEDED,
          504
        );
      }
      if (cause.code === 'UND_ERR_BODY_TIMEOUT' || cause.code === 'UND_ERR_HEADERS_TIMEOUT') {
        throw new NetworkError(
          `Response Timeout: Gateway ${url} accepted connection but failed to respond`,
          types.NetworkErrorCode.RESPONSE_TIMEOUT_EXCEEDED,
          504
        );
      }
    }

    throw new NetworkError(`Network Error: ${error.message}`, types.NetworkErrorCode.NETWORK_FAILURE, 500);
  } finally {
    clearTimeout(timeoutId);
  }
}
