export const configuration = {
  hyperswitchBaseUrl: process.env.HS_BASE_URL ?? "http://127.0.0.1:8080",
  adminApiKey: process.env.HS_ADMIN_API_KEY ?? "test_admin",
  databaseUrl:
    process.env.HS_DATABASE_URL ??
    `postgresql://db_user:db_pass@127.0.0.1:${process.env.HS_DB_PORT ?? "15432"}/hyperswitch_db`,
} as const;
