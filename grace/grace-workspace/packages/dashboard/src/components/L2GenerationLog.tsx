import React from "react";
import { T } from "../theme";

// Type definitions matching the core types
interface WebSearchResult {
  title: string;
  url: string;
  snippet?: string;
}

interface WebSearchQuery {
  query: string;
  timestamp: string;
  results: WebSearchResult[];
  resultCount: number;
}

interface WorkflowExecutionLog {
  phase: "links_discovery" | "techspec_generation";
  workflowFile: string;
  readAt: string;
  output: string;
  status: "success" | "failed";
}

interface CommandExecution {
  command: string;
  workingDir: string;
  output?: string;
  durationMs?: number;
  status: "success" | "failed";
}

interface FileCreated {
  path: string;
  description: string;
  sizeBytes?: number;
}

interface L2GenerationLog {
  workflowExecutions: WorkflowExecutionLog[];
  webSearchQueries: WebSearchQuery[];
  filesCreated: FileCreated[];
  commandsExecuted: CommandExecution[];
}

// Card container component
function SectionCard({ children, title, icon }: { children: React.ReactNode; title: string; icon: string }) {
  return (
    <div
      style={{
        background: T.bgElev,
        border: `1px solid ${T.border}`,
        borderRadius: 12,
        marginBottom: 16,
        boxShadow: T.shadow,
        overflow: "hidden",
      }}
    >
      <div
        style={{
          background: T.bgSidebar,
          borderBottom: `1px solid ${T.border}`,
          padding: "14px 18px",
          display: "flex",
          alignItems: "center",
          gap: 10,
        }}
      >
        <span style={{ fontSize: 18 }}>{icon}</span>
        <span
          style={{
            fontSize: 13,
            fontWeight: 700,
            color: T.text,
            letterSpacing: 0.3,
          }}
        >
          {title}
        </span>
      </div>
      <div style={{ padding: 18 }}>{children}</div>
    </div>
  );
}

// Status badge component
function StatusBadge({ status }: { status: "success" | "failed" }) {
  const isSuccess = status === "success";
  return (
    <span
      style={{
        display: "inline-flex",
        alignItems: "center",
        gap: 4,
        padding: "3px 10px",
        borderRadius: 999,
        background: isSuccess ? T.successSoft : T.errorSoft,
        color: isSuccess ? T.success : T.error,
        fontSize: 11,
        fontWeight: 600,
        textTransform: "uppercase",
        letterSpacing: 0.5,
      }}
    >
      <span>{isSuccess ? "✓" : "✗"}</span>
      {status}
    </span>
  );
}

// Workflow execution item
function WorkflowItem({ wf }: { wf: WorkflowExecutionLog }) {
  return (
    <div
      style={{
        display: "flex",
        alignItems: "flex-start",
        gap: 12,
        padding: "12px 0",
        borderBottom: `1px solid ${T.border}`,
      }}
    >
      <StatusBadge status={wf.status} />
      <div style={{ flex: 1 }}>
        <div
          style={{
            fontSize: 12,
            fontWeight: 600,
            color: T.text,
            fontFamily: "ui-monospace, SFMono-Regular, monospace",
            marginBottom: 4,
          }}
        >
          {wf.workflowFile}
        </div>
        <div style={{ fontSize: 11, color: T.textMuted, marginBottom: 4 }}>
          {new Date(wf.readAt).toLocaleString()}
        </div>
        <div style={{ fontSize: 12, color: T.text, lineHeight: 1.5 }}>{wf.output}</div>
      </div>
    </div>
  );
}

// Web search query item
function SearchQueryItem({ query }: { query: WebSearchQuery }) {
  return (
    <div
      style={{
        marginBottom: 16,
        padding: 14,
        background: T.bg,
        border: `1px solid ${T.border}`,
        borderRadius: 8,
      }}
    >
      <div
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          marginBottom: 10,
        }}
      >
        <code
          style={{
            fontSize: 12,
            color: T.accent,
            background: T.accentSoft,
            padding: "4px 10px",
            borderRadius: 6,
            fontFamily: "ui-monospace, SFMono-Regular, monospace",
          }}
        >
          {query.query}
        </code>
        <span
          style={{
            fontSize: 11,
            color: T.textMuted,
            fontWeight: 600,
          }}
        >
          {query.resultCount} results
        </span>
      </div>

      {query.results.length > 0 && (
        <ul
          style={{
            margin: 0,
            padding: 0,
            listStyle: "none",
          }}
        >
          {query.results.slice(0, 5).map((result, idx) => (
            <li
              key={idx}
              style={{
                padding: "8px 0",
                borderBottom: idx < query.results.length - 1 ? `1px dashed ${T.border}` : "none",
              }}
            >
              <a
                href={result.url}
                target="_blank"
                rel="noopener noreferrer"
                style={{
                  fontSize: 12,
                  color: T.accent,
                  textDecoration: "none",
                  fontWeight: 600,
                  display: "block",
                  marginBottom: 2,
                }}
              >
                {result.title || result.url}
              </a>
              {result.snippet && (
                <div
                  style={{
                    fontSize: 11,
                    color: T.textMuted,
                    lineHeight: 1.4,
                  }}
                >
                  {result.snippet.length > 120
                    ? result.snippet.slice(0, 120) + "..."
                    : result.snippet}
                </div>
              )}
            </li>
          ))}
          {query.results.length > 5 && (
            <li style={{ fontSize: 11, color: T.textSubtle, paddingTop: 6, fontStyle: "italic" }}>
              + {query.results.length - 5} more results
            </li>
          )}
        </ul>
      )}
    </div>
  );
}

// Command execution item
function CommandItem({ cmd }: { cmd: CommandExecution }) {
  const duration = cmd.durationMs ? `${(cmd.durationMs / 1000).toFixed(1)}s` : null;

  return (
    <div
      style={{
        marginBottom: 12,
        padding: 12,
        background: T.codeBg,
        border: `1px solid ${T.border}`,
        borderRadius: 8,
      }}
    >
      <pre
        style={{
          margin: 0,
          padding: 0,
          fontSize: 11,
          fontFamily: "ui-monospace, SFMono-Regular, monospace",
          color: T.text,
          whiteSpace: "pre-wrap",
          wordBreak: "break-word",
          marginBottom: 8,
        }}
      >
        <span style={{ color: T.textSubtle }}>$</span> {cmd.command}
      </pre>
      <div
        style={{
          display: "flex",
          alignItems: "center",
          gap: 16,
          fontSize: 11,
          color: T.textMuted,
        }}
      >
        <span>CWD: {cmd.workingDir}</span>
        {duration && <span>Duration: {duration}</span>}
        <StatusBadge status={cmd.status} />
      </div>
    </div>
  );
}

// File created item
function FileCreatedItem({ file }: { file: FileCreated }) {
  const sizeDisplay = file.sizeBytes
    ? file.sizeBytes < 1024
      ? `${file.sizeBytes}B`
      : `${(file.sizeBytes / 1024).toFixed(1)}KB`
    : null;

  return (
    <div
      style={{
        display: "flex",
        alignItems: "center",
        justifyContent: "space-between",
        padding: "10px 0",
        borderBottom: `1px dashed ${T.border}`,
      }}
    >
      <div>
        <div
          style={{
            fontSize: 12,
            fontWeight: 600,
            color: T.text,
            fontFamily: "ui-monospace, SFMono-Regular, monospace",
          }}
        >
          {file.path}
        </div>
        <div style={{ fontSize: 11, color: T.textMuted }}>{file.description}</div>
      </div>
      {sizeDisplay && (
        <span
          style={{
            fontSize: 11,
            color: T.textSubtle,
            background: T.codeBg,
            padding: "2px 8px",
            borderRadius: 4,
          }}
        >
          {sizeDisplay}
        </span>
      )}
    </div>
  );
}

// Main L2 Generation Log component
export function L2GenerationLog({ log }: { log: L2GenerationLog }) {
  if (!log) {
    return (
      <div style={{ padding: 20, color: T.textMuted, fontStyle: "italic" }}>
        No generation log available.
      </div>
    );
  }

  const hasWorkflows = log.workflowExecutions && log.workflowExecutions.length > 0;
  const hasQueries = log.webSearchQueries && log.webSearchQueries.length > 0;
  const hasCommands = log.commandsExecuted && log.commandsExecuted.length > 0;
  const hasFiles = log.filesCreated && log.filesCreated.length > 0;

  return (
    <div style={{ maxWidth: 900 }}>
      {/* Workflow Execution Section */}
      {hasWorkflows && (
        <SectionCard title="Workflow Execution" icon="📋">
          <div>
            {log.workflowExecutions.map((wf, idx) => (
              <WorkflowItem key={idx} wf={wf} />
            ))}
          </div>
        </SectionCard>
      )}

      {/* Web Search Findings Section */}
      {hasQueries && (
        <SectionCard title="Web Search Findings" icon="🔍">
          <div>
            {log.webSearchQueries.map((query, idx) => (
              <SearchQueryItem key={idx} query={query} />
            ))}
          </div>
        </SectionCard>
      )}

      {/* Commands Executed Section */}
      {(hasCommands || hasFiles) && (
        <SectionCard title="Commands & Files" icon="⚙️">
          {hasCommands && (
            <div style={{ marginBottom: hasFiles ? 20 : 0 }}>
              <div
                style={{
                  fontSize: 11,
                  fontWeight: 700,
                  textTransform: "uppercase",
                  letterSpacing: 0.5,
                  color: T.textMuted,
                  marginBottom: 10,
                }}
              >
                Commands Executed
              </div>
              {log.commandsExecuted.map((cmd, idx) => (
                <CommandItem key={idx} cmd={cmd} />
              ))}
            </div>
          )}

          {hasFiles && (
            <div>
              <div
                style={{
                  fontSize: 11,
                  fontWeight: 700,
                  textTransform: "uppercase",
                  letterSpacing: 0.5,
                  color: T.textMuted,
                  marginBottom: 10,
                }}
              >
                Files Created
              </div>
              {log.filesCreated.map((file, idx) => (
                <FileCreatedItem key={idx} file={file} />
              ))}
            </div>
          )}
        </SectionCard>
      )}

      {/* Empty state */}
      {!hasWorkflows && !hasQueries && !hasCommands && !hasFiles && (
        <div
          style={{
            padding: 40,
            textAlign: "center",
            color: T.textMuted,
            background: T.bgElev,
            border: `1px dashed ${T.border}`,
            borderRadius: 12,
          }}
        >
          <div style={{ fontSize: 32, marginBottom: 12 }}>📝</div>
          <div style={{ fontSize: 14 }}>No detailed generation log available.</div>
          <div style={{ fontSize: 12, marginTop: 6 }}>
            The L2 spec was generated without detailed logging enabled.
          </div>
        </div>
      )}
    </div>
  );
}

export default L2GenerationLog;
