import { useState, useCallback } from "react";
import { T } from "../theme";
import { Markdown } from "./Markdown";

interface Attachment {
  name: string;
  dataUrl: string;
}

export function ClarifyingQuestions({
  notes,
  questions,
  onSubmit,
}: {
  notes?: string;
  questions: string[];
  onSubmit: (payload: {
    answers: Record<string, string>;
    attachments: Record<string, Attachment[]>;
  }) => void;
}) {
  const [answers, setAnswers] = useState<Record<string, string>>({});
  const [attachments, setAttachments] = useState<Record<string, Attachment[]>>(
    {}
  );
  const [error, setError] = useState<string | null>(null);

  const handlePaste = useCallback(
    (q: string, e: React.ClipboardEvent<HTMLTextAreaElement>) => {
      const items = Array.from(e.clipboardData.items);
      const images = items.filter((i) => i.type.startsWith("image/"));
      if (images.length === 0) return;
      e.preventDefault();
      for (const item of images) {
        const file = item.getAsFile();
        if (!file) continue;
        const reader = new FileReader();
        reader.onload = () => {
          const dataUrl = reader.result as string;
          setAttachments((a) => ({
            ...a,
            [q]: [
              ...(a[q] ?? []),
              { name: file.name || `pasted-${Date.now()}.png`, dataUrl },
            ],
          }));
        };
        reader.readAsDataURL(file);
      }
    },
    []
  );

  const handleFile = (q: string, files: FileList | null) => {
    if (!files) return;
    for (const file of Array.from(files)) {
      if (!file.type.startsWith("image/")) continue;
      const reader = new FileReader();
      reader.onload = () => {
        const dataUrl = reader.result as string;
        setAttachments((a) => ({
          ...a,
          [q]: [...(a[q] ?? []), { name: file.name, dataUrl }],
        }));
      };
      reader.readAsDataURL(file);
    }
  };

  const removeAttachment = (q: string, idx: number) => {
    setAttachments((a) => ({
      ...a,
      [q]: (a[q] ?? []).filter((_, i) => i !== idx),
    }));
  };

  const hasAnswer = (q: string) =>
    (answers[q] ?? "").trim().length > 0 || (attachments[q] ?? []).length > 0;

  const allAnswered = questions.every(hasAnswer);

  const submit = () => {
    if (!allAnswered) {
      setError(
        "Every question needs either a text answer or at least one attachment."
      );
      return;
    }
    setError(null);
    onSubmit({ answers, attachments });
  };

  return (
    <div
      style={{
        background: T.warnSoft,
        border: `1px solid ${T.warn}`,
        borderRadius: 12,
        padding: 24,
        maxWidth: 760,
        boxShadow: T.shadow,
      }}
    >
      <div style={{ display: "flex", alignItems: "center", gap: 12, marginBottom: 18 }}>
        <div
          style={{
            width: 34,
            height: 34,
            borderRadius: "50%",
            background: T.warn,
            color: "#fff",
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            fontSize: 17,
            fontWeight: 700,
            flexShrink: 0,
            animation: "pulse 1.8s ease-in-out infinite",
          }}
        >
          ?
        </div>
        <div>
          <div
            style={{
              fontSize: 11,
              fontWeight: 700,
              letterSpacing: 0.8,
              textTransform: "uppercase",
              color: T.warn,
            }}
          >
            Clarifying questions required
          </div>
          <div style={{ fontSize: 16, fontWeight: 700, color: T.text, marginTop: 2 }}>
            PM needs more detail before stage 3 can start
          </div>
        </div>
      </div>

      {notes && (
        <div
          style={{
            marginBottom: 20,
            paddingBottom: 16,
            borderBottom: `1px solid ${T.warn}`,
          }}
        >
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
            Reviewer notes
          </div>
          <Markdown text={notes} />
        </div>
      )}

      {questions.map((q, i) => {
        const attached = attachments[q] ?? [];
        return (
          <div key={i} style={{ marginBottom: 18 }}>
            <div
              style={{
                display: "flex",
                alignItems: "flex-start",
                gap: 10,
                marginBottom: 8,
              }}
            >
              <div
                style={{
                  width: 22,
                  height: 22,
                  borderRadius: "50%",
                  background: T.accent,
                  color: "#fff",
                  fontSize: 11,
                  fontWeight: 700,
                  display: "flex",
                  alignItems: "center",
                  justifyContent: "center",
                  flexShrink: 0,
                  marginTop: 1,
                }}
              >
                {i + 1}
              </div>
              <div
                style={{
                  fontSize: 14,
                  fontWeight: 600,
                  color: T.text,
                  lineHeight: 1.45,
                  flex: 1,
                }}
              >
                {q}
              </div>
            </div>
            <div style={{ marginLeft: 32 }}>
              <textarea
                value={answers[q] ?? ""}
                onChange={(e) => {
                  setAnswers((a) => ({ ...a, [q]: e.target.value }));
                  setError(null);
                }}
                onPaste={(e) => handlePaste(q, e)}
                placeholder="Type an answer or paste a screenshot (⌘V / Ctrl+V)…"
                style={{
                  width: "100%",
                  minHeight: 64,
                  padding: "10px 12px",
                  border: `1px solid ${T.border}`,
                  borderRadius: 8,
                  background: T.bgElev,
                  color: T.text,
                  fontSize: 13,
                  fontFamily: "inherit",
                  resize: "vertical",
                  boxSizing: "border-box",
                }}
              />
              {/* Attachment previews */}
              {attached.length > 0 && (
                <div
                  style={{
                    display: "flex",
                    flexWrap: "wrap",
                    gap: 8,
                    marginTop: 8,
                  }}
                >
                  {attached.map((att, j) => (
                    <div
                      key={j}
                      style={{
                        position: "relative",
                        border: `1px solid ${T.border}`,
                        borderRadius: 8,
                        overflow: "hidden",
                        background: T.bgElev,
                        width: 120,
                      }}
                    >
                      <img
                        src={att.dataUrl}
                        alt={att.name}
                        style={{
                          display: "block",
                          width: "100%",
                          height: 80,
                          objectFit: "cover",
                        }}
                      />
                      <div
                        style={{
                          fontSize: 10,
                          color: T.textMuted,
                          padding: "4px 6px",
                          overflow: "hidden",
                          textOverflow: "ellipsis",
                          whiteSpace: "nowrap",
                        }}
                      >
                        {att.name}
                      </div>
                      <button
                        onClick={() => removeAttachment(q, j)}
                        style={{
                          position: "absolute",
                          top: 4,
                          right: 4,
                          width: 20,
                          height: 20,
                          borderRadius: "50%",
                          border: "none",
                          background: "rgba(0,0,0,0.6)",
                          color: "#fff",
                          fontSize: 12,
                          fontWeight: 700,
                          cursor: "pointer",
                          lineHeight: 1,
                        }}
                        aria-label="Remove"
                      >
                        ×
                      </button>
                    </div>
                  ))}
                </div>
              )}
              {/* File picker link */}
              <label
                style={{
                  display: "inline-block",
                  marginTop: 6,
                  fontSize: 11,
                  color: T.accent,
                  cursor: "pointer",
                  textDecoration: "underline",
                }}
              >
                + attach image
                <input
                  type="file"
                  accept="image/*"
                  multiple
                  onChange={(e) => handleFile(q, e.target.files)}
                  style={{ display: "none" }}
                />
              </label>
            </div>
          </div>
        );
      })}

      {error && (
        <div
          style={{
            marginTop: 8,
            padding: "9px 12px",
            background: T.errorSoft,
            border: `1px solid ${T.error}`,
            borderRadius: 6,
            color: T.error,
            fontSize: 12,
          }}
        >
          {error}
        </div>
      )}

      <div
        style={{
          marginTop: 18,
          paddingTop: 16,
          borderTop: `1px solid ${T.warn}`,
          display: "flex",
          justifyContent: "space-between",
          alignItems: "center",
          gap: 12,
        }}
      >
        <div style={{ fontSize: 11, color: T.textMuted }}>
          {questions.filter(hasAnswer).length} / {questions.length} answered · text
          or images both count
        </div>
        <button
          onClick={submit}
          disabled={!allAnswered}
          style={{
            padding: "9px 20px",
            borderRadius: 8,
            border: "none",
            background: allAnswered ? T.accent : T.border,
            color: allAnswered ? "#fff" : T.textSubtle,
            fontSize: 13,
            fontWeight: 700,
            cursor: allAnswered ? "pointer" : "not-allowed",
          }}
        >
          Submit answers →
        </button>
      </div>
    </div>
  );
}
