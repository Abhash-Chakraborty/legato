package tech.abhashchakraborty.legato

import org.json.JSONArray
import org.json.JSONObject
import tech.abhashchakraborty.legato.core.TransformRequest
import java.net.HttpURLConnection
import java.net.URL

/**
 * Sends a transform to the AI provider the PC shared, as the PC's
 * `llm_client.rs` does: OpenAI-style chat completions or Anthropic messages,
 * with reasoning turned off where the provider allows it. HTTPS only, and
 * redirects aren't followed, so the key can't be carried anywhere else.
 */
object LlmClient {
    /** The model's answer to `text`. Blocks; throws with a plain reason. */
    fun complete(request: TransformRequest, key: String?, text: String): String {
        val base = request.baseUrl.trimEnd('/')
        if (!base.startsWith("https://")) error("${request.providerLabel} isn't on an HTTPS address, so this phone won't send to it")
        val anthropic = request.apiFormat == "anthropic"
        val url = URL(if (anthropic) "$base/messages" else "$base/chat/completions")
        val body = if (anthropic) {
            JSONObject()
                .put("model", request.model)
                .put("max_tokens", 4096)
                .put("system", request.system)
                .put("messages", JSONArray().put(message("user", text)))
        } else {
            JSONObject()
                .put("model", request.model)
                .put("messages", JSONArray().put(message("system", request.system)).put(message("user", text)))
                .also { reasoningOff(it, request) }
        }
        val conn = (url.openConnection() as HttpURLConnection).apply {
            requestMethod = "POST"
            instanceFollowRedirects = false
            connectTimeout = 15_000
            readTimeout = 60_000
            doOutput = true
            setRequestProperty("Content-Type", "application/json")
            if (!key.isNullOrBlank()) {
                if (anthropic) {
                    setRequestProperty("x-api-key", key)
                    setRequestProperty("anthropic-version", "2023-06-01")
                } else {
                    setRequestProperty("Authorization", "Bearer $key")
                }
            }
        }
        try {
            conn.outputStream.use { it.write(body.toString().toByteArray()) }
            val code = conn.responseCode
            if (code !in 200..299) error(problem(request.providerLabel, code))
            val answer = JSONObject(conn.inputStream.bufferedReader().use { it.readText() })
            return if (anthropic) {
                val parts = answer.getJSONArray("content")
                (0 until parts.length()).map { parts.getJSONObject(it) }
                    .filter { it.optString("type") == "text" }
                    .joinToString("") { it.optString("text") }
            } else {
                answer.getJSONArray("choices").getJSONObject(0).getJSONObject("message").optString("content")
            }
        } finally {
            conn.disconnect()
        }
    }

    private fun message(role: String, content: String) = JSONObject().put("role", role).put("content", content)

    /** The PC's `reasoning_disable_params`: each provider's way to skip thinking first. */
    private fun reasoningOff(body: JSONObject, request: TransformRequest) {
        val base = request.baseUrl.lowercase()
        when {
            "api.openai.com" in base -> openAiLowestEffort(request.model)?.let { body.put("reasoning_effort", it) }
            "api.deepseek.com" in base -> body.put("thinking", JSONObject().put("type", "disabled"))
            request.providerId == "openrouter" -> body.put("reasoning", JSONObject().put("effort", "none").put("exclude", true))
        }
    }

    /** GPT-5.1 and later take "none", the first GPT-5 models "minimal", the o-series "low". */
    private fun openAiLowestEffort(model: String): String? {
        val m = model.lowercase().substringAfterLast('/')
        return when {
            m.startsWith("gpt-5") -> {
                val rest = m.removePrefix("gpt-5")
                when {
                    "chat" in rest -> null
                    rest.startsWith(".") -> "none"
                    else -> "minimal"
                }
            }
            m.length > 1 && m[0] == 'o' && m[1].isDigit() -> "low"
            else -> null
        }
    }

    private fun problem(provider: String, code: Int): String = when (code) {
        401, 403 -> "$provider turned the key down. Check it in Legato on your PC."
        404 -> "$provider doesn't have that model"
        429 -> "$provider is rate limiting"
        408, 504 -> "$provider didn't answer in time"
        else -> "$provider answered with an error ($code)"
    }
}
