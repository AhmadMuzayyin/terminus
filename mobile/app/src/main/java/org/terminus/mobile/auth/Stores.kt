package org.terminus.mobile.auth

/** Penyimpanan refresh token. Implementasi asli: [KeystoreTokenStore]; test: fake in-memory. */
interface RefreshTokenStore {
    suspend fun load(): String?
    suspend fun save(token: String)
    suspend fun clear()
}

/** Config non-rahasia yang harus terbaca sebelum login (prefill URL, vault terakhir). */
data class AppConfig(val serverUrl: String? = null, val vaultId: String? = null)

interface ConfigStore {
    suspend fun load(): AppConfig
    suspend fun save(config: AppConfig)
}
