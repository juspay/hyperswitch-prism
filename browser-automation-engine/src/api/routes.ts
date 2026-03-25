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

    const result = await engine.run(runRequest);
    return reply.code(200).send(result);
  });
}
