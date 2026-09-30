package org.terminus.mobile.api

import kotlinx.serialization.encodeToString

/**
 * Endpoint yang dipanggil SEBELUM ada sesi (login/register/refresh) atau
 * dengan access token mentah (resolusi vault waktu login). Sesudah login,
 * request ber-auth lewat [ApiSession].
 */
class AuthApi(private val client: HttpClient) {
    private val json get() = client.json

    suspend fun login(baseUrl: String, email: String, password: String): AuthTokens =
        client.decode(client.send("POST", "$baseUrl/api/v1/auth/login", json.encodeToString(LoginRequest(email, password))))

    /** Cuma sukses kalau server belum punya user (admin pertama); error server diteruskan apa adanya. */
    suspend fun register(baseUrl: String, email: String, password: String, fullName: String): AuthTokens =
        client.decode(
            client.send("POST", "$baseUrl/api/v1/auth/register", json.encodeToString(RegisterRequest(email, password, fullName))),
        )

    /**
     * Tukar refresh token. 401 = token dicabut/kedaluwarsa -> [ApiError.SessionExpired]
     * (token WAJIB dibuang). Gagal jaringan/5xx -> error lain (token dipertahankan).
     */
    suspend fun refresh(baseUrl: String, refreshToken: String): AuthTokens {
        val result = client.send("POST", "$baseUrl/api/v1/auth/refresh", json.encodeToString(RefreshRequest(refreshToken)))
        if (result.status == 401) {
            throw ApiError.SessionExpired(client.httpError(result).message ?: "Sesi berakhir")
        }
        return client.decode(result)
    }

    suspend fun me(baseUrl: String, accessToken: String): Account =
        client.decode(client.send("GET", "$baseUrl/api/v1/auth/me", bearer = accessToken))

    suspend fun listVaults(baseUrl: String, accessToken: String): List<VaultSummary> =
        client.decode(client.send("GET", "$baseUrl/api/v1/vaults", bearer = accessToken))

    suspend fun createVault(baseUrl: String, accessToken: String, name: String): VaultSummary =
        client.decode(client.send("POST", "$baseUrl/api/v1/vaults", json.encodeToString(CreateVaultRequest(name)), accessToken))
}
