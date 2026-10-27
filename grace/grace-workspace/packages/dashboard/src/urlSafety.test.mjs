import assert from "node:assert/strict";
import test from "node:test";
import {
  isSupportedImageMimeType,
  safeHttpUrl,
} from "./urlSafety.ts";

test("external links only accept absolute HTTP(S) URLs", () => {
  assert.equal(safeHttpUrl("https://github.com/juspay/hyperswitch-prism/pull/1"), "https://github.com/juspay/hyperswitch-prism/pull/1");
  assert.equal(safeHttpUrl("http://example.com/docs"), "http://example.com/docs");
  for (const value of ["javascript:alert(1)", "data:text/html,<script></script>", "//example.com", "/docs", "not a url", null]) {
    assert.equal(safeHttpUrl(value), undefined);
  }
});

test("attachments only accept raster image types", () => {
  assert.equal(isSupportedImageMimeType("image/png"), true);
  assert.equal(isSupportedImageMimeType("image/jpeg"), true);
  assert.equal(isSupportedImageMimeType("image/svg+xml"), false);
});
