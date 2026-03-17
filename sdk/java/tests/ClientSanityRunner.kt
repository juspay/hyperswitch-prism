import payments.*
import java.io.File
import java.nio.charset.StandardCharsets
import org.json.JSONObject
import java.util.Base64
import java.util.Scanner

fun main() {
    val sc = Scanner(System.`in`)
    val sb = StringBuilder()
    while (sc.hasNextLine()) {
        sb.append(sc.nextLine())
    }
    
    val input = JSONObject(sb.toString())
    val scenarioId = input.getString("scenario_id")
    val sourceId = input.getString("source_id")
    val reqData = input.getJSONObject("request")
    val proxy = input.optJSONObject("proxy")
    val clientTimeoutMs = input.optLong("client_timeout_ms", -1L)
    val clientResponseTimeoutMs = input.optLong("client_response_timeout_ms", -1L)

    val headers = mutableMapOf<String, String>()
    if (reqData.has("headers")) {
        val h = reqData.getJSONObject("headers")
        for (key in h.keys()) {
            headers[key] = h.getString(key)
        }
    }
    headers["x-source"] = sourceId
    headers["x-scenario-id"] = scenarioId

    val body = if (reqData.has("body") && !reqData.isNull("body")) {
        val b = reqData.getString("body")
        if (b.startsWith("base64:")) {
            Base64.getDecoder().decode(b.substring(7))
        } else {
            b.toByteArray(StandardCharsets.UTF_8)
        }
    } else null

    val request = HttpRequest(
        url = reqData.getString("url"),
        method = reqData.getString("method"),
        headers = headers,
        body = body
    )

    val httpConfig = when {
        clientTimeoutMs > 0 -> HttpConfig.newBuilder().setTotalTimeoutMs(clientTimeoutMs.toInt()).build()
        clientResponseTimeoutMs > 0 -> HttpConfig.newBuilder().setResponseTimeoutMs(clientResponseTimeoutMs.toInt()).build()
        else -> null
    }

    val output = JSONObject()
    try {
        val clientConfig = if (proxy != null) {
            val httpUrl = proxy.optString("http_url", "")
            if (httpUrl.isNotEmpty()) {
                HttpConfig.newBuilder()
                    .setProxy(ProxyOptions.newBuilder().setHttpUrl(httpUrl).build())
                    .build()
            } else null
        } else null

        val client = HttpClient.createClient(clientConfig)
        val sdkResponse = HttpClient.execute(request, httpConfig, client)
        
        val ct = (sdkResponse.headers["content-type"] ?: "").lowercase()
        val bodyStr = if ("application/octet-stream" in ct) {
            Base64.getEncoder().encodeToString(sdkResponse.body)
        } else {
            String(sdkResponse.body, StandardCharsets.UTF_8)
        }
        
        output.put("response", JSONObject().apply {
            put("statusCode", sdkResponse.statusCode)
            put("headers", JSONObject(sdkResponse.headers))
            put("body", bodyStr)
        })
    } catch (e: Exception) {
        val code = if (e is NetworkError) e.code.name else "UNKNOWN_ERROR"
        output.put("error", JSONObject().apply {
            put("code", code)
            put("message", e.message ?: e.toString())
        })
    }

    println(output.toString())
}
