import { mkdir, readFile, rename, stat, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";

export async function writeAtomic(path: string, body: string | object): Promise<void> {
  await mkdir(dirname(path), { recursive: true });
  const tmp = `${path}.tmp-${process.pid}-${Date.now()}`;
  const data = typeof body === "string" ? body : JSON.stringify(body, null, 2);
  await writeFile(tmp, data, "utf8");
  await rename(tmp, path);
}

export async function readIfFresh<T>(path: string, ttlMs: number): Promise<T | null> {
  try {
    const st = await stat(path);
    if (Date.now() - st.mtimeMs > ttlMs) return null;
    const raw = await readFile(path, "utf8");
    return JSON.parse(raw) as T;
  } catch {
    return null;
  }
}

export function dailyTreeCachePath(cacheDir: string): string {
  const d = new Date();
  const yyyy = d.getUTCFullYear();
  const mm = String(d.getUTCMonth() + 1).padStart(2, "0");
  const dd = String(d.getUTCDate()).padStart(2, "0");
  return join(cacheDir, `tree-${yyyy}-${mm}-${dd}.json`);
}
