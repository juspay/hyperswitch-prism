import React from "react";
import { T } from "../../theme";

export const inputStyle: React.CSSProperties = {
  width: "100%",
  padding: "10px 14px",
  borderRadius: 6,
  border: `1px solid ${T.border}`,
  background: T.bg,
  color: T.text,
  fontSize: 14,
  outline: "none",
  fontFamily: "inherit",
  boxSizing: "border-box",
};

export function Section({
  title,
  description,
  children,
}: {
  title: string;
  description?: string;
  children: React.ReactNode;
}) {
  return (
    <div style={{ marginBottom: 24 }}>
      <h3
        style={{
          fontSize: 13,
          fontWeight: 700,
          color: T.textMuted,
          textTransform: "uppercase",
          letterSpacing: 1,
          margin: "0 0 4px 0",
        }}
      >
        {title}
      </h3>
      {description && (
        <p style={{ fontSize: 12, color: T.textSubtle, margin: "0 0 14px 0" }}>
          {description}
        </p>
      )}
      <div style={{ marginTop: description ? 0 : 12 }}>{children}</div>
    </div>
  );
}

export function Field({
  label,
  required,
  hint,
  children,
}: {
  label: string;
  required?: boolean;
  hint?: string;
  children: React.ReactNode;
}) {
  return (
    <div style={{ marginBottom: 16 }}>
      <label
        style={{
          display: "block",
          fontSize: 13,
          fontWeight: 600,
          color: T.textMuted,
          marginBottom: 6,
        }}
      >
        {label}
        {required && <span style={{ color: T.error, marginLeft: 4 }}>*</span>}
      </label>
      {children}
      {hint && (
        <div style={{ fontSize: 11, color: T.textSubtle, marginTop: 4 }}>
          {hint}
        </div>
      )}
    </div>
  );
}

export function Pill({
  active,
  disabled,
  onClick,
  children,
  title,
}: {
  active: boolean;
  disabled?: boolean;
  onClick: () => void;
  children: React.ReactNode;
  title?: string;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={disabled}
      title={title}
      style={{
        padding: "6px 12px",
        borderRadius: 999,
        border: `1px solid ${active ? T.accent : T.border}`,
        background: active ? T.accentSoft : T.bg,
        color: active ? T.accent : T.text,
        fontSize: 12,
        fontWeight: active ? 600 : 500,
        cursor: disabled ? "not-allowed" : "pointer",
        opacity: disabled ? 0.55 : 1,
        outline: "none",
      }}
    >
      {children}
    </button>
  );
}

export function Toggle({
  checked,
  onChange,
  label,
  hint,
}: {
  checked: boolean;
  onChange: (v: boolean) => void;
  label: string;
  hint?: string;
}) {
  return (
    <label
      style={{
        display: "flex",
        alignItems: "flex-start",
        gap: 10,
        cursor: "pointer",
        padding: "6px 0",
      }}
    >
      <input
        type="checkbox"
        checked={checked}
        onChange={(e) => onChange(e.target.checked)}
        style={{ marginTop: 3 }}
      />
      <span>
        <span style={{ fontSize: 14, color: T.text }}>{label}</span>
        {hint && (
          <span
            style={{
              display: "block",
              fontSize: 11,
              color: T.textSubtle,
              marginTop: 2,
            }}
          >
            {hint}
          </span>
        )}
      </span>
    </label>
  );
}

export function RepeatingList({
  items,
  placeholder,
  onAdd,
  onRemove,
  onUpdate,
}: {
  items: string[];
  placeholder: string;
  onAdd: (text: string) => void;
  onRemove: (index: number) => void;
  onUpdate?: (index: number, text: string) => void;
}) {
  const [draft, setDraft] = React.useState("");
  const add = () => {
    if (draft.trim()) {
      onAdd(draft.trim());
      setDraft("");
    }
  };
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
      {items.map((item, i) => (
        <div key={i} style={{ display: "flex", gap: 6 }}>
          {onUpdate ? (
            <input
              value={item}
              onChange={(e) => onUpdate(i, e.target.value)}
              style={{ ...inputStyle, flex: 1 }}
            />
          ) : (
            <span
              style={{
                ...inputStyle,
                flex: 1,
                display: "block",
                background: T.bg,
              }}
            >
              {item}
            </span>
          )}
          <button
            type="button"
            onClick={() => onRemove(i)}
            style={{
              padding: "0 10px",
              borderRadius: 6,
              border: `1px solid ${T.border}`,
              background: T.bg,
              color: T.textMuted,
              cursor: "pointer",
              fontSize: 13,
            }}
          >
            ✕
          </button>
        </div>
      ))}
      <div style={{ display: "flex", gap: 6 }}>
        <input
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && (e.preventDefault(), add())}
          placeholder={placeholder}
          style={{ ...inputStyle, flex: 1 }}
        />
        <button
          type="button"
          onClick={add}
          disabled={!draft.trim()}
          style={{
            padding: "8px 14px",
            borderRadius: 6,
            border: `1px solid ${T.border}`,
            background: draft.trim() ? T.bgElev : T.bg,
            color: draft.trim() ? T.text : T.textSubtle,
            cursor: draft.trim() ? "pointer" : "not-allowed",
            fontWeight: 600,
          }}
        >
          Add
        </button>
      </div>
    </div>
  );
}
