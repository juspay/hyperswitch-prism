import { describe, it } from "node:test";
import assert from "node:assert/strict";

import {
  resolveProxyUrl,
  generateProxyCacheKey,
  NetworkError,
  NetworkErrorCode,
} from "../src/http_client";

/** Shared test URL to avoid repetition across test cases. */
const STRIPE_CHARGES_URL = "https://api.stripe.com/v1/charges";

describe("resolveProxyUrl", () => {
  it("returns null when proxy is undefined", () => {
    assert.strictEqual(
      resolveProxyUrl(STRIPE_CHARGES_URL, undefined),
      null
    );
  });

  it("returns null when proxy is null", () => {
    assert.strictEqual(
      resolveProxyUrl(STRIPE_CHARGES_URL, null),
      null
    );
  });

  it("returns null for empty proxy config", () => {
    assert.strictEqual(
      resolveProxyUrl(STRIPE_CHARGES_URL, {}),
      null
    );
  });

  it("prefers httpsUrl over httpUrl", () => {
    const proxy = {
      httpUrl: "http://proxy.local:8080",
      httpsUrl: "https://proxy.local:8443",
    };
    assert.strictEqual(
      resolveProxyUrl(STRIPE_CHARGES_URL, proxy),
      "https://proxy.local:8443"
    );
  });

  it("falls back to httpUrl when httpsUrl is absent", () => {
    const proxy = { httpUrl: "http://proxy.local:8080" };
    assert.strictEqual(
      resolveProxyUrl(STRIPE_CHARGES_URL, proxy),
      "http://proxy.local:8080"
    );
  });

  it("uses httpsUrl when httpUrl is absent", () => {
    const proxy = { httpsUrl: "https://proxy.local:8443" };
    assert.strictEqual(
      resolveProxyUrl(STRIPE_CHARGES_URL, proxy),
      "https://proxy.local:8443"
    );
  });

  it("bypasses proxy for URLs in the bypassUrls list", () => {
    const proxy = {
      httpsUrl: "https://proxy.local:8443",
      bypassUrls: [
        STRIPE_CHARGES_URL,
        "https://internal.service.local",
      ],
    };
    assert.strictEqual(
      resolveProxyUrl(STRIPE_CHARGES_URL, proxy),
      null
    );
  });

  it("applies proxy when URL is not in bypassUrls", () => {
    const proxy = {
      httpsUrl: "https://proxy.local:8443",
      bypassUrls: ["https://internal.service.local"],
    };
    assert.strictEqual(
      resolveProxyUrl(STRIPE_CHARGES_URL, proxy),
      "https://proxy.local:8443"
    );
  });

  it("applies proxy when bypassUrls is empty", () => {
    const proxy = {
      httpUrl: "http://proxy.local:8080",
      bypassUrls: [],
    };
    assert.strictEqual(
      resolveProxyUrl(STRIPE_CHARGES_URL, proxy),
      "http://proxy.local:8080"
    );
  });

  it("applies proxy when bypassUrls is undefined", () => {
    const proxy = { httpUrl: "http://proxy.local:8080" };
    assert.strictEqual(
      resolveProxyUrl(STRIPE_CHARGES_URL, proxy),
      "http://proxy.local:8080"
    );
  });
});

describe("generateProxyCacheKey", () => {
  it("returns empty string for undefined proxy", () => {
    assert.strictEqual(generateProxyCacheKey(undefined), "");
  });

  it("returns empty string for null proxy", () => {
    assert.strictEqual(generateProxyCacheKey(null), "");
  });

  it("generates key with httpUrl only", () => {
    const proxy = { httpUrl: "http://proxy.local:8080" };
    assert.strictEqual(
      generateProxyCacheKey(proxy),
      "http://proxy.local:8080||"
    );
  });

  it("generates key with httpsUrl only", () => {
    const proxy = { httpsUrl: "https://proxy.local:8443" };
    assert.strictEqual(
      generateProxyCacheKey(proxy),
      "|https://proxy.local:8443|"
    );
  });

  it("generates key with both URLs", () => {
    const proxy = {
      httpUrl: "http://proxy.local:8080",
      httpsUrl: "https://proxy.local:8443",
    };
    assert.strictEqual(
      generateProxyCacheKey(proxy),
      "http://proxy.local:8080|https://proxy.local:8443|"
    );
  });

  it("is deterministic regardless of bypassUrls order", () => {
    const base = { httpUrl: "http://proxy.local:8080" };
    const urls = [
      "https://z.example.com",
      "https://a.example.com",
      "https://m.example.com",
    ];

    const keyA = generateProxyCacheKey({ ...base, bypassUrls: [urls[0], urls[1], urls[2]] });
    const keyB = generateProxyCacheKey({ ...base, bypassUrls: [urls[1], urls[2], urls[0]] });
    const keyC = generateProxyCacheKey({ ...base, bypassUrls: [urls[2], urls[0], urls[1]] });

    assert.strictEqual(keyA, keyB);
    assert.strictEqual(keyB, keyC);
  });

  it("sorts bypassUrls in the key", () => {
    const proxy = {
      httpUrl: "http://proxy.local:8080",
      bypassUrls: ["https://z.com", "https://a.com"],
    };
    assert.strictEqual(
      generateProxyCacheKey(proxy),
      "http://proxy.local:8080||https://a.com,https://z.com"
    );
  });

  it("handles proxy with only bypassUrls", () => {
    const proxy = { bypassUrls: ["https://skip.me"] };
    assert.strictEqual(generateProxyCacheKey(proxy), "||https://skip.me");
  });
});

describe("NetworkError", () => {
  it("has name set to NetworkError", () => {
    const err = new NetworkError("test");
    assert.strictEqual(err.name, "NetworkError");
  });

  it("extends Error", () => {
    const err = new NetworkError("test");
    assert.ok(err instanceof Error);
    assert.ok(err instanceof NetworkError);
  });

  it("preserves the message", () => {
    const err = new NetworkError("connection refused");
    assert.strictEqual(err.message, "connection refused");
  });

  it("stores all constructor arguments", () => {
    const headers = { "content-type": "application/json" };
    const err = new NetworkError(
      "Bad Gateway",
      NetworkErrorCode.NETWORK_FAILURE,
      502,
      '{"error":"upstream_down"}',
      headers
    );
    assert.strictEqual(err.code, NetworkErrorCode.NETWORK_FAILURE);
    assert.strictEqual(err.statusCode, 502);
    assert.strictEqual(err.body, '{"error":"upstream_down"}');
    assert.deepStrictEqual(err.headers, headers);
  });

  it("defaults code to NETWORK_ERROR_CODE_UNSPECIFIED", () => {
    const err = new NetworkError("timeout");
    assert.strictEqual(err.code, NetworkErrorCode.NETWORK_ERROR_CODE_UNSPECIFIED);
  });

  it("leaves optional fields undefined when omitted", () => {
    const err = new NetworkError("timeout");
    assert.strictEqual(err.statusCode, undefined);
    assert.strictEqual(err.body, undefined);
    assert.strictEqual(err.headers, undefined);
  });

  it("maps NETWORK_FAILURE to its string name via errorCode", () => {
    const err = new NetworkError("fail", NetworkErrorCode.NETWORK_FAILURE);
    assert.strictEqual(err.errorCode, "NETWORK_FAILURE");
  });

  it("maps CONNECT_TIMEOUT_EXCEEDED via errorCode", () => {
    const err = new NetworkError("timeout", NetworkErrorCode.CONNECT_TIMEOUT_EXCEEDED);
    assert.strictEqual(err.errorCode, "CONNECT_TIMEOUT_EXCEEDED");
  });

  it("maps TOTAL_TIMEOUT_EXCEEDED via errorCode", () => {
    const err = new NetworkError("timeout", NetworkErrorCode.TOTAL_TIMEOUT_EXCEEDED);
    assert.strictEqual(err.errorCode, "TOTAL_TIMEOUT_EXCEEDED");
  });

  it("maps URL_PARSING_FAILED via errorCode", () => {
    const err = new NetworkError("bad url", NetworkErrorCode.URL_PARSING_FAILED);
    assert.strictEqual(err.errorCode, "URL_PARSING_FAILED");
  });

  it("maps INVALID_PROXY_CONFIGURATION via errorCode", () => {
    const err = new NetworkError("bad proxy", NetworkErrorCode.INVALID_PROXY_CONFIGURATION);
    assert.strictEqual(err.errorCode, "INVALID_PROXY_CONFIGURATION");
  });

  it("returns NETWORK_ERROR_CODE_UNSPECIFIED for unrecognised codes", () => {
    const err = new NetworkError("unknown", 99999 as unknown as NetworkErrorCode);
    assert.strictEqual(err.errorCode, "NETWORK_ERROR_CODE_UNSPECIFIED");
  });

  it("returns NETWORK_ERROR_CODE_UNSPECIFIED for default code", () => {
    const err = new NetworkError("default");
    assert.strictEqual(err.errorCode, "NETWORK_ERROR_CODE_UNSPECIFIED");
  });
});
