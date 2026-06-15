import { readFile } from "node:fs/promises";
import { join } from "node:path";
import type { ParityConfig } from "../config.js";

export interface PayloadSpec {
  payload: unknown;
  service: string;
  rpc: string;
  protoFile: string;
  expected: Record<string, unknown>; // field path -> expected value
  source: "leaf-body" | "fixture";
  fixturePath?: string;
}

const PAYLOAD_BLOCK_RE = /```json\s*([\s\S]+?)```/i;
const EXPECTED_BLOCK_RE = /Expected oracle field[s]?[:\s]*```json\s*([\s\S]+?)```/i;
const RPC_LINE_RE = /(?:RPC|Service)\s*:\s*([A-Za-z_]+(?:\.[A-Za-z_]+)?)\s*\/\s*([A-Za-z_]+)/;
const PROTO_LINE_RE = /Proto\s*:\s*([a-z0-9_-]+\.proto)/i;

function parseFromBody(body: string): Partial<PayloadSpec> | null {
  const payloadMatch = body.match(PAYLOAD_BLOCK_RE);
  if (!payloadMatch) return null;
  let payload: unknown;
  try {
    payload = JSON.parse(payloadMatch[1]);
  } catch {
    return null;
  }
  const expectedMatch = body.match(EXPECTED_BLOCK_RE);
  let expected: Record<string, unknown> = {};
  if (expectedMatch) {
    try {
      expected = JSON.parse(expectedMatch[1]);
    } catch {
      // tolerate; expected can still come from issue text fallback
    }
  }
  const rpc = body.match(RPC_LINE_RE);
  const proto = body.match(PROTO_LINE_RE);
  if (!rpc || !proto) return null;
  return {
    payload,
    expected,
    service: rpc[1],
    rpc: rpc[2],
    protoFile: proto[1],
    source: "leaf-body",
  };
}

async function loadFixture(cfg: ParityConfig, connector: string, flow: string): Promise<PayloadSpec | null> {
  const fixturePath = join(cfg.prismPath, "crates/grpc-server/grpc-server/tests/fixtures", connector, `${flow}.json`);
  try {
    const raw = await readFile(fixturePath, "utf8");
    const obj = JSON.parse(raw) as Partial<PayloadSpec>;
    if (!obj.payload || !obj.service || !obj.rpc || !obj.protoFile) return null;
    return { ...obj, expected: obj.expected ?? {}, source: "fixture", fixturePath } as PayloadSpec;
  } catch {
    return null;
  }
}

export async function extractPayload(cfg: ParityConfig, leaf: { body: string; connector: string; flow: string }): Promise<PayloadSpec | null> {
  const fromBody = parseFromBody(leaf.body);
  if (fromBody?.payload && fromBody.service && fromBody.rpc && fromBody.protoFile) {
    return { ...(fromBody as PayloadSpec), expected: fromBody.expected ?? {}, source: "leaf-body" };
  }
  return await loadFixture(cfg, leaf.connector, leaf.flow);
}

export function getByPath(obj: unknown, path: string): unknown {
  const parts = path.split(".");
  let cur: any = obj;
  for (const p of parts) {
    if (cur == null) return undefined;
    const idx = p.match(/^\[(\d+)\]$/);
    if (idx) cur = cur[Number(idx[1])];
    else cur = cur[p];
  }
  return cur;
}

export function diffField(observed: unknown, expected: unknown, path: string): { path: string; match: boolean; expected: unknown; observed: unknown } {
  const o = getByPath(observed, path);
  return { path, match: JSON.stringify(o) === JSON.stringify(expected), expected, observed: o };
}
