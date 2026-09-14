import express from "express"
import fs from "fs"
import path from "path"
import { createStoreRoutes } from "./routes/store"
import { createAdminRoutes } from "./routes/admin"
import { createWebhookRoutes } from "./routes/webhooks"

const CLIENT_DIST = path.resolve(__dirname, "../client/dist")

export function createApp(credentials?: Record<string, any>) {
  const app = express()
  // Webhooks must see the raw, unparsed body: HMAC source verification is
  // computed over the exact bytes the connector sent, so this route is
  // mounted before the JSON body parser.
  app.use("/hooks/payment", express.raw({ type: "*/*" }), createWebhookRoutes(credentials))
  app.use(express.json())

  app.get("/health", (_req, res) => {
    res.json({ status: "ok", timestamp: new Date().toISOString() })
  })

  app.use("/store", createStoreRoutes(credentials))
  app.use("/admin", createAdminRoutes(credentials))

  // Serve the built test client so every connector has a URL on this server,
  // e.g. http://localhost:3000/paypal loads the PayPal checkout elements.
  app.use(express.static(CLIENT_DIST))
  app.get("*", (req, res, next) => {
    if (
      req.path.startsWith("/store") ||
      req.path.startsWith("/admin") ||
      req.path.startsWith("/hooks") ||
      req.path === "/health"
    ) {
      return next()
    }

    const indexHtml = path.join(CLIENT_DIST, "index.html")
    if (!fs.existsSync(indexHtml)) {
      return res.status(404).json({
        error:
          "Client build not found. Run `npm run build --workspace=app/client` first.",
      })
    }
    res.sendFile(indexHtml)
  })

  app.use(
    (
      err: Error,
      _req: express.Request,
      res: express.Response,
      _next: express.NextFunction
    ) => {
      console.error("[Server Error]", err)
      res.status(500).json({ error: err.message })
    }
  )

  return app
}
