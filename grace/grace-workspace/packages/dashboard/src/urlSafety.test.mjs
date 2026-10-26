import assert from "node:assert/strict";
import test from "node:test";
import {
  isSupportedImageMimeType,
  safeHttpUrl,
  safeImageDataUrl,
} from "./urlSafety.ts";

test("external links only accept absolute HTTP(S) URLs", () => {
  assert.equal(safeHttpUrl("https://github.com/juspay/hyperswitch-prism/pull/1"), "https://github.com/juspay/hyperswitch-prism/pull/1");
  assert.equal(safeHttpUrl("http://example.com/docs"), "http://example.com/docs");
  for (const value of ["javascript:alert(1)", "data:text/html,<script></script>", "//example.com", "/docs", "not a url", null]) {
    assert.equal(safeHttpUrl(value), undefined);
  }
});

test("attachment previews accept raster data URLs and reject active content", () => {
  assert.equal(isSupportedImageMimeType("image/png"), true);
  assert.equal(isSupportedImageMimeType("image/svg+xml"), false);
  assert.equal(safeImageDataUrl("data:image/png;base64,iVBORw0KGgo="), "data:image/png;base64,iVBORw0KGgo=");
  for (const value of ["data:image/svg+xml;base64,PHN2Zz4=", "data:text/html;base64,PHNjcmlwdD4=", "javascript:alert(1)", "data:image/png;base64,"]) {
    assert.equal(safeImageDataUrl(value), undefined);
  }
});
