// Copies committed JSON data files into dist/ after tsc, since tsc only emits .js/.d.ts.
import { mkdirSync, copyFileSync, readdirSync, existsSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = dirname(dirname(fileURLToPath(import.meta.url)));
const srcDir = join(root, "src", "data");
const outDir = join(root, "dist", "data");

mkdirSync(outDir, { recursive: true });

if (existsSync(srcDir)) {
  for (const file of readdirSync(srcDir)) {
    if (file.endsWith(".json")) {
      copyFileSync(join(srcDir, file), join(outDir, file));
      console.log(`copied data/${file} -> dist/data/${file}`);
    }
  }
}
