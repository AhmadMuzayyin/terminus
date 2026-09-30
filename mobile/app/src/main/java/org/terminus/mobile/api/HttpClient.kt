package org.terminus.mobile.api

import java.io.IOException
import java.util.concurrent.TimeUnit
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import kotlinx.serialization.json.Json
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.RequestBody.Companion.toRequestBody

/** Hasil mentah satu request — status APA PUN dikembalikan, caller yang memutuskan. */
data class HttpResult(val status: Int, val body: String) {
    val isSuccess: Boolean get() = status in 200..299
}

/**
 * Lapisan HTTP paling bawah: kirim request, balikan [HttpResult]. Cuma
 * kegagalan JARINGAN yang jadi exception ([ApiError.Network]). Murni
 * JVM (tanpa Android) supaya bisa dites dengan MockWebServer.
 */
class HttpClient(
    private val http: OkHttpClient = defaultOkHttp(),
    val json: Json = defaultJson,
) {
    suspend fun send(method: String, url: String, bodyJson: String? = null, bearer: String? = null): HttpResult =
        withContext(Dispatchers.IO) {
            val body = bodyJson?.toRequestBody(JSON_TYPE)
                ?: if (method == "POST" || method == "PUT" || method == "PATCH") "{}".toRequestBody(JSON_TYPE) else null
            val request = Request.Builder()
                .url(url)
                .method(method, body)
                .apply { if (bearer != null) header("Authorization", "Bearer $bearer") }
                .build()
            try {
                http.newCall(request).execute().use { resp -> HttpResult(resp.code, resp.body.string()) }
            } catch (e: IOException) {
                throw ApiError.Network("Tidak bisa terhubung ke server: ${e.message ?: e.javaClass.simpleName}")
            } catch (e: IllegalArgumentException) {
                throw ApiError.Network("Alamat server tidak valid: ${e.message}")
            }
        }

    /** Decode body sukses, atau lempar [ApiError.Http] berisi pesan `error` dari server. */
    inline fun <reified T> decode(result: HttpResult): T {
        if (!result.isSuccess) throw httpError(result)
        return try {
            json.decodeFromString<T>(result.body)
        } catch (e: Exception) {
            throw ApiError.Http(result.status, "Respons server tidak terduga: ${e.message}")
        }
    }

    fun expectSuccess(result: HttpResult) {
        if (!result.isSuccess) throw httpError(result)
    }

    fun httpError(result: HttpResult): ApiError.Http {
        val message = runCatching { json.decodeFromString<ErrorBody>(result.body).error }.getOrNull()
        return ApiError.Http(result.status, message ?: "Server membalas HTTP ${result.status}")
    }

    companion object {
        private val JSON_TYPE = "application/json; charset=utf-8".toMediaType()

        val defaultJson = Json {
            ignoreUnknownKeys = true
            explicitNulls = false
        }

        /** Timeout wajib — tanpa ini server yang hang bikin layar login loading selamanya. */
        fun defaultOkHttp(): OkHttpClient = OkHttpClient.Builder()
            .connectTimeout(15, TimeUnit.SECONDS)
            .readTimeout(20, TimeUnit.SECONDS)
            .writeTimeout(20, TimeUnit.SECONDS)
            .build()
    }
}
