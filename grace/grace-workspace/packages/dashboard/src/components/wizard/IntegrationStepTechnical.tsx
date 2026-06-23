import { T } from "../../theme";
import {
  ALL_FLOWS,
  AUTH_LOCATIONS,
  AUTH_SCHEMES,
  CURRENCY_UNITS,
  PM_CATEGORIES,
  REGIONS,
  flowDependents,
  type AuthLocation,
  type AuthScheme,
  type CurrencyUnit,
  type PMCategory,
  type Region,
} from "./enums";
import { Field, Pill, RepeatingList, Section, Toggle, inputStyle } from "./shared";
import type { WizardAction, WizardState } from "./types";

export function IntegrationStepTechnical({
  state,
  dispatch,
}: {
  state: WizardState;
  dispatch: (a: WizardAction) => void;
}) {
  const set = (patch: Partial<WizardState>) => dispatch({ type: "set", patch });
  const selectedFlows = new Set(state.supportedFlows);

  return (
    <div>
      <Section title="Authentication">
        <Field label="Auth scheme" required>
          <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
            {AUTH_SCHEMES.map((scheme) => (
              <Pill
                key={scheme}
                active={state.authScheme === scheme}
                onClick={() => set({ authScheme: scheme as AuthScheme })}
              >
                {scheme}
              </Pill>
            ))}
          </div>
        </Field>
        {state.authScheme && (
          <Field
            label="Auth details"
            hint="Signing algorithm, header format, OAuth flow choice…"
          >
            <textarea
              value={state.authDetails}
              onChange={(e) => set({ authDetails: e.target.value })}
              placeholder="e.g. HMAC-SHA256 over canonical request, header X-Sig"
              rows={2}
              style={{ ...inputStyle, resize: "vertical", fontFamily: "inherit" }}
            />
          </Field>
        )}
        <Field label="Auth location" required>
          <div style={{ display: "flex", gap: 8 }}>
            {AUTH_LOCATIONS.map((loc) => (
              <Pill
                key={loc}
                active={state.authLocation === loc}
                onClick={() => set({ authLocation: loc as AuthLocation })}
              >
                {loc}
              </Pill>
            ))}
          </div>
        </Field>
        <Field
          label="Credential field names"
          hint="The keys in creds.json this connector needs (e.g. merchantKey, merchantSecret)."
        >
          <RepeatingList
            items={state.credentialFields}
            placeholder="e.g. merchantKey"
            onAdd={(text) => dispatch({ type: "addCredField", text })}
            onRemove={(i) => dispatch({ type: "removeCredField", index: i })}
          />
        </Field>
      </Section>

      <Section
        title="Amount & Endpoints"
        description="Currency unit is load-bearing for Rust codegen — wrong choice breaks every TryFrom in transformers.rs."
      >
        <Field label="Currency unit" required>
          <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
            {CURRENCY_UNITS.map((u) => (
              <Pill
                key={u}
                active={state.currencyUnit === u}
                onClick={() => set({ currencyUnit: u as CurrencyUnit })}
              >
                {u}
              </Pill>
            ))}
          </div>
        </Field>
        <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 12 }}>
          <Field label="Production base URL">
            <input
              value={state.baseUrl}
              onChange={(e) => set({ baseUrl: e.target.value })}
              placeholder="https://api.connector.com"
              style={inputStyle}
            />
          </Field>
          <Field label="Sandbox URL">
            <input
              value={state.sandboxUrl}
              onChange={(e) => set({ sandboxUrl: e.target.value })}
              placeholder="https://sandbox.connector.com"
              style={inputStyle}
            />
          </Field>
        </div>
      </Section>

      <Section
        title="Feature Toggles"
        description="Enabling these locks the corresponding flows on."
      >
        <Toggle
          checked={state.supports3DS}
          onChange={(v) => dispatch({ type: "setToggle", key: "supports3DS", value: v })}
          label="Supports 3D Secure"
          hint="Selects the 3DS-aware Card pattern instead of Standard JSON."
        />
        <Toggle
          checked={state.supportsWebhooks}
          onChange={(v) =>
            dispatch({ type: "setToggle", key: "supportsWebhooks", value: v })
          }
          label="Supports webhooks"
          hint="Forces IncomingWebhook into supported flows."
        />
        <Toggle
          checked={state.supportsRecurring}
          onChange={(v) =>
            dispatch({ type: "setToggle", key: "supportsRecurring", value: v })
          }
          label="Supports recurring / MIT"
          hint="Forces SetupMandate + RepeatPayment."
        />
      </Section>

      <Section
        title="Supported Flows"
        description="Authorize is required. Prerequisites auto-lock (e.g. Refund→Capture)."
      >
        <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
          {ALL_FLOWS.map((flow) => {
            const required = flow === "Authorize";
            const dependents = flowDependents(flow, selectedFlows);
            const lockedByDependents = dependents.length > 0;
            const active = selectedFlows.has(flow);
            return (
              <Pill
                key={flow}
                active={active}
                disabled={required || lockedByDependents}
                title={
                  required
                    ? "Authorize is always required"
                    : lockedByDependents
                    ? `Required by ${dependents.join(", ")}`
                    : undefined
                }
                onClick={() => {
                  if (required || lockedByDependents) return;
                  dispatch({ type: "toggleFlow", flow });
                }}
              >
                {flow}
                {(required || lockedByDependents) && (
                  <span style={{ marginLeft: 4, opacity: 0.7 }}>🔒</span>
                )}
              </Pill>
            );
          })}
        </div>
      </Section>

      <Section
        title="Supported Payment Methods"
        description="At least one required. Pick a primary at the bottom — it drives L2 search queries."
      >
        {(Object.keys(PM_CATEGORIES) as PMCategory[]).map((cat) => (
          <div key={cat} style={{ marginBottom: 14 }}>
            <div
              style={{
                fontSize: 12,
                fontWeight: 700,
                color: T.textMuted,
                textTransform: "uppercase",
                letterSpacing: 0.5,
                marginBottom: 6,
              }}
            >
              {cat}
            </div>
            <div style={{ display: "flex", gap: 6, flexWrap: "wrap" }}>
              {PM_CATEGORIES[cat].map((method) => {
                const selected = (state.selectedPMs[cat] ?? []).includes(method);
                return (
                  <Pill
                    key={method}
                    active={selected}
                    onClick={() => dispatch({ type: "togglePM", category: cat, method })}
                  >
                    {method}
                  </Pill>
                );
              })}
            </div>
          </div>
        ))}
        <Field label="Primary payment method" required>
          <select
            value={state.primaryPaymentMethod}
            onChange={(e) => set({ primaryPaymentMethod: e.target.value })}
            style={inputStyle}
          >
            <option value="">— select —</option>
            {Object.entries(state.selectedPMs).flatMap(([c, ms]) =>
              ms.map((m) => (
                <option key={`${c}:${m}`} value={`${c}:${m}`}>
                  {c} / {m}
                </option>
              )),
            )}
          </select>
        </Field>
      </Section>

      <Section title="Optional Context">
        <Field label="Webhook URL pattern">
          <input
            value={state.webhookUrlPattern}
            onChange={(e) => set({ webhookUrlPattern: e.target.value })}
            placeholder="POST {baseUrl}/webhooks/{event}"
            style={inputStyle}
          />
        </Field>
        <Field label="Regions">
          <div style={{ display: "flex", gap: 6, flexWrap: "wrap" }}>
            {REGIONS.map((r) => {
              const active = state.regions.includes(r);
              return (
                <Pill
                  key={r}
                  active={active}
                  onClick={() =>
                    set({
                      regions: active
                        ? state.regions.filter((x) => x !== r)
                        : [...state.regions, r as Region],
                    })
                  }
                >
                  {r}
                </Pill>
              );
            })}
          </div>
        </Field>
        <Field label="Supported currencies" hint="Comma-separated ISO codes (e.g. USD, EUR, GBP)">
          <input
            value={state.supportedCurrencies}
            onChange={(e) => set({ supportedCurrencies: e.target.value })}
            placeholder="USD, EUR, GBP"
            style={inputStyle}
          />
        </Field>
        <Field label="Sandbox credentials hint" hint="Format / where to obtain test keys.">
          <textarea
            value={state.sandboxCredentialsHint}
            onChange={(e) => set({ sandboxCredentialsHint: e.target.value })}
            placeholder="Test mode uses sk_test_ prefix; obtainable at dashboard.connector.com"
            rows={2}
            style={{ ...inputStyle, resize: "vertical", fontFamily: "inherit" }}
          />
        </Field>
      </Section>
    </div>
  );
}
