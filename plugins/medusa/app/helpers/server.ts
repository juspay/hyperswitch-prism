import http from "http"
import { createApp } from "../server"

export interface TestServer {
  url: string
  stop: () => Promise<void>
}

export async function startServer(options: {
  credentials?: Record<string, any>
  port?: number
}): Promise<TestServer> {
  const app = createApp(options.credentials)
  const server = http.createServer(app)

  return new Promise((resolve, reject) => {
    server.listen(options.port ?? 0, "127.0.0.1", () => {
      const address = server.address()
      if (!address || typeof address === "string") {
        reject(new Error("Failed to get server address"))
        return
      }
      const url = `http://127.0.0.1:${address.port}`
      resolve({
        url,
        stop: () =>
          new Promise((res) => {
            server.close(() => res())
          }),
      })
    })

    server.on("error", reject)
  })
}
