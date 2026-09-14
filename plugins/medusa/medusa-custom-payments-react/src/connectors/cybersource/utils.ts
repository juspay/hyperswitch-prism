/**
 * Dynamically load the Cybersource Flex Microform (v2) SDK.
 *
 * The capture-context JWT returned by the backend embeds the exact client
 * library URL and its Subresource Integrity (SRI) hash. We prefer values the
 * backend surfaced explicitly, and fall back to decoding the JWT payload.
 */

let scriptLoadPromise: Promise<any> | null = null;

declare global {
  interface Window {
    Flex?: any;
  }
}

/** Decode the `ctx` block of the capture-context JWT (payload, base64url). */
export function decodeCaptureContext(
  captureContext: string
): { clientLibrary?: string; clientLibraryIntegrity?: string } {
  try {
    const payload = captureContext.split(".")[1];
    if (!payload) return {};
    const json = JSON.parse(
      decodeURIComponent(
        atob(payload.replace(/-/g, "+").replace(/_/g, "/"))
          .split("")
          .map((c) => "%" + c.charCodeAt(0).toString(16).padStart(2, "0"))
          .join("")
      )
    );
    const ctx = json?.ctx?.[0]?.data ?? {};
    return {
      clientLibrary: ctx.clientLibrary,
      clientLibraryIntegrity: ctx.clientLibraryIntegrity,
    };
  } catch {
    return {};
  }
}

/**
 * Inject the Flex Microform script. Resolves with `window.Flex`.
 *
 * @param clientLibrary    library URL (from session data or decoded JWT)
 * @param clientLibraryIntegrity  SRI hash applied to the script tag when present
 */
export function loadCybersourceFlexScript(
  clientLibrary: string,
  clientLibraryIntegrity?: string
): Promise<any> {
  if (typeof window === "undefined") {
    return Promise.reject(
      new Error("Cybersource Flex SDK can only be loaded in the browser")
    );
  }

  if (!clientLibrary) {
    return Promise.reject(
      new Error("Cybersource: missing clientLibrary URL for Flex Microform")
    );
  }

  if (window.Flex) {
    return Promise.resolve(window.Flex);
  }

  if (scriptLoadPromise) {
    return scriptLoadPromise;
  }

  scriptLoadPromise = new Promise((resolve, reject) => {
    const existing = document.querySelector(`script[src="${clientLibrary}"]`);
    if (existing) {
      existing.addEventListener("load", () =>
        window.Flex
          ? resolve(window.Flex)
          : reject(new Error("Flex SDK loaded but window.Flex is undefined"))
      );
      existing.addEventListener("error", () =>
        reject(new Error("Failed to load Cybersource Flex SDK"))
      );
      return;
    }

    const script = document.createElement("script");
    script.src = clientLibrary;
    script.async = true;
    if (clientLibraryIntegrity) {
      script.integrity = clientLibraryIntegrity;
      script.crossOrigin = "anonymous";
    }

    script.onload = () =>
      window.Flex
        ? resolve(window.Flex)
        : reject(new Error("Flex SDK loaded but window.Flex is undefined"));
    script.onerror = () =>
      reject(new Error("Failed to load Cybersource Flex SDK from CDN"));

    document.head.appendChild(script);
  });

  return scriptLoadPromise;
}
