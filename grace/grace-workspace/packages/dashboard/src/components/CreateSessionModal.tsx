import { useState } from "react";
import type { SessionCopyStrategy } from "../hooks/useSessions";
import { T } from "../theme";

interface Props {
  defaultSourcePath: string;
  onCreate: (input: {
    name: string;
    description?: string;
    sourcePath: string;
    strategy: SessionCopyStrategy;
  }) => void;
  onClose: () => void;
}

/**
 * Modal that asks for the four bits of information SessionManager.create
 * needs: a human-readable name, optional description, source folder, and
 * copy strategy. We default the source path to the supervisor's
 * config.projectRoot — that's what most users want.
 */
export function CreateSessionModal({
  defaultSourcePath,
  onCreate,
  onClose,
}: Props) {
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [sourcePath, setSourcePath] = useState(defaultSourcePath);
  const [strategy, setStrategy] = useState<SessionCopyStrategy>("git-worktree");

  const submit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!name.trim()) return;
    onCreate({
      name: name.trim(),
      description: description.trim() || undefined,
      sourcePath: sourcePath.trim(),
      strategy,
    });
  };

  return (
    <div
      style={{
        position: "fixed",
        inset: 0,
        background: "rgba(0,0,0,0.32)",
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        zIndex: 100,
      }}
      onClick={onClose}
    >
      <form
        onClick={(e) => e.stopPropagation()}
        onSubmit={submit}
        style={{
          width: 520,
          background: T.bgElev,
          borderRadius: 10,
          padding: 24,
          boxShadow: T.shadowLg,
          display: "flex",
          flexDirection: "column",
          gap: 16,
          color: T.text,
        }}
      >
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
          <h2 style={{ margin: 0, fontSize: 16, fontWeight: 700 }}>Create New Session</h2>
          <button
            type="button"
            onClick={onClose}
            style={{ border: "none", background: "transparent", cursor: "pointer", color: T.textMuted, fontSize: 18 }}
          >
            ×
          </button>
        </div>

        <Field label="Session Name *">
          <input
            autoFocus
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder="ApplePay / Stripe Implementation"
            style={inputStyle()}
          />
        </Field>

        <Field label="Description (optional)">
          <textarea
            value={description}
            onChange={(e) => setDescription(e.target.value)}
            rows={2}
            placeholder="What this session is for…"
            style={{ ...inputStyle(), resize: "vertical", fontFamily: "inherit" }}
          />
        </Field>

        <Field
          label="Source Folder"
          hint="A copy of this folder will be created for isolation. Must be a git repo for the worktree strategy."
        >
          <input
            value={sourcePath}
            onChange={(e) => setSourcePath(e.target.value)}
            style={inputStyle()}
          />
        </Field>

        <Field label="Copy Strategy">
          <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
            {(
              [
                ["git-worktree", "Git worktree (fast, recommended)"],
                ["full", "Full copy (slow, complete isolation; skips node_modules/target)"],
                ["shallow", "Shallow git clone (fastest, depth=1)"],
              ] as [SessionCopyStrategy, string][]
            ).map(([value, label]) => (
              <label
                key={value}
                style={{ display: "flex", alignItems: "center", gap: 8, cursor: "pointer", fontSize: 13 }}
              >
                <input
                  type="radio"
                  name="strategy"
                  checked={strategy === value}
                  onChange={() => setStrategy(value)}
                />
                {label}
              </label>
            ))}
          </div>
        </Field>

        <div style={{ display: "flex", justifyContent: "flex-end", gap: 8, marginTop: 4 }}>
          <button
            type="button"
            onClick={onClose}
            style={{
              padding: "7px 14px",
              borderRadius: 6,
              border: `1px solid ${T.border}`,
              background: "transparent",
              color: T.text,
              fontSize: 13,
              cursor: "pointer",
            }}
          >
            Cancel
          </button>
          <button
            type="submit"
            disabled={!name.trim()}
            style={{
              padding: "7px 16px",
              borderRadius: 6,
              border: "none",
              background: name.trim() ? T.accent : T.border,
              color: "#fff",
              fontWeight: 600,
              fontSize: 13,
              cursor: name.trim() ? "pointer" : "not-allowed",
            }}
          >
            Create Session
          </button>
        </div>
      </form>
    </div>
  );
}

function Field({
  label,
  hint,
  children,
}: {
  label: string;
  hint?: string;
  children: React.ReactNode;
}) {
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 5 }}>
      <span style={{ fontSize: 12, fontWeight: 600, color: T.textMuted }}>{label}</span>
      {children}
      {hint && (
        <span style={{ fontSize: 11, color: T.textSubtle }}>ℹ {hint}</span>
      )}
    </div>
  );
}

function inputStyle(): React.CSSProperties {
  return {
    padding: "8px 10px",
    borderRadius: 6,
    border: `1px solid ${T.border}`,
    background: T.bg,
    color: T.text,
    fontSize: 13,
    outline: "none",
    fontFamily: "inherit",
  };
}
