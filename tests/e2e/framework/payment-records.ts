import pg from "pg";
import { configuration } from "./configuration.js";

export type RecordedPayment = {
  gateway: string | null;
  attemptStatus: string;
  connectorTransactionIdPresent: boolean;
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
           AND a.connector_transaction_id <> '') AS "connectorTransactionIdPresent"
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
