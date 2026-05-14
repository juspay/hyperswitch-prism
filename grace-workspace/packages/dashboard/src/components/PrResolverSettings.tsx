import { useEffect, useMemo, useState } from "react";
import { T } from "../theme";
import type {
  EffectiveConfig,
  RuntimeOverlay,
} from "../hooks/usePrResolver";

interface Props {
  effectiveConfig: EffectiveConfig | null;
  runtimeOverlay: RuntimeOverlay;
  running: boolean;
  onSave: (overlay: RuntimeOverlay) => void;
  onReset: () => void;
}

const FIELD_LABELS = {
  githubRepo: "GitHub repo (owner/name)",
  trigger: "Trigger tag",
  pollInterval: "Poll interval (seconds)",
  maxConcurrent: "Max concurrent PRs",
  maxBuildLoops: "Max build-fix loops",
  maxCommentsPerCycle: "Max comments per cycle",
} as const;

/**
 * Settings panel for the PR Resolver. Mirrors the supervisor's runtime
 * overlay: fields default to the supervisor's effective config; saving
 * sends the form as a new overlay (the supervisor validates, persists,
 * and restarts the service). "Reset to defaults" clears the overlay so
 * the in-file config.yml values take over again.
 */
export function PrResolverSettings({
  effectiveConfig,
  runtimeOverlay,
  running,
  onSave,
  onReset,
}: Props) {
  const [form, setForm] = useState<EffectiveConfig | null>(null);

  // Re-hydrate from the supervisor when its effectiveConfig changes, but
  // only when the form isn't dirty — otherwise we'd clobber what the user
  // is currently typing.
  useEffect(() => {
    setForm((prev) => {
      if (!effectiveConfig) return prev;
      if (!prev) return effectiveConfig;
      const isDirty =
        JSON.stringify(prev) !== JSON.stringify(effectiveConfig);
      return isDirty ? prev : effectiveConfig;
    });
  }, [effectiveConfig]);

  const isDirty = useMemo(() => {
    if (!form || !effectiveConfig) return false;
    return JSON.stringify(form) !== JSON.stringify(effectiveConfig);
  }, [form, effectiveConfig]);

  const overrideCount = useMemo(() => {
    return Object.values(runtimeOverlay).filter((v) => v !== undefined).length;
  }, [runtimeOverlay]);

  if (!form) {
    return (
      <Card>
        <div style={{ color: T.textMuted, fontSize: 13 }}>
          Loading settings from supervisor…
        </div>
      </Card>
    );
  }

  const updateField = <K extends keyof EffectiveConfig>(
    key: K,
    value: EffectiveConfig[K]
  ) => {
    setForm((prev) => (prev ? { ...prev, [key]: value } : prev));
  };

  return (
    <Card>
      <div
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          marginBottom: 16,
        }}
      >
        <div>
          <h2
            style={{
              margin: 0,
              fontSize: 14,
              fontWeight: 700,
              color: T.text,
            }}
          >
            Settings
          </h2>
          <div style={{ fontSize: 11, color: T.textMuted, marginTop: 4 }}>
            Stored at <code style={code}>~/.byne/pr-resolver-config.json</code>{" "}
            on save. Cargo commands + access lists stay in <code style={code}>config.yml</code>.
            {overrideCount > 0 && (
              <>
                {" · "}
                <span style={{ color: T.warn }}>
                  {overrideCount} override{overrideCount === 1 ? "" : "s"} active
                </span>
              </>
            )}
          </div>
        </div>
        <div style={{ display: "flex", gap: 8 }}>
          <button
            type="button"
            onClick={onReset}
            disabled={overrideCount === 0}
            style={{
              padding: "6px 12px",
              borderRadius: 6,
              border: `1px solid ${T.border}`,
              background: T.bg,
              color: overrideCount === 0 ? T.textSubtle : T.text,
              cursor: overrideCount === 0 ? "not-allowed" : "pointer",
              fontSize: 12,
              fontWeight: 500,
            }}
          >
            Reset to config.yml
          </button>
          <button
            type="button"
            onClick={() => onSave(toOverlay(form))}
            disabled={!isDirty || running}
            title={
              running
                ? "Wait for the current cycle to finish before saving"
                : !isDirty
                  ? "No unsaved changes"
                  : "Save & restart the service"
            }
            style={{
              padding: "6px 16px",
              borderRadius: 6,
              border: "none",
              background: isDirty && !running ? T.accent : T.border,
              color: "#fff",
              cursor: isDirty && !running ? "pointer" : "not-allowed",
              fontSize: 12,
              fontWeight: 600,
            }}
          >
            {running ? "Polling…" : isDirty ? "Save changes" : "Saved"}
          </button>
        </div>
      </div>

      <div
        style={{
          display: "grid",
          gridTemplateColumns: "repeat(2, minmax(0, 1fr))",
          gap: "12px 20px",
        }}
      >
        <Field label={FIELD_LABELS.githubRepo}>
          <input
            type="text"
            placeholder="juspay/hyperswitch-prism"
            value={form.githubRepo}
            onChange={(e) => updateField("githubRepo", e.target.value)}
            style={inputStyle}
          />
        </Field>
        <Field label={FIELD_LABELS.trigger}>
          <input
            type="text"
            placeholder="@HS-prism-bot"
            value={form.trigger}
            onChange={(e) => updateField("trigger", e.target.value)}
            style={inputStyle}
          />
        </Field>
        <Field label={FIELD_LABELS.pollInterval}>
          <NumberInput
            value={form.pollInterval}
            onChange={(v) => updateField("pollInterval", v)}
            min={5}
          />
        </Field>
        <Field label={FIELD_LABELS.maxConcurrent}>
          <NumberInput
            value={form.maxConcurrent}
            onChange={(v) => updateField("maxConcurrent", v)}
            min={1}
            max={10}
          />
        </Field>
        <Field label={FIELD_LABELS.maxBuildLoops}>
          <NumberInput
            value={form.maxBuildLoops}
            onChange={(v) => updateField("maxBuildLoops", v)}
            min={1}
            max={10}
          />
        </Field>
        <Field label={FIELD_LABELS.maxCommentsPerCycle}>
          <NumberInput
            value={form.maxCommentsPerCycle}
            onChange={(v) => updateField("maxCommentsPerCycle", v)}
            min={1}
            max={500}
          />
        </Field>
      </div>
    </Card>
  );
}

function Card({ children }: { children: React.ReactNode }) {
  return (
    <section
      style={{
        background: T.bgElev,
        border: `1px solid ${T.border}`,
        borderRadius: 10,
        padding: 20,
        margin: "16px 32px",
        boxShadow: T.shadow,
      }}
    >
      {children}
    </section>
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
    <label
      style={{
        display: "flex",
        flexDirection: "column",
        gap: 4,
        fontSize: 11,
        color: T.textMuted,
        fontWeight: 500,
      }}
    >
      <span>{label}</span>
      {children}
    </label>
  );
}

const inputStyle: React.CSSProperties = {
  padding: "8px 10px",
  borderRadius: 6,
  border: `1px solid ${T.border}`,
  background: T.bg,
  color: T.text,
  fontSize: 13,
  fontFamily:
    "system-ui, -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif",
  outline: "none",
};

function NumberInput({
  value,
  onChange,
  min,
  max,
}: {
  value: number;
  onChange: (n: number) => void;
  min?: number;
  max?: number;
}) {
  return (
    <input
      type="number"
      value={Number.isFinite(value) ? value : 0}
      min={min}
      max={max}
      onChange={(e) => {
        const n = Number(e.target.value);
        if (Number.isFinite(n)) onChange(n);
      }}
      style={inputStyle}
    />
  );
}

/**
 * Promote the editable form into a runtime overlay payload. We always send
 * the full set so the supervisor doesn't have to merge against a stale copy.
 * Empty strings become "clear this override" semantically.
 */
function toOverlay(form: EffectiveConfig): RuntimeOverlay {
  return {
    enabled: form.enabled,
    autoApprove: form.autoApprove,
    githubRepo: form.githubRepo.trim(),
    trigger: form.trigger.trim(),
    pollInterval: form.pollInterval,
    maxConcurrent: form.maxConcurrent,
    maxBuildLoops: form.maxBuildLoops,
    maxCommentsPerCycle: form.maxCommentsPerCycle,
  };
}

const code: React.CSSProperties = {
  background: T.codeBg,
  padding: "1px 5px",
  borderRadius: 3,
  fontFamily: "monospace",
  fontSize: 10,
};
