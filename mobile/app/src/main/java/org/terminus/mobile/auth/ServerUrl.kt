package org.terminus.mobile.auth

/**
 * Trim + buang `/` di akhir. Skema WAJIB eksplisit — menebak `https://`
 * diam-diam bikin server `http://` di LAN tidak bisa dihubungi tanpa
 * user tahu kenapa (sama aturan dengan desktop).
 */
fun normalizeServerUrl(raw: String): Result<String> {
    val url = raw.trim().trimEnd('/')
    return when {
        url.isEmpty() -> Result.failure(IllegalArgumentException("Server URL wajib diisi."))
        !(url.startsWith("http://") || url.startsWith("https://")) ->
            Result.failure(IllegalArgumentException("Server URL harus diawali http:// atau https://"))
        else -> Result.success(url)
    }
}
