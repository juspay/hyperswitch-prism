import fs from "node:fs";
import path from "node:path";
import YAML from "yaml";
import Mustache from "mustache";
import { getConfig } from "../config.js";

export interface PromptFrontmatter {
  name: string;
  description?: string;
  variables?: string[];
}

export interface LoadedPrompt {
  name: string;
  frontmatter: PromptFrontmatter;
  body: string;
  path: string;
}

// Variable values may contain Rust paths, cargo errors, diff hunks — none of
// which should be HTML-escaped on the way to Claude.
Mustache.escape = (s: string) => s;

export function loadPrompt(name: string, promptsDir?: string): LoadedPrompt {
  const dir = promptsDir ?? getConfig().prResolver.promptsDir;
  if (!fs.existsSync(dir)) {
    throw new Error(
      `PR Resolver prompts directory does not exist: ${dir} — set prResolver.promptsDir or copy prompts from grace/pr-resolver/prompts/`
    );
  }
  const filePath = path.join(dir, `${name}.md`);
  if (!fs.existsSync(filePath)) {
    throw new Error(`Prompt not found: ${filePath}`);
  }
  const raw = fs.readFileSync(filePath, "utf-8");
  const { frontmatter, body } = parseFrontmatter(raw, filePath);
  return { name: frontmatter.name, frontmatter, body, path: filePath };
}

export function renderPrompt(
  name: string,
  vars: Record<string, unknown>,
  promptsDir?: string
): string {
  const prompt = loadPrompt(name, promptsDir);
  warnOnVarMismatch(prompt, vars);
  return Mustache.render(prompt.body, vars);
}

function warnOnVarMismatch(
  prompt: LoadedPrompt,
  vars: Record<string, unknown>
): void {
  const declared = prompt.frontmatter.variables ?? [];
  if (declared.length === 0) return;
  const declaredSet = new Set(declared);
  for (const v of declared) {
    if (!(v in vars)) {
      // eslint-disable-next-line no-console
      console.warn(
        `[pr-resolver:prompts] ${prompt.name}: declared variable '${v}' not supplied — Mustache will render empty`
      );
    }
  }
  for (const v of Object.keys(vars)) {
    if (!declaredSet.has(v)) {
      // eslint-disable-next-line no-console
      console.warn(
        `[pr-resolver:prompts] ${prompt.name}: supplied variable '${v}' is not declared in frontmatter — template may ignore it`
      );
    }
  }
}

function parseFrontmatter(
  raw: string,
  filePath: string
): { frontmatter: PromptFrontmatter; body: string } {
  const fenceMatch = raw.match(/^---\s*\n([\s\S]+?)\n---\s*\n?([\s\S]*)$/);
  if (!fenceMatch) {
    throw new Error(
      `Prompt ${filePath} has no YAML frontmatter fenced by '---' lines.`
    );
  }
  const yamlBody = fenceMatch[1] as string;
  const body = fenceMatch[2] as string;
  let parsed: PromptFrontmatter;
  try {
    parsed = YAML.parse(yamlBody) as PromptFrontmatter;
  } catch (err) {
    throw new Error(
      `Prompt ${filePath}: failed to parse YAML frontmatter: ${err instanceof Error ? err.message : String(err)}`
    );
  }
  if (!parsed || typeof parsed.name !== "string" || !parsed.name) {
    throw new Error(`Prompt ${filePath}: frontmatter must include 'name'`);
  }
  return { frontmatter: parsed, body };
}
