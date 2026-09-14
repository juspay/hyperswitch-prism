import type { Locus, LocusTarget } from "./types.js";
import type { ParityConfig } from "./config.js";

export interface ClassifyOpts {
  declaredTarget?: "prism" | "hyperswitch-bridge";
  understoodLocus: Locus;
  bridgeAvailable: boolean;
}

export interface ClassifyResult {
  target: LocusTarget;
  reclassified: boolean;
  reason?: string;
}

export function classifyTarget(opts: ClassifyOpts): ClassifyResult {
  const { understoodLocus, bridgeAvailable, declaredTarget } = opts;

  if (understoodLocus === null) {
    return { target: "escalate", reclassified: false, reason: "no understanding summary parsed" };
  }
  if (understoodLocus === "hs-connector") {
    return { target: "escalate", reclassified: false, reason: "oracle disagreement: locus is in hyperswitch's own connector; humans must adjudicate" };
  }
  if (understoodLocus === "ambiguous") {
    return { target: "escalate", reclassified: false, reason: "ambiguous locus; awaiting human classification" };
  }

  let mapped: LocusTarget;
  if (understoodLocus === "prism-transformer") mapped = "prism";
  else if (understoodLocus === "hs-bridge") {
    if (!bridgeAvailable) {
      return { target: "escalate", reclassified: false, reason: "locus is hyperswitch bridge but bridgeWritePath is not configured (set PARITY_BRIDGE_PATH)" };
    }
    mapped = "hyperswitch-bridge";
  } else {
    return { target: "escalate", reclassified: false, reason: `unknown locus value: ${understoodLocus}` };
  }

  const reclassified =
    (declaredTarget === "prism" && mapped !== "prism") ||
    (declaredTarget === "hyperswitch-bridge" && mapped !== "hyperswitch-bridge");

  return { target: mapped, reclassified };
}

export interface DiffValidateOpts {
  changedFiles: string[];
  target: LocusTarget;
  rules: ParityConfig["rules"];
}

export function validateDiff(opts: DiffValidateOpts): { ok: boolean; violations: string[] } {
  const violations: string[] = [];
  const { changedFiles, target, rules } = opts;

  if (target === "prism") {
    for (const f of changedFiles) {
      const inConnIntegration = f.startsWith("crates/integrations/connector-integration/");
      const inTypesTraits = rules.forbiddenPrismDirs.some((d) => f.startsWith(d));
      if (!inConnIntegration) {
        violations.push(`prism diff touches non-connector-integration path: ${f}`);
      }
      if (inTypesTraits) {
        violations.push(`prism diff touches forbidden ${rules.forbiddenPrismDirs.join("/")}: ${f}`);
      }
    }
  } else if (target === "hyperswitch-bridge") {
    const bridgeFile = "crates/external_services/src/grpc_client/unified_connector_service.rs";
    const bridgeCrate = "crates/external_services/";
    for (const f of changedFiles) {
      for (const forbidden of rules.forbiddenOracleDirs) {
        if (f.startsWith(forbidden)) {
          violations.push(`hs-bridge diff touches forbidden oracle dir ${forbidden}: ${f}`);
        }
      }
      const isBridgeFile = f === bridgeFile;
      const isPrivateSupportingType = f.startsWith(bridgeCrate) && !f.startsWith(`${bridgeCrate}src/lib.rs`);
      if (!isBridgeFile && !isPrivateSupportingType) {
        violations.push(`hs-bridge diff touches path outside ${bridgeFile} and its crate: ${f}`);
      }
    }
  } else {
    violations.push(`target ${target} should not produce a diff`);
  }

  return { ok: violations.length === 0, violations };
}

const NON_SLUG_RE = /[^a-z0-9]+/g;
export function slugifyTitle(title: string): string {
  return title.toLowerCase().replace(NON_SLUG_RE, "-").replace(/^-|-$/g, "").slice(0, 40);
}

export function branchName(target: LocusTarget, connector: string, flow: string, title: string): string {
  const slug = slugifyTitle(title) || "fix";
  if (target === "hyperswitch-bridge") return `parity/bridge/${connector}-${flow}-${slug}`;
  return `parity/${connector}/${flow}-${slug}`;
}
