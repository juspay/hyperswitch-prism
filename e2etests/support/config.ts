import { builtinOverrides } from "../otp/overrides.js";
import { readFileSync } from "node:fs";

export type Json = Record<string, any>;
export type Connector =
  | "adyen"
  | "stripe"
  | "paypal"
  | "authorizedotnet"
  | "cybersource"
  | "checkout";
export type Merchant = { org: string; merchant: string; connector: Connector };
export type Overrides = {
  credential?: string;
  merchant?: Json;
  profile?: Json;
  connector?: Json;
  payment?: Json;
  paymentVariants?: Record<string, Json>;
};

export function merge<T extends Json>(base: T, patch: Json = {}): T {
  const result: Json = structuredClone(base);
  for (const [key, value] of Object.entries(patch)) {
    if (["__proto__", "constructor", "prototype"].includes(key))
      throw new Error("Invalid override key");
    result[key] =
      value && typeof value === "object" && !Array.isArray(value)
        ? merge(
            result[key] && typeof result[key] === "object" ? result[key] : {},
            value,
          )
        : structuredClone(value);
  }
  return result as T;
}

export function credentials(
  document: Json,
  connector: string,
  selected?: string,
): Json {
  const entry = document[connector];
  const value = selected
    ? entry?.[selected]
    : entry?.connector_account_details
      ? entry
      : Object.values(entry ?? {}).find(
          (item: any) => item?.connector_account_details,
        );
  if (!value?.connector_account_details) {
    throw new Error(
      `Missing HS sandbox credentials for ${connector}${selected ? "/" + selected : ""}`,
    );
  }
  return structuredClone(value);
}

export function loadCredentials(connector: Connector, selected?: string): Json {
  const path =
    process.env.CONNECTOR_AUTH_FILE_PATH ??
    process.env.CYPRESS_CONNECTOR_AUTH_FILE_PATH;
  if (!path)
    throw new Error(
      "Set CONNECTOR_AUTH_FILE_PATH to the decrypted HS main creds.json",
    );
  return credentials(
    JSON.parse(readFileSync(path, "utf8")),
    connector,
    selected,
  );
}

export function loadOverrides(merchant: Merchant): Overrides {
  const path = process.env.E2E_OVERRIDES_FILE;
  const overrides = path ? JSON.parse(readFileSync(path, "utf8")) : {};
  const id = `${merchant.org}/${merchant.merchant}/${merchant.connector}`;
  return merge(
    merge(
      builtinOverrides[id] ?? {},
      overrides.connectors?.[merchant.connector] ?? {},
    ),
    overrides.merchants?.[id] ?? {},
  );
}

export function assertOtp(body: Json): void {
  if (
    body.authentication_type !== "no_three_ds" ||
    body.payment_method !== "card" ||
    body.off_session === true ||
    body.setup_future_usage != null ||
    body.customer_acceptance != null ||
    body.mandate_data != null ||
    body.mandate_id != null ||
    body.recurring_details != null ||
    !body.payment_method_data?.card ||
    !Number.isSafeInteger(body.amount) ||
    body.amount <= 0
  ) {
    throw new Error(
      "OTP requires a positive amount, fresh card, No3DS, and no mandate/setup fields",
    );
  }
}
