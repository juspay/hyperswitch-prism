const imageMimeTypes = new Set([
  "image/png",
  "image/jpeg",
  "image/gif",
  "image/webp",
  "image/avif",
]);

/** Only absolute web URLs may be opened from pipeline or discovery data. */
export function safeHttpUrl(value: unknown): string | undefined {
  if (typeof value !== "string") return undefined;
  try {
    const url = new URL(value);
    return url.protocol === "https:" || url.protocol === "http:"
      ? url.href
      : undefined;
  } catch {
    return undefined;
  }
}

export function isSupportedImageMimeType(mimeType: string): boolean {
  return imageMimeTypes.has(mimeType);
}

export function isSupportedImageFile(file: File): boolean {
  return isSupportedImageMimeType(file.type);
}

/** FileReader previews must remain raster image data URLs. */
export function safeImageDataUrl(value: unknown): string | undefined {
  if (typeof value !== "string") return undefined;
  const comma = value.indexOf(",");
  if (comma < 0 || comma === value.length - 1) return undefined;
  const header = value.slice(0, comma);
  const mimeType = header.slice("data:".length, -";base64".length);
  return header === `data:${mimeType};base64` && isSupportedImageMimeType(mimeType)
    ? value
    : undefined;
}
