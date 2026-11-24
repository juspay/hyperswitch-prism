import assert from "node:assert/strict";
import test from "node:test";
import { localTargets } from "./local-target.js";

const localEnvironment = {
  CONNECTOR_AUTH_FILE_PATH: "/private/test/connector-auth.json",
};

test("accepts the default loopback HS and database targets", () => {
  assert.deepEqual(localTargets(localEnvironment), {
    hyperswitchBaseUrl: "http://127.0.0.1:8080",
    databaseUrl:
      "postgresql://db_user:db_pass@127.0.0.1:15432/hyperswitch_db",
    connectorAuthFilePath: "/private/test/connector-auth.json",
  });
});

test("accepts explicit localhost ports", () => {
  const targets = localTargets({
    ...localEnvironment,
    HS_BASE_URL: "http://localhost:9090",
    HS_DATABASE_URL: "postgresql://test:test@localhost:5433/test_db",
  });
  assert.equal(targets.hyperswitchBaseUrl, "http://localhost:9090");
});

test("rejects remote HS and database targets", () => {
  assert.throws(
    () =>
      localTargets({
        ...localEnvironment,
        HS_BASE_URL: "https://payments.example.com",
      }),
    /HS must use localhost/,
  );
  assert.throws(
    () =>
      localTargets({
        ...localEnvironment,
        HS_DATABASE_URL: "postgresql://test:test@database.example.com/test_db",
      }),
    /HS database must use localhost/,
  );
});

test("rejects CI, production mode, and relative credential paths", () => {
  assert.throws(
    () => localTargets({ ...localEnvironment, CI: "true" }),
    /disabled in CI/,
  );
  assert.throws(
    () => localTargets({ ...localEnvironment, JENKINS_URL: "configured" }),
    /disabled in CI/,
  );
  assert.throws(
    () => localTargets({ ...localEnvironment, UCS_ENVIRONMENT: "production" }),
    /refuses UCS_ENVIRONMENT=production/,
  );
  assert.throws(
    () => localTargets({ CONNECTOR_AUTH_FILE_PATH: "connector-auth.json" }),
    /absolute path/,
  );
});
