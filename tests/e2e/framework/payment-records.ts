import pg from "pg";
import { configuration } from "./configuration.js";

export async function readPaymentRoute(
  merchantId: string,
  paymentId: string,
  connector: string,
  attemptStatus: string,
): Promise<{ gateway: string; processorReferenceRecorded: boolean }> {
  const client = new pg.Client({
    connectionString: configuration.databaseUrl,
    connectionTimeoutMillis: 10_000,
  });
  await client.connect();
  try {
    const result = await client.query(
      `SELECT p.feature_metadata->>'gateway_system' AS gateway,
        EXISTS (
          SELECT 1 FROM payment_attempt a
          WHERE a.merchant_id = p.merchant_id
            AND a.payment_id = p.payment_id
            AND a.connector = $3
            AND a.status::text = $4
            AND a.connector_transaction_id IS NOT NULL
            AND a.connector_transaction_id <> ''
        ) AS processor_reference_recorded
       FROM payment_intent p
       WHERE p.merchant_id = $1 AND p.payment_id = $2`,
      [merchantId, paymentId, connector, attemptStatus],
    );
    return {
      gateway: result.rows[0]?.gateway,
      processorReferenceRecorded:
        result.rows[0]?.processor_reference_recorded === true,
    };
  } finally {
    await client.end();
  }
}
