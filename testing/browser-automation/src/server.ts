import Fastify from "fastify";
import { registerRoutes } from "./api/routes";
import { PlaywrightDriverFactory } from "./drivers/playwrightDriver";
import { AutomationEngine } from "./engine/automationEngine";

async function start(): Promise<void> {
  const app = Fastify({ logger: true });
  const engine = new AutomationEngine(new PlaywrightDriverFactory());

  await registerRoutes(app, engine);

  const portEnv = process.env.PORT;
  const port = portEnv ? Number.parseInt(portEnv, 10) : 3000;

  if (Number.isNaN(port) || port <= 0 || port > 65535) {
    throw new Error(`Invalid PORT environment variable: ${portEnv}`);
  }

  const host = process.env.HOST ?? "0.0.0.0";

  await app.listen({ port, host });
  app.log.info(`browser automation engine listening on http://${host}:${port}`);

  // Graceful shutdown handler
  const shutdown = async (signal: string): Promise<void> => {
    app.log.info(`Received ${signal}, starting graceful shutdown`);
    try {
      await engine.cleanup();
      await app.close();
      app.log.info("Graceful shutdown completed");
      process.exit(0);
    } catch (error) {
      app.log.error({ error }, "Error during shutdown");
      process.exit(1);
    }
  };

  process.on("SIGTERM", () => void shutdown("SIGTERM"));
  process.on("SIGINT", () => void shutdown("SIGINT"));
}

start().catch((error) => {
  // Final guard to surface startup failures in environments without logging pipeline.
  console.error(error);
  process.exit(1);
});
