import pg from "pg";
import { configuration } from "./configuration.js";

export type RecordedPayment = {
  gateway: string | null;
  attemptStatus: string;
  connectorTransactionIdPresent: boolean;
  customerRecordExists: boolean;
};

export type RecordedMandate = {
  status: string;
  type: string;
  connector: string;
  connectorMandateIdPresent: boolean;
  paymentMethodIdPresent: boolean;
};

export type RecordedNetworkTransactionReference = {
  networkTransactionId: string;
  transactionLinkId: string | null;
};

export async function readRecordedPayment(
  merchantId: string,
  paymentId: string,
): Promise<RecordedPayment> {
  const client = new pg.Client({
    connectionString: configuration.databaseUrl,
    connectionTimeoutMillis: 10_000,
    options: "-c default_transaction_read_only=on",
  });
  await client.connect();
  try {
    const result = await client.query<RecordedPayment>(
      `SELECT
         p.feature_metadata->>'gateway_system' AS gateway,
         a.status::text AS "attemptStatus",
         (a.connector_transaction_id IS NOT NULL
           AND a.connector_transaction_id <> '') AS "connectorTransactionIdPresent",
         EXISTS (
           SELECT 1 FROM customers c
           WHERE c.merchant_id = p.merchant_id AND c.customer_id = p.customer_id
         ) AS "customerRecordExists"
       FROM payment_intent p
       JOIN payment_attempt a
         ON a.merchant_id = p.merchant_id
        AND a.payment_id = p.payment_id
        AND a.attempt_id = p.active_attempt_id
       WHERE p.merchant_id = $1 AND p.payment_id = $2`,
      [merchantId, paymentId],
    );
    if (result.rows.length !== 1) {
      throw new Error("Expected exactly one payment with an active attempt");
    }
    return result.rows[0];
  } finally {
    await client.end();
  }
}

export async function readRecordedMandate(
  merchantId: string,
  mandateId: string,
): Promise<RecordedMandate> {
  const client = new pg.Client({
    connectionString: configuration.databaseUrl,
    connectionTimeoutMillis: 10_000,
    options: "-c default_transaction_read_only=on",
  });
  await client.connect();
  try {
    const result = await client.query<RecordedMandate>(
      `SELECT
         mandate_status::text AS status,
         mandate_type::text AS type,
         connector,
         (connector_mandate_id IS NOT NULL
           AND connector_mandate_id <> '') AS "connectorMandateIdPresent",
         (payment_method_id IS NOT NULL
           AND payment_method_id <> '') AS "paymentMethodIdPresent"
       FROM mandate
       WHERE merchant_id = $1 AND mandate_id = $2`,
      [merchantId, mandateId],
    );
    if (result.rows.length !== 1) {
      throw new Error("Expected exactly one recorded mandate");
    }
    return result.rows[0];
  } finally {
    await client.end();
  }
}

export async function readRecordedNetworkTransactionReference(
  merchantId: string,
  paymentId: string,
): Promise<RecordedNetworkTransactionReference> {
  const client = new pg.Client({
    connectionString: configuration.databaseUrl,
    connectionTimeoutMillis: 10_000,
    options: "-c default_transaction_read_only=on",
  });
  await client.connect();
  try {
    const result = await client.query<RecordedNetworkTransactionReference>(
      `SELECT
         a.network_transaction_id AS "networkTransactionId",
         a.network_transaction_link_id AS "transactionLinkId"
       FROM payment_intent p
       JOIN payment_attempt a
         ON a.merchant_id = p.merchant_id
        AND a.payment_id = p.payment_id
        AND a.attempt_id = p.active_attempt_id
       WHERE p.merchant_id = $1 AND p.payment_id = $2
         AND a.network_transaction_id IS NOT NULL
         AND a.network_transaction_id <> ''`,
      [merchantId, paymentId],
    );
    if (result.rows.length !== 1) {
      throw new Error(
        "Expected exactly one active attempt with a network transaction reference",
      );
    }
    return result.rows[0];
  } finally {
    await client.end();
  }
}
