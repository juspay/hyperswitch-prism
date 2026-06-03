import type { FastifyInstance } from "fastify";
import { AutomationEngine } from "../engine/automationEngine";
import { parseRunRequest, RequestValidationError } from "../utils/validation";

export async function registerRoutes(
  app: FastifyInstance,
  engine: AutomationEngine
): Promise<void> {
  app.post("/run", async (request, reply) => {
    let runRequest;
    try {
      runRequest = parseRunRequest(request.body);
    } catch (error) {
      const message =
        error instanceof RequestValidationError ? error.message : "Invalid request payload";
      return reply.code(400).send({
        success: false,
        error: message,
        data: {},
        steps: []
      });
    }

    try {
      const result = await engine.run(runRequest);
      // Return appropriate HTTP status based on automation success
      const statusCode = result.success ? 200 : 500;
      return reply.code(statusCode).send(result);
    } catch (error) {
      // Handle unexpected errors (e.g., Playwright crash, OOM)
      const message = error instanceof Error ? error.message : "Internal server error";
      app.log.error({ error }, "Automation engine error");
      return reply.code(500).send({
        success: false,
        error: message,
        data: {},
        finalUrl: "",
        steps: [],
        durationMs: 0
      });
    }
  });
}
