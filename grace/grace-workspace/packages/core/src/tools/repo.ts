import fs from "node:fs";
import fsp from "node:fs/promises";
import path from "node:path";
import type { ToolDef, ToolHandler } from "../llm.js";

const SKIP_DIRS = new Set([
  "node_modules",
  ".git",
  "dist",
  "build",
  "lib",
  ".next",
  "coverage",
  "pipeline-artifacts",
  ".yarn",
  ".turbo",
  ".cache",
]);

const MAX_READ_BYTES = 60_000;
const MAX_GLOB_HITS = 100;
const MAX_GREP_HITS = 60;
const MAX_WALK_FILES = 8000;

/**
 * Clamp a user-supplied path to the project root. Returns an absolute path
 * that is guaranteed to live inside `root`, or throws.
 */
function clamp(root: string, input: string): string {
  if (typeof input !== "string" || !input.trim()) {
    throw new Error(`path must be a non-empty string, got: ${JSON.stringify(input)}`);
  }
  const resolved = path.isAbsolute(input)
    ? path.resolve(input)
    : path.resolve(root, input);
  const rel = path.relative(root, resolved);
  if (rel.startsWith("..") || path.isAbsolute(rel)) {
    throw new Error(
      `path escapes project root: ${input} (resolved to ${resolved})`
    );
  }
  return resolved;
}

function walk(rootDir: string): string[] {
  const out: string[] = [];
  const stack = [rootDir];
  while (stack.length && out.length < MAX_WALK_FILES) {
    const dir = stack.pop()!;
    let entries: fs.Dirent[];
    try {
      entries = fs.readdirSync(dir, { withFileTypes: true });
    } catch {
      continue;
    }
    for (const e of entries) {
      if (out.length >= MAX_WALK_FILES) break;
      if (e.isDirectory()) {
        if (SKIP_DIRS.has(e.name) || e.name.startsWith(".")) continue;
        stack.push(path.join(dir, e.name));
      } else if (e.isFile()) {
        out.push(path.join(dir, e.name));
      }
    }
  }
  return out;
}

/**
 * Convert a simple glob to a regex. Supports `*`, `**`, `?`, `[abc]`, `{a,b}`.
 * Intentionally minimal — meant for agents, not a spec-compliant glob.
 */
function globToRegex(glob: string): RegExp {
  let re = "^";
  let inBraces = false;
  for (let i = 0; i < glob.length; i++) {
    const c = glob[i]!;
    if (c === "*") {
      if (glob[i + 1] === "*") {
        re += ".*";
        i++;
        if (glob[i + 1] === "/") i++;
      } else {
        re += "[^/]*";
      }
    } else if (c === "?") {
      re += "[^/]";
    } else if (c === "{") {
      re += "(";
      inBraces = true;
    } else if (c === "}") {
      re += ")";
      inBraces = false;
    } else if (c === "," && inBraces) {
      re += "|";
    } else if ("+()^$.|\\".includes(c)) {
      re += "\\" + c;
    } else {
      re += c;
    }
  }
  re += "$";
  return new RegExp(re);
}

function relToRoot(root: string, abs: string): string {
  return path.relative(root, abs) || ".";
}

export function createRepoTools(projectRoot: string): {
  tools: ToolDef[];
  handlers: Record<string, ToolHandler>;
} {
  const root = path.resolve(projectRoot);

  // ─── read_file ──────────────────────────────────────────────────────────
  const readFile: ToolHandler = async (args) => {
    const p = String(args.path ?? "");
    const abs = clamp(root, p);
    const stat = await fsp.stat(abs).catch(() => null);
    if (!stat) return `ERROR: file not found: ${p}`;
    if (!stat.isFile()) return `ERROR: not a regular file: ${p}`;
    const raw = await fsp.readFile(abs, "utf-8");
    if (raw.length > MAX_READ_BYTES) {
      return (
        raw.slice(0, MAX_READ_BYTES) +
        `\n\n... [truncated from ${raw.length} to ${MAX_READ_BYTES} chars — use grep to find the part you need]`
      );
    }
    return raw;
  };

  // ─── list_dir ───────────────────────────────────────────────────────────
  const listDir: ToolHandler = async (args) => {
    const p = String(args.path ?? ".");
    const abs = clamp(root, p);
    const stat = await fsp.stat(abs).catch(() => null);
    if (!stat) return `ERROR: directory not found: ${p}`;
    if (!stat.isDirectory()) return `ERROR: not a directory: ${p}`;
    const entries = await fsp.readdir(abs, { withFileTypes: true });
    const lines = entries
      .filter((e) => !e.name.startsWith(".") && !SKIP_DIRS.has(e.name))
      .map((e) => (e.isDirectory() ? `${e.name}/` : e.name))
      .sort();
    return lines.join("\n") || "(empty)";
  };

  // ─── glob ───────────────────────────────────────────────────────────────
  const glob: ToolHandler = async (args) => {
    const pattern = String(args.pattern ?? "");
    if (!pattern) return `ERROR: pattern is required`;
    const re = globToRegex(pattern);
    const files = walk(root);
    const hits: string[] = [];
    for (const f of files) {
      if (hits.length >= MAX_GLOB_HITS) break;
      const rel = relToRoot(root, f);
      if (re.test(rel)) hits.push(rel);
    }
    if (hits.length === 0) return `(no matches for ${pattern})`;
    return hits.join("\n");
  };

  // ─── grep ───────────────────────────────────────────────────────────────
  const grep: ToolHandler = async (args) => {
    const patternStr = String(args.pattern ?? "");
    const pathArg = args.path ? String(args.path) : undefined;
    if (!patternStr) return `ERROR: pattern is required`;
    let re: RegExp;
    try {
      re = new RegExp(patternStr, "i");
    } catch (err) {
      return `ERROR: invalid regex: ${err instanceof Error ? err.message : String(err)}`;
    }
    let files: string[];
    if (pathArg) {
      const abs = clamp(root, pathArg);
      const stat = await fsp.stat(abs).catch(() => null);
      if (!stat) return `ERROR: path not found: ${pathArg}`;
      files = stat.isDirectory() ? walk(abs) : [abs];
    } else {
      files = walk(root);
    }
    const out: string[] = [];
    for (const f of files) {
      if (out.length >= MAX_GREP_HITS) break;
      let content: string;
      try {
        content = await fsp.readFile(f, "utf-8");
      } catch {
        continue;
      }
      const lines = content.split("\n");
      for (let i = 0; i < lines.length; i++) {
        if (out.length >= MAX_GREP_HITS) break;
        const line = lines[i]!;
        if (re.test(line)) {
          out.push(`${relToRoot(root, f)}:${i + 1}: ${line.trim().slice(0, 240)}`);
        }
      }
    }
    if (out.length === 0) return `(no matches for /${patternStr}/)`;
    return out.join("\n");
  };

  // ─── Tool schema definitions (OpenAI tools format) ──────────────────────
  const tools: ToolDef[] = [
    {
      type: "function",
      function: {
        name: "read_file",
        description:
          "Read the contents of a text file inside the project repo. Path is relative to the project root. Returns up to 60,000 characters (truncated if larger).",
        parameters: {
          type: "object",
          properties: {
            path: {
              type: "string",
              description:
                "Path relative to the project root, e.g. src/screens/Settings/Foo.res",
            },
          },
          required: ["path"],
        },
      },
    },
    {
      type: "function",
      function: {
        name: "list_dir",
        description:
          "List immediate children of a directory inside the project repo. Hidden and build directories are filtered. Returns file and directory names, one per line (directories end with '/').",
        parameters: {
          type: "object",
          properties: {
            path: {
              type: "string",
              description:
                "Directory path relative to the project root (default: '.' for the root).",
            },
          },
          required: ["path"],
        },
      },
    },
    {
      type: "function",
      function: {
        name: "glob",
        description:
          "Find files in the project repo whose path matches a glob pattern. Supports *, **, ?, {a,b}. Returns up to 100 matching paths.",
        parameters: {
          type: "object",
          properties: {
            pattern: {
              type: "string",
              description:
                "Glob pattern, e.g. 'src/**/*.res' or 'src/screens/**/BusinessProfile*'",
            },
          },
          required: ["pattern"],
        },
      },
    },
    {
      type: "function",
      function: {
        name: "grep",
        description:
          "Case-insensitive regex search across files in the project repo. Returns up to 60 matching lines shaped as '<path>:<line>: <text>'. Optional `path` scopes the search to a file or subdirectory.",
        parameters: {
          type: "object",
          properties: {
            pattern: {
              type: "string",
              description: "JavaScript regex pattern (case-insensitive)",
            },
            path: {
              type: "string",
              description:
                "Optional file or directory to limit the search to. Default: whole repo.",
            },
          },
          required: ["pattern"],
        },
      },
    },
  ];

  const handlers: Record<string, ToolHandler> = {
    read_file: readFile,
    list_dir: listDir,
    glob,
    grep,
  };

  return { tools, handlers };
}
