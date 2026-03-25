import Fastify from "fastify";
import { registerRoutes } from "./api/routes";
import { PlaywrightDriverFactory } from "./drivers/playwrightDriver";
import { AutomationEngine } from "./engine/automationEngine";

async function start(): Promise<void> {
  const app = Fastify({ logger: true });
  const engine = new AutomationEngine(new PlaywrightDriverFactory());

  await registerRoutes(app, engine);

  const port = Number(process.env.PORT ?? 3000);
  const host = process.env.HOST ?? "0.0.0.0";

  await app.listen({ port, host });
  app.log.info(`browser automation engine listening on http://${host}:${port}`);
}

start().catch((error) => {
  // Final guard to surface startup failures in environments without logging pipeline.
  console.error(error);
  process.exit(1);
});
