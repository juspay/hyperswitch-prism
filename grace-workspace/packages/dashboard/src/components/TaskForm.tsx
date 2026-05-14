import { useState } from "react";
import { T } from "../theme";

export interface SubmittedAttachment {
  name: string;
  mimeType: string;
  text?: string;
  dataBase64?: string;
  size: number;
}

export interface SubmittedTask {
  title: string;
  description: string;
  acceptanceCriteria: string[];
  connectorDocUrls?: string[];
  targetFiles?: string[];
  projectRoot?: string;
  attachments?: SubmittedAttachment[];
  /** AI runner to use for this task */
  runner?: "opencode" | "claude-code";
  /** Optional model override for the selected runner */
  runnerModel?: string;
  /** Grace/10XGRACE workflow: Payment method to implement (e.g., "Card", "Wallet", "BankTransfer") */
  paymentMethod?: string;
  /** Grace/10XGRACE workflow: Target connector names to implement for */
  targetConnectors?: string[];
  /** Grace/10XGRACE workflow: Payment method category */
  paymentMethodCategory?:
    | "card"
    | "wallet"
    | "bank_transfer"
    | "bank_debit"
    | "bnpl"
    | "crypto"
    | "voucher"
    | "gift_card"
    | "pay_later"
    | string;
  /** Grace/10XGRACE workflow: Priority classification */
  priority?: "critical" | "high" | "medium" | "low";
  /** Grace/10XGRACE workflow: Connector documentation URLs */
  connectorDocs?: Array<{
    connector: string;
    urls: Array<{
      title: string;
      url: string;
      type:
        | "api_reference"
        | "payment_method_guide"
        | "authentication_guide"
        | "webhooks_guide"
        | "testing_guide"
        | "error_reference";
      verified?: boolean;
    }>;
  }>;
  /** Grace/10XGRACE workflow: Implementation prerequisites */
  prerequisites?: string[];
  /** Grace/10XGRACE workflow: Estimated complexity */
  estimatedComplexity?: "low" | "medium" | "high";
}

const TEXT_LIKE = /^(text\/|application\/(json|xml|yaml|x-yaml|sql|javascript|typescript|x-sh)$|application\/.*\+(json|xml|yaml)$)/;
const MAX_ATTACHMENT_SIZE = 5 * 1024 * 1024; // 5 MB per file

async function readFileAsAttachment(file: File): Promise<SubmittedAttachment> {
  const mimeType = file.type || "application/octet-stream";
  const isText =
    TEXT_LIKE.test(mimeType) ||
    /\.(md|txt|json|yaml|yml|res|tsx?|jsx?|css|scss|html|csv|sql|py|rb|go|rs|sh)$/i.test(
      file.name
    );
  if (isText) {
    const text = await file.text();
    return { name: file.name, mimeType, text, size: file.size };
  }
  const buf = await file.arrayBuffer();
  // browser btoa needs binary string
  let binary = "";
  const bytes = new Uint8Array(buf);
  for (let i = 0; i < bytes.length; i++) binary += String.fromCharCode(bytes[i]!);
  return {
    name: file.name,
    mimeType,
    dataBase64: btoa(binary),
    size: file.size,
  };
}

export function TaskForm({
  onSubmit,
  disabled,
  wsConnected,
}: {
  onSubmit: (task: SubmittedTask) => void;
  disabled?: boolean;
  wsConnected?: boolean;
}) {
  const [title, setTitle] = useState("");
  const [description, setDescription] = useState("");
  const [criteria, setCriteria] = useState("");
  const [advanced, setAdvanced] = useState(false);
  const [connectorDocUrls, setConnectorDocUrls] = useState("");
  const [targetFiles, setTargetFiles] = useState("");
  const [projectRoot, setProjectRoot] = useState("");
  const [attachments, setAttachments] = useState<SubmittedAttachment[]>([]);

  // AI Runner selection
  const [runner, setRunner] = useState<"opencode" | "claude-code">("opencode");
  const [runnerModel, setRunnerModel] = useState("");

  // Grace/10XGRACE workflow: Payment method fields
  const [paymentMethod, setPaymentMethod] = useState("");
  const [targetConnectors, setTargetConnectors] = useState("");
  const [paymentMethodCategory, setPaymentMethodCategory] = useState<
    | "card"
    | "wallet"
    | "bank_transfer"
    | "bank_debit"
    | "bnpl"
    | "crypto"
    | "voucher"
    | "gift_card"
    | "pay_later"
    | ""
  >("");
  const [priority, setPriority] = useState<
    "critical" | "high" | "medium" | "low" | ""
  >("");
  const [prerequisites, setPrerequisites] = useState("");
  const [estimatedComplexity, setEstimatedComplexity] = useState<
    "low" | "medium" | "high" | ""
  >("");

  const [error, setError] = useState<string | null>(null);

  const onFilesPicked = async (fileList: FileList | null) => {
    if (!fileList || fileList.length === 0) return;
    setError(null);
    const next: SubmittedAttachment[] = [];
    for (const f of Array.from(fileList)) {
      if (f.size > MAX_ATTACHMENT_SIZE) {
        setError(
          `${f.name} is ${(f.size / 1024 / 1024).toFixed(1)}MB — max ${MAX_ATTACHMENT_SIZE / 1024 / 1024}MB per file`
        );
        continue;
      }
      try {
        next.push(await readFileAsAttachment(f));
      } catch (err) {
        setError(
          `failed to read ${f.name}: ${err instanceof Error ? err.message : String(err)}`
        );
      }
    }
    if (next.length > 0) setAttachments((prev) => [...prev, ...next]);
  };

  const removeAttachment = (idx: number) => {
    setAttachments((prev) => prev.filter((_, i) => i !== idx));
  };

  const criteriaList = criteria
    .split("\n")
    .map((l) => l.trim())
    .filter(Boolean);

  const canSubmit = title.trim().length > 0 && !disabled;

  const submit = () => {
    if (!canSubmit) {
      setError("Task title is required");
      return;
    }
    setError(null);

    onSubmit({
      title: title.trim(),
      description: description.trim(),
      acceptanceCriteria: criteriaList,
      connectorDocUrls: connectorDocUrls
        ? connectorDocUrls.split("\n").map((s) => s.trim()).filter(Boolean)
        : undefined,
      targetFiles: targetFiles
        ? targetFiles.split(",").map((s) => s.trim()).filter(Boolean)
        : undefined,
      projectRoot: projectRoot.trim() || undefined,
      attachments: attachments.length > 0 ? attachments : undefined,
      // AI Runner fields
      runner,
      runnerModel: runnerModel.trim() || undefined,
      // Grace/10XGRACE workflow fields
      paymentMethod: paymentMethod.trim() || undefined,
      targetConnectors: targetConnectors
        ? targetConnectors.split(",").map((s) => s.trim()).filter(Boolean)
        : undefined,
      paymentMethodCategory: paymentMethodCategory || undefined,
      priority: priority || undefined,
      prerequisites: prerequisites
        ? prerequisites.split(",").map((s) => s.trim()).filter(Boolean)
        : undefined,
      estimatedComplexity: estimatedComplexity || undefined,
    });
  };

  const field: React.CSSProperties = {
    width: "100%",
    padding: "8px 10px",
    background: T.bgElev,
    border: `1px solid ${T.border}`,
    borderRadius: 6,
    color: T.text,
    fontSize: 13,
    fontFamily: "inherit",
    outline: "none",
    boxSizing: "border-box",
  };
  const label: React.CSSProperties = {
    display: "block",
    fontSize: 11,
    color: T.textMuted,
    fontWeight: 600,
    marginBottom: 4,
  };

  return (
    <div
      style={{
        maxWidth: 600,
        background: T.bgElev,
        border: `1px solid ${T.border}`,
        borderRadius: 12,
        padding: 22,
        boxShadow: T.shadow,
      }}
    >
      {!wsConnected && (
        <div
          style={{
            background: T.warnSoft,
            color: T.warn,
            padding: "8px 10px",
            borderRadius: 6,
            fontSize: 12,
            marginBottom: 12,
          }}
        >
          WebSocket not connected. Start the engine with{" "}
          <code>node packages/cli/dist/index.js run --task-from-ui</code> before
          submitting.
        </div>
      )}

      {/* AI Runner Selection */}
      <div style={{ marginBottom: 16, paddingBottom: 16, borderBottom: `1px solid ${T.border}` }}>
        <label style={{...label, fontSize: 12, color: T.accent}}>AI Runner</label>
        <div style={{ display: "flex", gap: 12, marginTop: 8 }}>
          <button
            type="button"
            onClick={() => setRunner("opencode")}
            disabled={disabled}
            style={{
              flex: 1,
              padding: "12px 16px",
              borderRadius: 8,
              border: `2px solid ${runner === "opencode" ? T.accent : T.border}`,
              background: runner === "opencode" ? T.accentSoft : T.bg,
              cursor: disabled ? "not-allowed" : "pointer",
              display: "flex",
              flexDirection: "column",
              alignItems: "center",
              gap: 4,
              opacity: disabled ? 0.6 : 1,
            }}
          >
            <span style={{ fontWeight: 600, color: T.text }}>OpenCode</span>
            <span style={{ fontSize: 11, color: T.textMuted }}>Local LLM gateway</span>
          </button>
          <button
            type="button"
            onClick={() => setRunner("claude-code")}
            disabled={disabled}
            style={{
              flex: 1,
              padding: "12px 16px",
              borderRadius: 8,
              border: `2px solid ${runner === "claude-code" ? T.accent : T.border}`,
              background: runner === "claude-code" ? T.accentSoft : T.bg,
              cursor: disabled ? "not-allowed" : "pointer",
              display: "flex",
              flexDirection: "column",
              alignItems: "center",
              gap: 4,
              opacity: disabled ? 0.6 : 1,
            }}
          >
            <span style={{ fontWeight: 600, color: T.text }}>Claude Code</span>
            <span style={{ fontSize: 11, color: T.textMuted }}>Anthropic&apos;s CLI</span>
          </button>
        </div>

        {/* Runner-specific options */}
        {runner === "claude-code" && (
          <div style={{ marginTop: 12 }}>
            <label style={label}>Model (optional)</label>
            <input
              style={field}
              value={runnerModel}
              onChange={(e) => setRunnerModel(e.target.value)}
              placeholder="claude-sonnet-4-6"
              disabled={disabled}
            />
          </div>
        )}
      </div>

      <div style={{ marginBottom: 10 }}>
        <label style={label}>Title</label>
        <input
          style={field}
          value={title}
          onChange={(e) => setTitle(e.target.value)}
          placeholder="Implement ApplePay for Stripe and Adyen"
          disabled={disabled}
        />
      </div>

      <div style={{ marginBottom: 10 }}>
        <label style={label}>Description</label>
        <textarea
          style={{ ...field, minHeight: 70, resize: "vertical" }}
          value={description}
          onChange={(e) => setDescription(e.target.value)}
          placeholder="Describe the payment method implementation requirements..."
          disabled={disabled}
        />
      </div>

      {/* Grace/10XGRACE workflow: Core payment method fields */}
      <div style={{ marginBottom: 10 }}>
        <label style={label}>Payment Method (e.g., Card, Wallet, BankTransfer)</label>
        <input
          style={field}
          value={paymentMethod}
          onChange={(e) => setPaymentMethod(e.target.value)}
          placeholder="ApplePay, BankDebit, GooglePay, etc."
          disabled={disabled}
        />
        {/* Warning for potential misclassification */}
        {paymentMethod && ["BankDebit", "Wallet", "PayLater", "Card", "BankTransfer", "Crypto"].includes(paymentMethod.trim()) && !targetConnectors && (
          <div
            style={{
              marginTop: 6,
              padding: "6px 10px",
              background: T.warnSoft,
              border: `1px solid ${T.warn}`,
              borderRadius: 4,
              fontSize: 11,
              color: T.warn,
            }}
          >
            <strong>Note:</strong> &quot;{paymentMethod.trim()}&quot; is typically a payment method, not a standalone flow.
            This will add the payment method to existing connector flows.
          </div>
        )}
      </div>

      <div style={{ marginBottom: 10 }}>
        <label style={label}>Target Connectors (comma-separated)</label>
        <input
          style={field}
          value={targetConnectors}
          onChange={(e) => setTargetConnectors(e.target.value)}
          placeholder="Stripe, Adyen, Checkout"
          disabled={disabled}
        />
      </div>

      <div style={{ marginBottom: 10 }}>
        <label style={label}>Acceptance criteria (one per line, optional)</label>
        <textarea
          style={{ ...field, minHeight: 60, resize: "vertical" }}
          value={criteria}
          onChange={(e) => setCriteria(e.target.value)}
          placeholder={"ApplePay works on Stripe\nApplePay works on Adyen\nError handling follows existing patterns"}
          disabled={disabled}
        />
      </div>

      <div style={{ marginBottom: 10 }}>
        <label style={label}>Attachments (specs, screenshots, connector docs — optional)</label>
        <input
          type="file"
          multiple
          onChange={(e) => {
            void onFilesPicked(e.target.files);
            // allow re-uploading the same file later by clearing the input
            e.target.value = "";
          }}
          disabled={disabled}
          style={{
            fontSize: 12,
            color: T.textMuted,
            cursor: disabled ? "not-allowed" : "pointer",
          }}
        />
        {attachments.length > 0 && (
          <ul
            style={{
              listStyle: "none",
              padding: 0,
              margin: "8px 0 0",
              display: "flex",
              flexDirection: "column",
              gap: 4,
            }}
          >
            {attachments.map((a, i) => (
              <li
                key={`${a.name}-${i}`}
                style={{
                  display: "flex",
                  alignItems: "center",
                  gap: 8,
                  padding: "5px 8px",
                  background: T.codeBg,
                  border: `1px solid ${T.border}`,
                  borderRadius: 5,
                  fontSize: 12,
                  color: T.text,
                }}
              >
                <span style={{ flex: 1, minWidth: 0, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                  {a.name}
                </span>
                <span style={{ color: T.textSubtle, fontSize: 11 }}>
                  {a.text !== undefined ? "text" : "binary"} · {(a.size / 1024).toFixed(1)} KB
                </span>
                <button
                  type="button"
                  onClick={() => removeAttachment(i)}
                  disabled={disabled}
                  style={{
                    background: "none",
                    border: "none",
                    color: T.textMuted,
                    cursor: "pointer",
                    fontSize: 14,
                    padding: 0,
                    lineHeight: 1,
                  }}
                  title="Remove"
                >
                  ×
                </button>
              </li>
            ))}
          </ul>
        )}
      </div>

      <button
        onClick={() => setAdvanced((v) => !v)}
        style={{
          background: "none",
          border: "none",
          color: T.accent,
          fontSize: 12,
          padding: 0,
          marginBottom: advanced ? 10 : 0,
          cursor: "pointer",
        }}
      >
        {advanced ? "− Hide advanced" : "+ Advanced options"}
      </button>

      {advanced && (
        <>
          <div style={{ marginBottom: 10 }}>
            <label style={label}>Connector Reference Document URLs (one per line)</label>
            <textarea
              style={{ ...field, minHeight: 80 }}
              value={connectorDocUrls}
              onChange={(e) => setConnectorDocUrls(e.target.value)}
              placeholder="https://docs.stripe.com/api/payment_intents&#10;https://docs.stripe.com/guides/payments"
              disabled={disabled}
            />
          </div>
          <div style={{ marginBottom: 10 }}>
            <label style={label}>Target files (comma-separated)</label>
            <input
              style={field}
              value={targetFiles}
              onChange={(e) => setTargetFiles(e.target.value)}
              placeholder="crates/integrations/connector-integration/src/connectors/stripe.rs"
              disabled={disabled}
            />
          </div>
          <div style={{ marginBottom: 10 }}>
            <label style={label}>Project root</label>
            <input
              style={field}
              value={projectRoot}
              onChange={(e) => setProjectRoot(e.target.value)}
              placeholder="../hyperswitch-prism"
              disabled={disabled}
            />
          </div>

          {/* Grace/10XGRACE workflow: Additional fields */}
          <div style={{ marginBottom: 10, paddingTop: 10, borderTop: `1px dashed ${T.border}` }}>
            <label style={{ ...label, color: T.accent }}>Payment Method Category</label>
            <select
              style={field}
              value={paymentMethodCategory}
              onChange={(e) =>
                setPaymentMethodCategory(
                  e.target.value as
                    | "card"
                    | "wallet"
                    | "bank_transfer"
                    | "bank_debit"
                    | "bnpl"
                    | "crypto"
                    | "voucher"
                    | "gift_card"
                    | "pay_later"
                    | ""
                )
              }
              disabled={disabled}
            >
              <option value="">Select category…</option>
              <option value="card">Card</option>
              <option value="wallet">Wallet</option>
              <option value="bank_transfer">Bank Transfer</option>
              <option value="bank_debit">Bank Debit</option>
              <option value="bnpl">Buy Now Pay Later</option>
              <option value="crypto">Crypto</option>
              <option value="voucher">Voucher</option>
              <option value="gift_card">Gift Card</option>
              <option value="pay_later">Pay Later</option>
            </select>
          </div>

          <div style={{ marginBottom: 10 }}>
            <label style={label}>Priority</label>
            <select
              style={field}
              value={priority}
              onChange={(e) =>
                setPriority(e.target.value as "critical" | "high" | "medium" | "low" | "")
              }
              disabled={disabled}
            >
              <option value="">Select priority…</option>
              <option value="critical">Critical</option>
              <option value="high">High</option>
              <option value="medium">Medium</option>
              <option value="low">Low</option>
            </select>
          </div>

          <div style={{ marginBottom: 10 }}>
            <label style={label}>Prerequisites (comma-separated)</label>
            <input
              style={field}
              value={prerequisites}
              onChange={(e) => setPrerequisites(e.target.value)}
              placeholder="PR-123, connector-auth-update, type-definitions"
              disabled={disabled}
            />
          </div>

          <div style={{ marginBottom: 10 }}>
            <label style={label}>Estimated Complexity</label>
            <select
              style={field}
              value={estimatedComplexity}
              onChange={(e) =>
                setEstimatedComplexity(e.target.value as "low" | "medium" | "high" | "")
              }
              disabled={disabled}
            >
              <option value="">Select complexity…</option>
              <option value="low">Low</option>
              <option value="medium">Medium</option>
              <option value="high">High</option>
            </select>
          </div>
        </>
      )}

      {error && (
        <div
          style={{
            marginTop: 8,
            padding: "8px 10px",
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
          display: "flex",
          justifyContent: "flex-end",
          marginTop: 16,
          paddingTop: 14,
          borderTop: `1px solid ${T.border}`,
        }}
      >
        <button
          onClick={submit}
          disabled={!canSubmit}
          style={{
            padding: "9px 22px",
            background: canSubmit ? T.accent : T.border,
            color: canSubmit ? "#fff" : T.textSubtle,
            border: "none",
            borderRadius: 6,
            fontSize: 13,
            fontWeight: 600,
            cursor: canSubmit ? "pointer" : "not-allowed",
          }}
        >
          Submit task →
        </button>
      </div>
    </div>
  );
}
