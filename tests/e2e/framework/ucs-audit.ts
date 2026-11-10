import { execFile } from "node:child_process";
import { promisify } from "node:util";
import type { TestInfo } from "@playwright/test";
import type { CompletedApiCall } from "./hyperswitch-client.js";

const execFileAsync = promisify(execFile);
type AuditEvent = Record<string, unknown> & {
  request_id: string;
  flow_type: string;
  stage: "GrpcRequest" | "ConnectorCall";
  timestamp: number;
};

// Parse only the processed, masked event emitted by UCS's event publisher.
// The surrounding log record is never attached.
export function matchingEvents(lines: string, requestId: string): AuditEvent[] {
  const events: AuditEvent[] = [];
  for (const line of lines.split("\n")) {
    try {
      const record = JSON.parse(line);
      let payload = record?.ucs_event;
      if (typeof payload !== "string") {
        if (
          record?.target !== "ucs_common_utils::events" ||
          typeof record.events_enabled !== "boolean" ||
          typeof record.message !== "string"
        ) continue;
        const marker = `Event processed (Kafka publishing: ${record.events_enabled ? "enabled" : "disabled"}) - Event JSON: `;
        const start = record.message.indexOf(marker);
        if (start < 0) continue;
        payload = record.message.slice(start + marker.length);
      }
      const event = JSON.parse(payload);
      if (
        event?.request_id === requestId &&
        (event.stage === "GrpcRequest" || event.stage === "ConnectorCall") &&
        typeof event.flow_type === "string" &&
        typeof event.timestamp === "number" &&
        Number.isFinite(event.timestamp)
      ) {
        events.push(event);
      }
    } catch {
      // Unrelated logs and malformed records are not audit evidence.
    }
  }
  return events.sort((a, b) => a.timestamp - b.timestamp);
}

export class UcsAudit {
  private readonly pending: {
    label: string;
    events: AuditEvent[];
    reason?: string;
  }[] = [];

  constructor(private readonly info: TestInfo) {}

  async record(call: CompletedApiCall): Promise<void> {
    const route = call.path.split("?")[0];
    if (!["payments", "refunds"].includes(route.split("/")[1])) return;
    const label = `${call.method} ${route}`;
    let reason = "HS response has no request ID";
    if (call.requestId) {
      try {
        const container = process.env.UCS_LOG_CONTAINER ?? (await this.container());
        const { stdout, stderr } = await execFileAsync(
          "docker", ["logs", "--since", call.since, "--", container],
          { encoding: "utf8", timeout: 5_000, maxBuffer: 16 * 1024 * 1024 },
        );
        const events = matchingEvents(`${stdout}\n${stderr}`, call.requestId);
        if (events.length) {
          this.pending.push({ label, events });
          return;
        }
        reason = "No matching masked UCS events were found for this API request";
      } catch {
        // Child-process errors can contain raw logs; never forward them.
        reason = "UCS audit collection could not complete within its Docker log limits";
      }
    }
    this.pending.push({ label, events: [], reason });
  }

  async attachPending(): Promise<void> {
    for (const { label, events, reason } of this.pending.splice(0)) {
      for (const event of events) {
        await this.info.attach(`${label} / ${event.flow_type} / ${event.stage}`, {
          body: JSON.stringify(event, null, 2),
          contentType: "application/json",
        });
      }
      if (reason) {
        this.info.annotations.push({ type: `UCS audit: ${label}`, description: reason });
      }
    }
  }

  private async container(): Promise<string> {
    const { stdout } = await execFileAsync("docker", [
      "ps", "--filter", "label=com.docker.compose.project=ucs-e2e",
      "--filter", "label=com.docker.compose.service=ucs", "--format", "{{.ID}}",
    ], { encoding: "utf8", timeout: 5_000, maxBuffer: 64 * 1024 });
    const containers = stdout.trim().split("\n").filter(Boolean);
    if (containers.length !== 1) throw new Error("Expected one UCS container");
    return containers[0];
  }
}
