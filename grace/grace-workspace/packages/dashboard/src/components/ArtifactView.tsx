import React, { useState } from "react";
import { T } from "../theme";
import { Markdown } from "./Markdown";
import { L2GenerationLog } from "./L2GenerationLog";
import { FilesChangedPreview } from "./FilesChangedPreview";

function Card({ children }: { children: React.ReactNode }) {
  return (
    <div
      style={{
        background: T.bgElev,
        border: `1px solid ${T.border}`,
        borderRadius: 12,
        padding: 22,
        width: "83.33%",
        maxWidth: "100%",
        boxShadow: T.shadow,
      }}
    >
      {children}
    </div>
  );
}

function Field({
  label,
  children,
}: {
  label: string;
  children: React.ReactNode;
}) {
  return (
    <div style={{ marginBottom: 14 }}>
      <div
        style={{
          fontSize: 10,
          fontWeight: 700,
          letterSpacing: 0.8,
          textTransform: "uppercase",
          color: T.textMuted,
          marginBottom: 4,
        }}
      >
        {label}
      </div>
      <div style={{ fontSize: 13, color: T.text, lineHeight: 1.5 }}>
        {children}
      </div>
    </div>
  );
}

function Tag({
  children,
  tone = "neutral",
}: {
  children: React.ReactNode;
  tone?: "ok" | "warn" | "error" | "neutral";
}) {
  const palette: Record<string, { bg: string; fg: string }> = {
    ok: { bg: T.successSoft, fg: T.success },
    warn: { bg: T.warnSoft, fg: T.warn },
    error: { bg: T.errorSoft, fg: T.error },
    neutral: { bg: T.codeBg, fg: T.textMuted },
  };
  const p = palette[tone]!;
  return (
    <span
      style={{
        display: "inline-block",
        padding: "3px 10px",
        borderRadius: 999,
        background: p.bg,
        color: p.fg,
        fontSize: 11,
        fontWeight: 600,
        textTransform: "uppercase",
        letterSpacing: 0.5,
      }}
    >
      {children}
    </span>
  );
}

function Pre({ children }: { children: React.ReactNode }) {
  return (
    <pre
      style={{
        background: T.codeBg,
        border: `1px solid ${T.border}`,
        borderRadius: 8,
        padding: 14,
        fontSize: 12,
        color: T.text,
        maxHeight: 420,
        overflow: "auto",
        margin: 0,
        whiteSpace: "pre-wrap",
        wordBreak: "break-word",
      }}
    >
      {children}
    </pre>
  );
}

// ─── Per-checkpoint renderers ──────────────────────────────────────────

function TaskArtifact({ task }: { task: any }) {
  return (
    <Card>
      <div
        style={{
          fontSize: 18,
          fontWeight: 700,
          color: T.text,
          marginBottom: 14,
        }}
      >
        {task.title || "(untitled)"}
      </div>
      {task.description && (
        <Field label="Description">
          <Markdown text={task.description} />
        </Field>
      )}
      {Array.isArray(task.acceptanceCriteria) && task.acceptanceCriteria.length > 0 && (
        <Field label={`Acceptance criteria (${task.acceptanceCriteria.length})`}>
          <ul style={{ margin: 0, paddingLeft: 20 }}>
            {task.acceptanceCriteria.map((c: string, i: number) => (
              <li key={i} style={{ marginBottom: 4 }}>
                {c}
              </li>
            ))}
          </ul>
        </Field>
      )}
      {task.figmaUrl && (
        <Field label="Figma">
          <a href={task.figmaUrl} target="_blank" rel="noreferrer" style={{ color: T.accent }}>
            {task.figmaUrl}
          </a>
        </Field>
      )}
      {Array.isArray(task.targetFiles) && task.targetFiles.length > 0 && (
        <Field label="Target files">
          <div style={{ fontFamily: "ui-monospace, monospace", fontSize: 12 }}>
            {task.targetFiles.join(", ")}
          </div>
        </Field>
      )}
      {task.projectRoot && (
        <Field label="Project root">
          <div style={{ fontFamily: "ui-monospace, monospace", fontSize: 12 }}>
            {task.projectRoot}
          </div>
        </Field>
      )}
    </Card>
  );
}

function Paragraphs({ text }: { text: string }) {
  const cleaned = text.replace(/\r/g, "").trim();
  const paragraphs = cleaned.split(/\n{2,}/);
  return (
    <div style={{ fontSize: 14, color: T.text, lineHeight: 1.65 }}>
      {paragraphs.map((p, i) => {
        const lines = p.split("\n").filter(Boolean);
        const looksLikeList =
          lines.length > 1 &&
          lines.every((l) => /^[-*•]\s+/.test(l) || /^\d+[.)]\s+/.test(l));
        if (looksLikeList) {
          return (
            <ul key={i} style={{ margin: "0 0 12px 0", paddingLeft: 22 }}>
              {lines.map((l, j) => (
                <li key={j} style={{ marginBottom: 4 }}>
                  {l.replace(/^[-*•]\s+/, "").replace(/^\d+[.)]\s+/, "")}
                </li>
              ))}
            </ul>
          );
        }
        return (
          <p key={i} style={{ margin: "0 0 12px 0", whiteSpace: "pre-wrap" }}>
            {p}
          </p>
        );
      })}
    </div>
  );
}

function ProductAlignmentArtifact({ doc }: { doc: any }) {
  const approved = doc.approved === true;
  return (
    <Card>
      <div
        style={{
          display: "flex",
          alignItems: "center",
          gap: 12,
          marginBottom: 18,
          paddingBottom: 16,
          borderBottom: `1px solid ${T.border}`,
        }}
      >
        <div
          style={{
            width: 36,
            height: 36,
            borderRadius: "50%",
            background: approved ? T.successSoft : T.warnSoft,
            color: approved ? T.success : T.warn,
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            fontSize: 18,
            fontWeight: 700,
            flexShrink: 0,
          }}
        >
          {approved ? "✓" : "!"}
        </div>
        <div style={{ flex: 1 }}>
          <div
            style={{
              fontSize: 11,
              fontWeight: 700,
              color: T.textMuted,
              textTransform: "uppercase",
              letterSpacing: 0.8,
            }}
          >
            Product manager review
          </div>
          <div style={{ fontSize: 17, fontWeight: 700, color: T.text, marginTop: 2 }}>
            {approved ? "Approved" : "Concerns raised"}
          </div>
        </div>
      </div>

      {doc.notes && (
        <div style={{ marginBottom: 16 }}>
          <div
            style={{
              fontSize: 10,
              fontWeight: 700,
              letterSpacing: 0.8,
              textTransform: "uppercase",
              color: T.textMuted,
              marginBottom: 10,
            }}
          >
            Reviewer notes
          </div>
          <Markdown text={doc.notes} />
        </div>
      )}

      {Array.isArray(doc.adjustedCriteria) && doc.adjustedCriteria.length > 0 && (
        <div
          style={{
            marginTop: 18,
            paddingTop: 16,
            borderTop: `1px solid ${T.border}`,
          }}
        >
          <div
            style={{
              fontSize: 10,
              fontWeight: 700,
              letterSpacing: 0.8,
              textTransform: "uppercase",
              color: T.textMuted,
              marginBottom: 10,
            }}
          >
            Suggested criteria adjustments
          </div>
          <ul
            style={{
              margin: 0,
              paddingLeft: 22,
              fontSize: 14,
              color: T.text,
              lineHeight: 1.6,
            }}
          >
            {doc.adjustedCriteria.map((c: string, i: number) => (
              <li key={i} style={{ marginBottom: 6 }}>
                {c}
              </li>
            ))}
          </ul>
        </div>
      )}
    </Card>
  );
}

function DesignGateArtifact({ gate }: { gate: any }) {
  return (
    <Card>
      <div
        style={{
          display: "flex",
          alignItems: "center",
          gap: 10,
          marginBottom: 14,
        }}
      >
        <div style={{ fontSize: 16, fontWeight: 700, color: T.text }}>
          Design gate
        </div>
        <Tag tone={gate.designRequired ? "ok" : "neutral"}>
          {gate.designRequired ? "Design required" : "No design needed"}
        </Tag>
      </div>
      {gate.figmaUrl && (
        <Field label="Figma reference">
          <a href={gate.figmaUrl} target="_blank" rel="noreferrer" style={{ color: T.accent }}>
            {gate.figmaUrl}
          </a>
        </Field>
      )}
      {gate.skipReason && <Field label="Skip reason">{gate.skipReason}</Field>}
    </Card>
  );
}

function downloadL2Spec(spec: any) {
  const connector = spec.researchFindings?.connectorDocs?.[0]?.connector || "Connector";
  const content = spec.specContent || "No specification content available.";
  const blob = new Blob([content], { type: "text/markdown" });
  const url = URL.createObjectURL(blob);
  const link = document.createElement("a");
  link.href = url;
  link.download = `${connector}_Technical_Spec.md`;
  document.body.appendChild(link);
  link.click();
  document.body.removeChild(link);
  URL.revokeObjectURL(url);
}

function L2PlanningArtifact({ spec }: { spec: any }) {
  const hasGenerationLog = spec?.generationLog && (
    spec.generationLog.workflowExecutions?.length > 0 ||
    spec.generationLog.webSearchQueries?.length > 0 ||
    spec.generationLog.commandsExecuted?.length > 0 ||
    spec.generationLog.filesCreated?.length > 0
  );

  const hasSpecContent = !!spec.specContent;

  return (
    <div>
      {hasSpecContent ? (
        /* New layout: Prominent tech spec display */
        <div>
          {/* Main Tech Spec Content */}
          <Card>
            <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 18 }}>
              <div style={{ fontSize: 18, fontWeight: 700, color: T.text }}>
                Technical Specification
              </div>
              <button
                onClick={() => downloadL2Spec(spec)}
                title="Download L2 specification"
                style={{
                  display: "flex",
                  alignItems: "center",
                  gap: 6,
                  padding: "8px 14px",
                  borderRadius: 6,
                  border: `1px solid ${T.borderStrong}`,
                  background: T.bgElev,
                  color: T.text,
                  fontSize: 12,
                  fontWeight: 600,
                  cursor: "pointer",
                }}
                onMouseEnter={(e) => {
                  e.currentTarget.style.background = T.accentSoft;
                  e.currentTarget.style.borderColor = T.accent;
                }}
                onMouseLeave={(e) => {
                  e.currentTarget.style.background = T.bgElev;
                  e.currentTarget.style.borderColor = T.borderStrong;
                }}
              >
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                  <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
                  <polyline points="7 10 12 15 17 10" />
                  <line x1="12" y1="15" x2="12" y2="3" />
                </svg>
                Download Spec
              </button>
            </div>
            <div style={{ margin: "-8px 0" }}>
              <Markdown text={spec.specContent} />
            </div>
          </Card>

          {/* Metadata Section */}
          <div style={{ marginTop: 20 }}>
            <details>
              <summary
                style={{
                  fontSize: 11,
                  fontWeight: 700,
                  color: T.textMuted,
                  textTransform: "uppercase",
                  letterSpacing: 0.8,
                  cursor: "pointer",
                  padding: "8px 0",
                  display: "flex",
                  alignItems: "center",
                  gap: 8,
                }}
              >
                <span>▶</span>
                <span>Metadata & Constraints</span>
                <span style={{ flex: 1, height: 1, background: T.border, marginLeft: 8 }} />
              </summary>
              <div style={{ paddingTop: 16 }}>
                <Card>
                  <Field label="Summary">{spec.summary}</Field>
                  <Field label="Estimated Complexity">
                    <Tag
                      tone={
                        spec.estimatedComplexity === "low"
                          ? "ok"
                          : spec.estimatedComplexity === "high"
                            ? "error"
                            : "warn"
                      }
                    >
                      {spec.estimatedComplexity}
                    </Tag>
                  </Field>

                  {/* Documentation Verification */}
                  {spec.researchFindings?.connectorDocs?.length > 0 && (
                    <Field label="Documentation Verification">
                      <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
                        {spec.researchFindings.connectorDocs.map((doc: any, idx: number) => (
                          <div key={idx} style={{ padding: "8px 12px", background: T.bgElev, borderRadius: 6 }}>
                            <div style={{ display: "flex", alignItems: "center", gap: 8, marginBottom: 4 }}>
                              <span style={{ fontWeight: 600 }}>{doc.connector}</span>
                              <Tag
                                tone={
                                  doc.verificationStatus === "valid"
                                    ? "ok"
                                    : doc.verificationStatus === "insufficient"
                                      ? "error"
                                      : "warn"
                                }
                              >
                                {doc.verificationScore}/10 - {doc.verificationStatus}
                              </Tag>
                            </div>
                            {doc.urls?.length > 0 && (
                              <div style={{ fontSize: 12, color: T.textMuted }}>
                                {doc.urls.length} URL{doc.urls.length !== 1 ? "s" : ""} documented
                              </div>
                            )}
                          </div>
                        ))}
                      </div>
                    </Field>
                  )}

                  {/* Documentation Gaps */}
                  {spec.researchFindings?.documentationGaps?.length > 0 && (
                    <Field label="Documentation Gaps">
                      <ul style={{ margin: 0, paddingLeft: 20, color: T.warn }}>
                        {spec.researchFindings.documentationGaps.map((gap: string, i: number) => (
                          <li key={i}>{gap}</li>
                        ))}
                      </ul>
                    </Field>
                  )}

                  {Array.isArray(spec.technicalConstraints) && spec.technicalConstraints.length > 0 && (
                    <Field label="Technical Constraints">
                      <ul style={{ margin: 0, paddingLeft: 20 }}>
                        {spec.technicalConstraints.map((c: string, i: number) => (
                          <li key={i}>{c}</li>
                        ))}
                      </ul>
                    </Field>
                  )}
                </Card>
              </div>
            </details>
          </div>
        </div>
      ) : (
        /* Legacy layout: For specs without specContent */
        <Card>
          <div style={{ fontSize: 16, fontWeight: 700, color: T.text, marginBottom: 14 }}>
            L2 Specification
          </div>
          <Field label="Summary">
            <Markdown text={spec.summary ?? ""} />
          </Field>
          <Field label="In scope">
            <Markdown text={spec.scope ?? ""} />
          </Field>
          <Field label="Out of scope">
            <Markdown text={spec.outOfScope ?? ""} />
          </Field>
          {Array.isArray(spec.technicalConstraints) && spec.technicalConstraints.length > 0 && (
            <Field label="Technical constraints">
              <ul style={{ margin: 0, paddingLeft: 20 }}>
                {spec.technicalConstraints.map((c: string, i: number) => (
                  <li key={i}>{c}</li>
                ))}
              </ul>
            </Field>
          )}
          <Field label="Estimated complexity">
            <Tag
              tone={
                spec.estimatedComplexity === "low"
                  ? "ok"
                  : spec.estimatedComplexity === "high"
                    ? "error"
                    : "warn"
              }
            >
              {spec.estimatedComplexity}
            </Tag>
          </Field>
        </Card>
      )}

      {/* Generation Log Section */}
      {hasGenerationLog && (
        <div style={{ marginTop: 24 }}>
          <details>
            <summary
              style={{
                fontSize: 11,
                fontWeight: 700,
                color: T.textMuted,
                textTransform: "uppercase",
                letterSpacing: 0.8,
                cursor: "pointer",
                padding: "8px 0",
                display: "flex",
                alignItems: "center",
                gap: 8,
              }}
            >
              <span>▶</span>
              <span>Generation Details</span>
              <span style={{ flex: 1, height: 1, background: T.border, marginLeft: 8 }} />
            </summary>
            <div style={{ paddingTop: 12 }}>
              <L2GenerationLog log={spec.generationLog} />
            </div>
          </details>
        </div>
      )}
    </div>
  );
}

function L3AnalysisArtifact({ analysis }: { analysis: any }) {
  const a = analysis?.analysis ?? {};
  const hasPrerequisitesIssues = a.prerequisitesStatus === "incomplete" ||
    (a.missingPrerequisites?.length ?? 0) > 0;
  const hasRisks = (analysis?.riskAssessment?.length ?? 0) > 0;

  return (
    <Card>
      {/* Header */}
      <div
        style={{
          display: "flex",
          alignItems: "baseline",
          justifyContent: "space-between",
          marginBottom: 20,
          paddingBottom: 14,
          borderBottom: `1px solid ${T.border}`,
        }}
      >
        <div>
          <div
            style={{
              fontSize: 11,
              fontWeight: 700,
              color: T.textMuted,
              textTransform: "uppercase",
              letterSpacing: 0.8,
            }}
          >
            L3 Analysis (2.3_codegen.md Phase 4)
          </div>
          <div style={{ fontSize: 17, fontWeight: 700, color: T.text, marginTop: 2 }}>
            {analysis?.connector} · {analysis?.flow}
          </div>
        </div>
        <Tag
          tone={
            a.prerequisitesStatus === "complete"
              ? "ok"
              : hasPrerequisitesIssues
                ? "error"
                : "warn"
          }
        >
          {a.prerequisitesStatus}
        </Tag>
      </div>

      {/* Flow Status Alert */}
      {a.flowAlreadyExists && (
        <div
          style={{
            padding: 12,
            background: T.warnSoft,
            border: `1px solid ${T.warn}`,
            borderRadius: 8,
            marginBottom: 20,
          }}
        >
          <div style={{ fontSize: 13, fontWeight: 600, color: T.warn }}>
            Flow Already Implemented
          </div>
          <div style={{ fontSize: 12, color: T.text, marginTop: 4 }}>
            The {analysis?.flow} flow already exists on {analysis?.connector}. Implementation will be skipped.
          </div>
        </div>
      )}

      {/* Missing Prerequisites */}
      {hasPrerequisitesIssues && (
        <div
          style={{
            padding: 12,
            background: T.errorSoft,
            border: `1px solid ${T.error}`,
            borderRadius: 8,
            marginBottom: 20,
          }}
        >
          <div style={{ fontSize: 13, fontWeight: 600, color: T.error }}>
            Missing Prerequisites
          </div>
          <ul style={{ margin: "8px 0 0 0", paddingLeft: 20 }}>
            {a.missingPrerequisites?.map((p: string, i: number) => (
              <li key={i} style={{ fontSize: 12, color: T.text }}>{p}</li>
            ))}
          </ul>
        </div>
      )}

      {/* Patterns Identified */}
      {Array.isArray(a.patternsIdentified) && a.patternsIdentified.length > 0 && (
        <Field label={`Patterns Identified (${a.patternsIdentified.length})`}>
          <ul style={{ margin: 0, paddingLeft: 20 }}>
            {a.patternsIdentified.map((p: string, i: number) => (
              <li key={i} style={{ fontSize: 13, marginBottom: 4 }}>{p}</li>
            ))}
          </ul>
        </Field>
      )}

      {/* Files to Modify */}
      {Array.isArray(a.filesToModify) && a.filesToModify.length > 0 && (
        <Field label={`Files to Modify (${a.filesToModify.length})`}>
          <ul style={{ margin: 0, paddingLeft: 20 }}>
            {a.filesToModify.map((f: string, i: number) => (
              <li
                key={i}
                style={{
                  fontFamily: "ui-monospace, monospace",
                  fontSize: 12,
                  marginBottom: 4,
                }}
              >
                {f}
              </li>
            ))}
          </ul>
        </Field>
      )}

      {/* Existing Flows */}
      {Array.isArray(a.existingFlows) && a.existingFlows.length > 0 && (
        <Field label="Existing Flows">
          <div style={{ display: "flex", flexWrap: "wrap", gap: 8 }}>
            {a.existingFlows.map((f: string, i: number) => (
              <span
                key={i}
                style={{
                  padding: "2px 10px",
                  borderRadius: 999,
                  background: T.codeBg,
                  border: `1px solid ${T.border}`,
                  fontSize: 11,
                  fontFamily: "ui-monospace, monospace",
                }}
              >
                {f}
              </span>
            ))}
          </div>
        </Field>
      )}

      {/* Files Changed Preview */}
      {analysis?.specification?.filesChangedPreview &&
        analysis.specification.filesChangedPreview.length > 0 && (
        <Field label="Files that will be changed">
          <FilesChangedPreview files={analysis.specification.filesChangedPreview} />
        </Field>
      )}

      {/* Implementation Notes */}
      {analysis?.implementationNotes && (
        <Field label="Implementation Notes">
          <div
            style={{
              fontSize: 13,
              lineHeight: 1.6,
              color: T.text,
              padding: 12,
              background: T.codeBg,
              borderRadius: 8,
              whiteSpace: "pre-wrap",
            }}
          >
            {analysis.implementationNotes}
          </div>
        </Field>
      )}

      {/* Risk Assessment */}
      {hasRisks && (
        <div style={{ marginTop: 16 }}>
          <Field label={`Risk Assessment (${analysis.riskAssessment.length})`}>
            <ul style={{ margin: 0, paddingLeft: 20 }}>
              {analysis.riskAssessment.map((r: string, i: number) => (
                <li key={i} style={{ fontSize: 13, color: T.warn, marginBottom: 4 }}>
                  {r}
                </li>
              ))}
            </ul>
          </Field>
        </div>
      )}
    </Card>
  );
}

function ThinkingIndicator({ message }: { message: string }) {
  return (
    <div
      style={{
        display: "flex",
        alignItems: "center",
        gap: 10,
        padding: "12px 14px",
        background: T.accentSoft,
        border: `1px solid ${T.accent}`,
        borderRadius: 8,
        marginTop: 12,
      }}
    >
      <div
        style={{
          width: 14,
          height: 14,
          borderRadius: "50%",
          border: `2px solid ${T.accent}`,
          borderTopColor: "transparent",
          animation: "spin 0.9s linear infinite",
          flexShrink: 0,
        }}
      />
      <span style={{ fontSize: 13, color: T.accent, fontWeight: 600 }}>
        {message}
      </span>
    </div>
  );
}

// ─── Codegen / Implementation Result Artifact ──────────────────────────

function ImplementationResultArtifact({ result }: { result: any }) {
  // Phase 5 only: success=true with grpcurlResult="NOT_RUN" is expected
  // Phase 6+: success=true with grpcurlResult="PASS" indicates full success
  const passed = result?.success === true;
  const phase5Only = passed && result?.grpcurlResult === "NOT_RUN";

  return (
    <Card>
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 14 }}>
        <div>
          <div style={{ fontSize: 16, fontWeight: 700, color: T.text }}>
            Implementation Result
          </div>
          <div style={{ fontSize: 12, color: T.textMuted, marginTop: 2 }}>
            {result?.connector} · {result?.flow} · {result?.buildIterations ?? 0} iterations
          </div>
        </div>
        <Tag tone={passed ? (phase5Only ? "neutral" : "ok") : "error"}>
          {phase5Only ? "PHASE_5_COMPLETE" : (result?.grpcurlResult ?? "UNKNOWN")}
        </Tag>
      </div>

      {/* Files Modified */}
      {Array.isArray(result?.filesModified) && result.filesModified.length > 0 && (
        <Field label="Files Modified">
          <ul style={{ margin: 0, paddingLeft: 20 }}>
            {result.filesModified.map((f: string, i: number) => (
              <li key={i} style={{ fontFamily: "ui-monospace, monospace", fontSize: 12 }}>
                {f}
              </li>
            ))}
          </ul>
        </Field>
      )}

      {/* Fix Log */}
      {Array.isArray(result?.fixLog) && result.fixLog.length > 0 && (
        <Field label="Fix Log">
          <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
            {result.fixLog.map((entry: any, i: number) => (
              <div key={i} style={{ fontSize: 12, padding: 8, background: T.codeBg, borderRadius: 6 }}>
                <div><strong>Iteration {entry.iteration}:</strong> {entry.error}</div>
                <div style={{ color: T.textMuted }}>{entry.fileChanged} - {entry.changeDescription}</div>
              </div>
            ))}
          </div>
        </Field>
      )}

      {/* grpcurl Output */}
      {result?.grpcurlOutput && (
        <div style={{ marginTop: 16 }}>
          <details>
            <summary style={{ fontSize: 11, fontWeight: 700, color: T.textMuted, cursor: "pointer" }}>
              grpcurl Output
            </summary>
            <div style={{ marginTop: 8 }}>
              <Pre>{result.grpcurlOutput}</Pre>
            </div>
          </details>
        </div>
      )}

      {/* Reason - shown for both success and failure */}
      {result?.reason && (
        <div style={{
          marginTop: 12,
          padding: 10,
          background: passed ? T.successSoft : T.errorSoft,
          borderRadius: 6,
          borderLeft: `3px solid ${passed ? T.success : T.error}`
        }}>
          <div style={{ fontSize: 11, fontWeight: 700, color: passed ? T.success : T.error }}>
            {passed ? "Info" : "Failure Reason"}
          </div>
          <div style={{ fontSize: 12, color: T.text, marginTop: 4 }}>{result.reason}</div>
        </div>
      )}
    </Card>
  );
}

// ─── gRPC Test Result Artifact ──────────────────────────────────────────

function CopyButton({ text }: { text: string }) {
  const [copied, setCopied] = useState(false);
  const handleCopy = async () => {
    try {
      await navigator.clipboard.writeText(text);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch {
      // ignore
    }
  };
  return (
    <button
      onClick={handleCopy}
      style={{
        fontSize: 11,
        padding: "4px 10px",
        borderRadius: 4,
        border: `1px solid ${T.border}`,
        background: copied ? T.successSoft : T.bgElev,
        color: copied ? T.success : T.text,
        cursor: "pointer",
        marginLeft: 8,
      }}
    >
      {copied ? "Copied!" : "Copy"}
    </button>
  );
}

function GrpcTestArtifact({ result }: { result: any }) {
  const passed = result?.status === "SUCCESS" || result?.grpcurl_result === "PASS";
  const failed = result?.grpcurl_result === "FAIL" || result?.status === "FAILED";

  // Prefer copy_paste_command, fallback to grpcurl_command
  const copyPasteCmd = result?.copy_paste_command || result?.copyPasteCommand;
  const originalCmd = result?.grpcurl_command || result?.grpcCommand;
  const commandToShow = copyPasteCmd || originalCmd;

  return (
    <Card>
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 14 }}>
        <div>
          <div style={{ fontSize: 16, fontWeight: 700, color: T.text }}>
            gRPC Test Result
          </div>
          <div style={{ fontSize: 12, color: T.textMuted, marginTop: 2 }}>
            {passed ? "Tests passed" : failed ? "Tests failed" : "Test status unknown"}
          </div>
        </div>
        <Tag tone={passed ? "ok" : failed ? "error" : "neutral"}>
          {result?.grpcurl_result ?? result?.status ?? "UNKNOWN"}
        </Tag>
      </div>

      {/* Copy-Paste Command */}
      {commandToShow && (
        <Field label={copyPasteCmd ? "Copy-Paste Command" : "Command"}>
          <div style={{ display: "flex", alignItems: "flex-start" }}>
            <div style={{ flex: 1, overflow: "hidden" }}>
              <Pre>{commandToShow}</Pre>
            </div>
            <CopyButton text={commandToShow} />
          </div>
        </Field>
      )}

      {/* Request Payload */}
      {result?.request_payload && (
        <Field label="Request Payload">
          <Pre>{result.request_payload}</Pre>
        </Field>
      )}

      {/* grpcurl Output */}
      {(result?.grpcurl_output || result?.grpcOutput || result?.output) && (
        <Field label="Output">
          <Pre>{result.grpcurl_output || result.grpcOutput || result.output}</Pre>
        </Field>
      )}

      {/* Response Summary */}
      {result?.response_summary && (
        <Field label="Response Summary">
          <div style={{ fontSize: 13, color: T.text }}>{result.response_summary}</div>
        </Field>
      )}

      {/* Error Reason */}
      {result?.reason && (
        <div style={{
          marginTop: 12,
          padding: 10,
          background: T.errorSoft,
          borderRadius: 6,
          borderLeft: `3px solid ${T.error}`
        }}>
          <div style={{ fontSize: 11, fontWeight: 700, color: T.error }}>
            Error
          </div>
          <div style={{ fontSize: 12, color: T.text, marginTop: 4 }}>{result.reason}</div>
        </div>
      )}
    </Card>
  );
}

function DiffView({ diff }: { diff: string }) {
  const lines = diff.split("\n");
  return (
    <div
      style={{
        fontFamily: "ui-monospace, SFMono-Regular, Menlo, monospace",
        fontSize: 12,
        lineHeight: 1.5,
        overflow: "auto",
        maxHeight: 500,
        border: `1px solid ${T.border}`,
        borderRadius: 6,
        marginTop: 6,
      }}
    >
      {lines.map((line, i) => {
        let bg = "transparent";
        let color = T.text;
        if (line.startsWith("+++") || line.startsWith("---")) {
          bg = T.codeBg;
          color = T.textMuted;
        } else if (line.startsWith("+")) {
          bg = "rgba(46, 160, 67, 0.12)";
          color = "#2ea043";
        } else if (line.startsWith("-")) {
          bg = "rgba(248, 81, 73, 0.12)";
          color = "#f85149";
        }
        return (
          <div
            key={i}
            style={{
              padding: "0 10px",
              background: bg,
              color,
              whiteSpace: "pre-wrap",
              wordBreak: "break-all",
              minHeight: 20,
              borderBottom:
                line.startsWith("+++") ? `1px solid ${T.border}` : undefined,
            }}
          >
            <span style={{ display: "inline-block", width: 32, color: T.textSubtle, textAlign: "right", marginRight: 10, userSelect: "none" }}>
              {i + 1}
            </span>
            {line}
          </div>
        );
      })}
    </div>
  );
}

function countDiffLines(diff: string): { adds: number; dels: number } {
  const lines = diff.split("\n");
  return {
    adds: lines.filter((l) => l.startsWith("+") && !l.startsWith("+++")).length,
    dels: lines.filter((l) => l.startsWith("-") && !l.startsWith("---")).length,
  };
}

interface GroupedFile {
  path: string;
  /** Most recent changeType wins for the badge. */
  changeType: string;
  /** Sum of bytes across all entries for this path. */
  bytes: number;
  /** All individual entries (subtask writes) for this path. */
  entries: Array<{ changeType: string; bytes: number; diff?: string }>;
  /** Merged diff across all entries. */
  mergedDiff: string;
  adds: number;
  dels: number;
}

function groupFiles(files: any[]): GroupedFile[] {
  const map = new Map<string, GroupedFile>();
  for (const f of files) {
    const existing = map.get(f.path);
    if (existing) {
      existing.changeType = f.changeType;
      existing.bytes = f.bytes;
      existing.entries.push({ changeType: f.changeType, bytes: f.bytes, diff: f.diff });
      if (f.diff) {
        existing.mergedDiff += (existing.mergedDiff ? "\n" : "") + f.diff;
      }
    } else {
      map.set(f.path, {
        path: f.path,
        changeType: f.changeType,
        bytes: f.bytes,
        entries: [{ changeType: f.changeType, bytes: f.bytes, diff: f.diff }],
        mergedDiff: f.diff ?? "",
        adds: 0,
        dels: 0,
      });
    }
  }
  for (const g of map.values()) {
    if (g.mergedDiff) {
      const c = countDiffLines(g.mergedDiff);
      g.adds = c.adds;
      g.dels = c.dels;
    }
  }
  return Array.from(map.values());
}

function WorkerCard({ worker, selected, onClick }: { worker: any; selected: boolean; onClick: () => void }) {
  const isRunning = worker.status === "running";
  const isQueued = worker.status === "queued";
  const isDone = worker.status === "done";

  const statusConfig = isRunning
    ? { label: "WORKING", bg: T.accentSoft, border: T.accent, labelBg: T.accent, labelFg: "#fff" }
    : isQueued
    ? { label: "WAITING", bg: T.warnSoft, border: T.warn, labelBg: T.warn, labelFg: "#fff" }
    : isDone
    ? { label: "DONE", bg: T.successSoft, border: T.success, labelBg: T.success, labelFg: "#fff" }
    : { label: "IDLE", bg: T.bgElev, border: T.border, labelBg: T.codeBg, labelFg: T.textMuted };

  const fileName = worker.file ? worker.file.split("/").pop() : null;
  const dirPart = worker.file ? worker.file.replace(/\/[^/]+$/, "") : null;

  return (
    <div
      onClick={onClick}
      style={{
        border: `2px solid ${selected ? T.accent : statusConfig.border}`,
        borderRadius: 10,
        background: statusConfig.bg,
        minWidth: 0,
        cursor: "pointer",
        overflow: "hidden",
        outline: selected ? `3px solid ${T.accentSoft}` : undefined,
        outlineOffset: 2,
      }}
    >
      {/* Header row */}
      <div style={{ display: "flex", alignItems: "center", gap: 8, padding: "8px 12px", borderBottom: `1px solid ${statusConfig.border}` }}>
        {/* Spinner or static icon */}
        {isRunning ? (
          <div style={{
            width: 14, height: 14, borderRadius: "50%", flexShrink: 0,
            border: `2px solid ${T.accent}`, borderTopColor: "transparent",
            animation: "spin 0.8s linear infinite",
          }} />
        ) : isQueued ? (
          <span style={{ fontSize: 13, lineHeight: 1 }}>⏳</span>
        ) : isDone ? (
          <span style={{ fontSize: 13, lineHeight: 1 }}>✓</span>
        ) : (
          <div style={{ width: 14, height: 14, borderRadius: "50%", background: T.border, flexShrink: 0 }} />
        )}

        <span style={{ fontSize: 12, fontWeight: 700, color: isRunning ? T.accent : isQueued ? T.warn : isDone ? T.success : T.textMuted, flex: 1 }}>
          MINIme {worker.id}
        </span>

        {/* Status pill */}
        <span style={{
          fontSize: 9, fontWeight: 700, letterSpacing: 0.6, textTransform: "uppercase",
          padding: "2px 7px", borderRadius: 999,
          background: statusConfig.labelBg, color: statusConfig.labelFg,
        }}>
          {statusConfig.label}
        </span>
      </div>

      {/* Body */}
      <div style={{ padding: "8px 12px" }}>
        {isRunning && fileName ? (
          <>
            <div style={{ display: "flex", alignItems: "center", gap: 6, marginBottom: 4 }}>
              {worker.changeType && (
                <span style={{
                  padding: "1px 6px", borderRadius: 4, fontSize: 9, fontWeight: 700, textTransform: "uppercase",
                  background: worker.changeType === "create" ? T.successSoft : worker.changeType === "delete" ? T.errorSoft : T.accentSoft,
                  color: worker.changeType === "create" ? T.success : worker.changeType === "delete" ? T.error : T.accent,
                }}>
                  {worker.changeType}
                </span>
              )}
              <span style={{ fontSize: 12, fontWeight: 600, color: T.text, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                {fileName}
              </span>
            </div>
            {dirPart && (
              <div style={{ fontSize: 10, color: T.textSubtle, fontFamily: "ui-monospace, monospace", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                {dirPart}
              </div>
            )}
          </>
        ) : isQueued ? (
          <div style={{ fontSize: 11, color: T.warn }}>
            another MINIme is writing this file first...
          </div>
        ) : (
          <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
            <span style={{ fontSize: 11, color: T.textSubtle }}>
              {isDone ? "done, boss 🫡" : "waiting for task..."}
            </span>
            {worker.completedCount > 0 && (
              <span style={{ fontSize: 11, color: isDone ? T.success : T.textSubtle, fontWeight: 600 }}>
                {worker.completedCount} file{worker.completedCount !== 1 ? "s" : ""}
              </span>
            )}
          </div>
        )}

        {/* Progress count when running */}
        {isRunning && worker.completedCount > 0 && (
          <div style={{ marginTop: 6, fontSize: 10, color: T.textSubtle }}>
            {worker.completedCount} file{worker.completedCount !== 1 ? "s" : ""} done so far
          </div>
        )}
      </div>
    </div>
  );
}

function WorkerGrid({ workers, files }: { workers: any[]; files: any[] }) {
  const [selectedId, setSelectedId] = React.useState<number | null>(null);
  if (!workers.length) return null;

  const selectedWorker = workers.find((w) => w.id === selectedId);
  const selectedFiles = selectedId !== null ? files.filter((f) => f.workerId === selectedId) : [];
  const grouped = groupFiles(selectedFiles);

  return (
    <div style={{ marginBottom: 16 }}>
      <div style={{ fontSize: 10, fontWeight: 700, letterSpacing: 0.8, textTransform: "uppercase", color: T.textMuted, marginBottom: 8 }}>
        MINImes ({workers.length})
      </div>
      <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fill, minmax(180px, 1fr))", gap: 6 }}>
        {workers.map((w: any) => (
          <WorkerCard
            key={w.id}
            worker={w}
            selected={selectedId === w.id}
            onClick={() => setSelectedId(selectedId === w.id ? null : w.id)}
          />
        ))}
      </div>

      {selectedWorker && (
        <div
          style={{
            marginTop: 10,
            border: `1px solid ${T.accent}`,
            borderRadius: 8,
            overflow: "hidden",
          }}
        >
          <div
            style={{
              padding: "10px 14px",
              background: T.accentSoft,
              borderBottom: `1px solid ${T.accent}`,
              display: "flex",
              alignItems: "center",
              justifyContent: "space-between",
            }}
          >
            <span style={{ fontSize: 13, fontWeight: 700, color: T.accent }}>
              MINIme {selectedWorker.id} — {grouped.length} file{grouped.length !== 1 ? "s" : ""}
            </span>
            <span
              onClick={() => setSelectedId(null)}
              style={{ fontSize: 12, color: T.textMuted, cursor: "pointer", padding: "2px 6px" }}
            >
              ✕
            </span>
          </div>
          <div style={{ padding: 10, display: "flex", flexDirection: "column", gap: 4, maxHeight: 300, overflowY: "auto" }}>
            {grouped.length === 0 ? (
              <div style={{ fontSize: 12, color: T.textSubtle, padding: "8px 4px" }}>
                {selectedWorker.status === "running" ? "Working on it..." : "Nothing yet"}
              </div>
            ) : grouped.map((g) => (
              <details key={g.path}>
                <summary
                  style={{
                    display: "flex", alignItems: "center", gap: 8, padding: "5px 8px",
                    fontSize: 12, fontFamily: "ui-monospace, monospace",
                    background: T.bgElev, border: `1px solid ${T.border}`,
                    borderRadius: 6, cursor: g.mergedDiff ? "pointer" : "default",
                    listStyle: "none", userSelect: "none",
                  }}
                >
                  <span style={{
                    padding: "1px 6px", borderRadius: 4, fontSize: 10, fontWeight: 600, textTransform: "uppercase",
                    background: g.changeType === "create" ? T.successSoft : g.changeType === "delete" ? T.errorSoft : T.accentSoft,
                    color: g.changeType === "create" ? T.success : g.changeType === "delete" ? T.error : T.accent,
                  }}>
                    {g.changeType}
                  </span>
                  <span style={{ flex: 1, color: T.text }}>{g.path}</span>
                  {g.adds > 0 || g.dels > 0 ? (
                    <span style={{ fontSize: 11 }}>
                      <span style={{ color: "#2ea043" }}>+{g.adds}</span>{" "}
                      <span style={{ color: "#f85149" }}>-{g.dels}</span>
                    </span>
                  ) : null}
                </summary>
                {g.mergedDiff && <DiffView diff={g.mergedDiff} />}
              </details>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}

function ImplementationArtifact({ result, isRunning }: { result: any; isRunning?: boolean }) {
  const rawFiles = Array.isArray(result?.files) ? result.files : [];
  const workers = Array.isArray(result?.workers) ? result.workers : [];
  const grouped = groupFiles(rawFiles);
  const totalBytes = grouped.reduce((n, g) => n + g.bytes, 0);
  const totalAdds = grouped.reduce((n, g) => n + g.adds, 0);
  const totalDels = grouped.reduce((n, g) => n + g.dels, 0);
  return (
    <Card>
      <div style={{ fontSize: 16, fontWeight: 700, color: T.text, marginBottom: 4 }}>
        Implementation
      </div>
      <div style={{ fontSize: 12, color: T.textMuted, marginBottom: 14 }}>
        {grouped.length} {grouped.length === 1 ? "file" : "files"} changed
        {rawFiles.length !== grouped.length ? ` (${rawFiles.length} writes)` : ""}
        {" · "}
        {(totalBytes / 1024).toFixed(1)} KB
        {totalAdds > 0 || totalDels > 0 ? (
          <>
            {" · "}
            <span style={{ color: "#2ea043", fontWeight: 600 }}>+{totalAdds}</span>
            {" "}
            <span style={{ color: "#f85149", fontWeight: 600 }}>-{totalDels}</span>
          </>
        ) : null}
      </div>
      {workers.length > 0 && <WorkerGrid workers={workers} files={rawFiles} />}
      <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
        {grouped.map((g) => {
          const hasDiff = g.mergedDiff.length > 0;
          const multiWrite = g.entries.length > 1;
          return (
            <details key={g.path}>
              <summary
                style={{
                  display: "flex",
                  alignItems: "center",
                  gap: 10,
                  padding: "6px 8px",
                  fontSize: 12,
                  fontFamily: "ui-monospace, monospace",
                  background: T.bgElev,
                  border: `1px solid ${T.border}`,
                  borderRadius: 6,
                  cursor: hasDiff ? "pointer" : "default",
                  listStyle: "none",
                  userSelect: "none",
                }}
              >
                <span
                  style={{
                    padding: "1px 6px",
                    borderRadius: 4,
                    fontSize: 10,
                    fontWeight: 600,
                    textTransform: "uppercase",
                    background:
                      g.changeType === "create"
                        ? T.successSoft
                        : g.changeType === "delete"
                          ? T.errorSoft
                          : T.accentSoft,
                    color:
                      g.changeType === "create"
                        ? T.success
                        : g.changeType === "delete"
                          ? T.error
                          : T.accent,
                  }}
                >
                  {g.changeType}
                </span>
                <span style={{ color: T.text, flex: 1 }}>
                  {g.path}
                  {multiWrite && (
                    <span style={{ color: T.textSubtle, fontSize: 10, marginLeft: 6 }}>
                      ({g.entries.length} writes)
                    </span>
                  )}
                </span>
                {hasDiff && (
                  <span style={{ fontSize: 11 }}>
                    <span style={{ color: "#2ea043" }}>+{g.adds}</span>{" "}
                    <span style={{ color: "#f85149" }}>-{g.dels}</span>
                  </span>
                )}
                <span style={{ color: T.textMuted, fontSize: 11 }}>
                  {g.bytes}B
                </span>
                {hasDiff && (
                  <span style={{ color: T.textSubtle, fontSize: 10 }}>
                    ▸ diff
                  </span>
                )}
              </summary>
              {hasDiff && <DiffView diff={g.mergedDiff} />}
            </details>
          );
        })}
      </div>
      {isRunning && (
        <ThinkingIndicator message="Writing next file..." />
      )}
    </Card>
  );
}

function AgentReportSection({
  agentNumber,
  agentName,
  description,
  content,
  tone,
}: {
  agentNumber: number;
  agentName: string;
  description: string;
  content: string;
  tone: "ok" | "warn" | "neutral";
}) {
  const toneColors = {
    ok: { bg: T.successSoft, border: T.success, badge: T.success },
    warn: { bg: T.accentSoft, border: T.accent, badge: T.accent },
    neutral: { bg: T.codeBg, border: T.border, badge: T.textMuted },
  };
  const c = toneColors[tone];
  return (
    <div
      style={{
        border: `1px solid ${c.border}`,
        borderRadius: 10,
        marginBottom: 16,
        overflow: "hidden",
      }}
    >
      <div
        style={{
          padding: "12px 16px",
          background: c.bg,
          display: "flex",
          alignItems: "center",
          gap: 10,
          borderBottom: `1px solid ${c.border}`,
        }}
      >
        <span
          style={{
            width: 26,
            height: 26,
            borderRadius: "50%",
            background: c.badge,
            color: "#fff",
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            fontSize: 12,
            fontWeight: 700,
            flexShrink: 0,
          }}
        >
          {agentNumber}
        </span>
        <div>
          <div style={{ fontSize: 13, fontWeight: 700, color: T.text }}>{agentName}</div>
          <div style={{ fontSize: 11, color: T.textMuted }}>{description}</div>
        </div>
      </div>
      <div
        style={{
          padding: 16,
          maxHeight: 500,
          overflow: "auto",
          background: T.bg,
        }}
      >
        <Markdown text={content || "(empty)"} />
      </div>
    </div>
  );
}

function FeatureResearchArtifact({ report }: { report: any }) {
  return (
    <Card>
      <div style={{ fontSize: 18, fontWeight: 700, color: T.text, marginBottom: 4 }}>
        Feature Research
      </div>
      <div style={{ fontSize: 12, color: T.textMuted, marginBottom: 20 }}>
        Three agents: Agent 1 + Agent 2 run in parallel, Agent 3 runs after both complete.
      </div>

      {/* Agent 1: Existing Structure */}
      <AgentReportSection
        agentNumber={1}
        agentName="Existing Structure Scout"
        description="Explored the repo — what's already there"
        content={report.existingStructure}
        tone="ok"
      />

      {/* Agent 2: Ideal Flow */}
      <AgentReportSection
        agentNumber={2}
        agentName="Ideal Flow Researcher"
        description="Researched the web — how this feature is typically built"
        content={report.idealFlow}
        tone="warn"
      />

      {/* Agent 3: Final Decision — highlighted */}
      <div
        style={{
          border: `2px solid ${T.accent}`,
          borderRadius: 10,
          marginBottom: 16,
          overflow: "hidden",
        }}
      >
        <div
          style={{
            padding: "14px 16px",
            background: T.accentSoft,
            display: "flex",
            alignItems: "center",
            gap: 10,
            borderBottom: `2px solid ${T.accent}`,
          }}
        >
          <span
            style={{
              width: 26,
              height: 26,
              borderRadius: "50%",
              background: T.accent,
              color: "#fff",
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              fontSize: 12,
              fontWeight: 700,
              flexShrink: 0,
            }}
          >
            3
          </span>
          <div>
            <div style={{ fontSize: 14, fontWeight: 700, color: T.text }}>
              Final Decision
            </div>
            <div style={{ fontSize: 11, color: T.textMuted }}>
              Synthesized from Agent 1 + Agent 2 — this is what L2 will use
            </div>
          </div>
        </div>
        <div style={{ padding: 16, background: T.bg }}>
          <div style={{ fontSize: 14, color: T.text, lineHeight: 1.6, marginBottom: 14 }}>
            <Markdown text={report.finalDecision ?? "(empty)"} />
          </div>
          {Array.isArray(report.actionItems) && report.actionItems.length > 0 && (
            <div>
              <div
                style={{
                  fontSize: 10,
                  fontWeight: 700,
                  letterSpacing: 0.8,
                  textTransform: "uppercase",
                  color: T.textMuted,
                  marginBottom: 8,
                }}
              >
                Action Items ({report.actionItems.length})
              </div>
              <ol style={{ margin: 0, paddingLeft: 20 }}>
                {report.actionItems.map((item: string, i: number) => (
                  <li key={i} style={{ marginBottom: 6, fontSize: 13, lineHeight: 1.5, color: T.text }}>
                    {item}
                  </li>
                ))}
              </ol>
            </div>
          )}
        </div>
      </div>
    </Card>
  );
}

// ─── Compiler Artifact ─────────────────────────────────────────────────

function CompilerArtifact({ result }: { result: any }) {
  // Handle compiler.ts format:
  // - On failure: result is an array of error strings (from compilationErrors)
  // - On success: result is { compiledFiles: [] }
  const errors = Array.isArray(result) ? result : result?.compilationErrors;
  const hasErrors = errors && errors.length > 0;
  const compiledFiles = result?.compiledFiles || [];

  return (
    <Card>
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 14 }}>
        <div>
          <div style={{ fontSize: 16, fontWeight: 700, color: T.text }}>
            Compiler Result
          </div>
          <div style={{ fontSize: 12, color: T.textMuted, marginTop: 2 }}>
            {compiledFiles.length > 0 ? `${compiledFiles.length} files compiled` : "Build check"}
          </div>
        </div>
        <Tag tone={hasErrors ? "error" : "ok"}>
          {hasErrors ? "FAILED" : "PASSED"}
        </Tag>
      </div>

      {hasErrors && (
        <Field label="Compilation Errors">
          <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
            {errors.map((error: string, i: number) => (
              <div
                key={i}
                style={{
                  padding: "10px 12px",
                  background: T.errorSoft,
                  border: `1px solid ${T.error}`,
                  borderRadius: 6,
                  fontSize: 12,
                  fontFamily: "ui-monospace, SFMono-Regular, Menlo, monospace",
                  color: T.text,
                  whiteSpace: "pre-wrap",
                  wordBreak: "break-word",
                }}
              >
                {error}
              </div>
            ))}
          </div>
        </Field>
      )}

      {!hasErrors && compiledFiles.length > 0 && (
        <Field label="Compiled Files">
          <ul style={{ margin: 0, paddingLeft: 20 }}>
            {compiledFiles.map((file: string, i: number) => (
              <li key={i} style={{ fontFamily: "ui-monospace, monospace", fontSize: 12 }}>
                {file}
              </li>
            ))}
          </ul>
        </Field>
      )}
    </Card>
  );
}

// ─── Codegen Artifact ──────────────────────────────────────────────────

// ─── PR Review Artifact ────────────────────────────────────────────────

function PrReviewArtifact({ result }: { result: any }) {
  const r = (result ?? {}) as {
    approved?: boolean;
    specComplianceScore?: number;
    comments?: Array<{
      file: string;
      line?: number | null;
      comment: string;
      severity: "info" | "warning" | "blocking";
    }>;
    status?: "SUCCESS" | "FAILED";
    prUrl?: string;
    prNumber?: number;
    branchName?: string;
    commitHash?: string;
    reason?: string;
  };

  const isFailed = r.status === "FAILED";
  const statusTone: "ok" | "error" | "warn" = isFailed
    ? "error"
    : r.status === "SUCCESS"
      ? "ok"
      : "warn";

  const commentsBySeverity = {
    blocking: (r.comments ?? []).filter((c) => c.severity === "blocking"),
    warning: (r.comments ?? []).filter((c) => c.severity === "warning"),
    info: (r.comments ?? []).filter((c) => c.severity === "info"),
  };

  return (
    <Card>
      {/* Header: status + score + View PR anchor */}
      <div
        style={{
          display: "flex",
          alignItems: "center",
          gap: 12,
          flexWrap: "wrap",
          marginBottom: 18,
        }}
      >
        <Tag tone={statusTone}>{r.status ?? "UNKNOWN"}</Tag>
        <Tag tone={r.approved ? "ok" : "warn"}>
          {r.approved ? "Approved" : "Not approved"}
        </Tag>
        {typeof r.specComplianceScore === "number" && (
          <Tag tone="neutral">
            score {r.specComplianceScore.toFixed(2)}
          </Tag>
        )}
        {r.prUrl ? (
          <a
            href={r.prUrl}
            target="_blank"
            rel="noreferrer"
            style={{
              marginLeft: "auto",
              fontSize: 12,
              fontWeight: 600,
              color: T.accent,
              padding: "5px 12px",
              border: `1px solid ${T.accent}`,
              borderRadius: 6,
              textDecoration: "none",
            }}
          >
            View PR{r.prNumber ? ` #${r.prNumber}` : ""} ↗
          </a>
        ) : (
          <span
            style={{
              marginLeft: "auto",
              fontSize: 11,
              fontStyle: "italic",
              color: T.textSubtle,
            }}
          >
            PR link not captured
          </span>
        )}
      </div>

      {/* Failure reason callout */}
      {isFailed && r.reason && (
        <div
          style={{
            padding: "10px 14px",
            borderRadius: 8,
            background: T.errorSoft,
            color: T.error,
            fontSize: 13,
            lineHeight: 1.5,
            marginBottom: 18,
            border: `1px solid ${T.error}30`,
          }}
        >
          <div style={{ fontWeight: 700, marginBottom: 4 }}>Failure reason</div>
          {r.reason}
        </div>
      )}

      {/* PR metadata */}
      {(r.branchName || r.commitHash) && (
        <div style={{ marginBottom: 18 }}>
          {r.branchName && (
            <Field label="Branch">
              <code style={{ fontFamily: "ui-monospace, monospace", fontSize: 12 }}>
                {r.branchName}
              </code>
            </Field>
          )}
          {r.commitHash && (
            <Field label="Commit">
              <code style={{ fontFamily: "ui-monospace, monospace", fontSize: 12 }}>
                {r.commitHash.slice(0, 12)}
              </code>
            </Field>
          )}
        </div>
      )}

      {/* Review feedback grouped by severity */}
      {(r.comments ?? []).length > 0 && (
        <div>
          <div
            style={{
              fontSize: 10,
              fontWeight: 700,
              letterSpacing: 0.8,
              textTransform: "uppercase",
              color: T.textMuted,
              marginBottom: 10,
            }}
          >
            Review feedback ({r.comments!.length})
          </div>
          {(["blocking", "warning", "info"] as const).map((sev) => {
            const list = commentsBySeverity[sev];
            if (list.length === 0) return null;
            const tone: "error" | "warn" | "neutral" =
              sev === "blocking" ? "error" : sev === "warning" ? "warn" : "neutral";
            return (
              <div key={sev} style={{ marginBottom: 12 }}>
                <div style={{ marginBottom: 6 }}>
                  <Tag tone={tone}>
                    {sev} ({list.length})
                  </Tag>
                </div>
                <ul style={{ margin: 0, paddingLeft: 20 }}>
                  {list.map((c, i) => (
                    <li
                      key={i}
                      style={{
                        fontSize: 13,
                        color: T.text,
                        marginBottom: 6,
                        lineHeight: 1.5,
                      }}
                    >
                      <code
                        style={{
                          fontFamily: "ui-monospace, monospace",
                          fontSize: 11,
                          color: T.textMuted,
                        }}
                      >
                        {c.file}
                        {c.line ? `:${c.line}` : ""}
                      </code>{" "}
                      — {c.comment}
                    </li>
                  ))}
                </ul>
              </div>
            );
          })}
        </div>
      )}
    </Card>
  );
}

// ─── Dispatcher ────────────────────────────────────────────────────────

export function ArtifactView({
  checkpointId,
  artifact,
  artifacts,
  isRunning,
}: {
  checkpointId: string;
  artifact: unknown;
  artifacts?: Record<string, unknown>;
  isRunning?: boolean;
}) {
  if (artifact === undefined || artifact === null) return null;

  switch (checkpointId) {
    case "task":
      return <TaskArtifact task={artifact as any} />;
    case "product_alignment":
      return <ProductAlignmentArtifact doc={artifact as any} />;
    case "feature_research":
      return <FeatureResearchArtifact report={artifact as any} />;
    case "design_gate":
      return <DesignGateArtifact gate={artifact as any} />;
    case "l2_planning":
      return <L2PlanningArtifact spec={artifact as any} />;
    case "l3_analysis":
      return <L3AnalysisArtifact analysis={artifact as any} />;
    case "implementation":
      return <ImplementationResultArtifact result={artifact as any} />;
    case "compiler":
      return <CompilerArtifact result={artifact} />;
    case "grpc_test":
      return <GrpcTestArtifact result={artifact as any} />;
    case "pr_review":
      return <PrReviewArtifact result={artifact as any} />;
    default:
      return (
        <Card>
          <Pre>{JSON.stringify(artifact, null, 2)}</Pre>
        </Card>
      );
  }
}
