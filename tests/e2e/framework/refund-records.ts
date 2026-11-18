import pg from "pg";
import { configuration } from "./configuration.js";

export type RecordedRefund = {
  status: string;
  connectorRefundIdPresent: boolean;
  paymentIdMatches: boolean;
};

export async function readRecordedRefund(
  merchantId: string,
  refundId: string,
  paymentId: string,
): Promise<RecordedRefund> {
  const client = new pg.Client({
    connectionString: configuration.databaseUrl,
    connectionTimeoutMillis: 10_000,
    options: "-c default_transaction_read_only=on",
  });
  await client.connect();
  try {
    const result = await client.query<RecordedRefund>(
      `SELECT
         refund_status::text AS status,
         (connector_refund_id IS NOT NULL
           AND connector_refund_id <> '') AS "connectorRefundIdPresent",
         payment_id = $3 AS "paymentIdMatches"
       FROM refund
       WHERE merchant_id = $1 AND refund_id = $2`,
      [merchantId, refundId, paymentId],
    );
    if (result.rows.length !== 1) {
      throw new Error("Expected exactly one recorded refund");
    }
    return result.rows[0];
  } finally {
    await client.end();
  }
}
