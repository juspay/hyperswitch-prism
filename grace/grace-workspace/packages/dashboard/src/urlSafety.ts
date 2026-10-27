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
