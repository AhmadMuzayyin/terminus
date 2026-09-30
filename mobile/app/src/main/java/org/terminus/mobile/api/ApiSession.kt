package org.terminus.mobile.api

import java.util.concurrent.atomic.AtomicBoolean
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.serialization.encodeToString

/**
 * Sesi login ke satu vault di satu server. Semua request ber-auth lewat
 * [send], yang menangani access token kedaluwarsa secara transparan:
 * 401 -> refresh SEKALI -> ulangi request (aturan yang sama dengan desktop,
 * mobile/DESIGN.md bagian 5).
 *
 * @param persistRefreshToken dipanggil tiap refresh token BARU keluar
 *   (rotasi) — backend langsung mencabut yang lama, jadi kalau tidak
 *   dipersist, login otomatis berikutnya pasti gagal.
 * @param onSessionExpired dipanggil kalau refresh ditolak server (user
 *   harus login ulang).
 */
class ApiSession(
    private val client: HttpClient,
    private val authApi: AuthApi,
    val baseUrl: String,
    val vaultId: String,
    tokens: AuthTokens,
    private val persistRefreshToken: suspend (String) -> Unit,
    private val onSessionExpired: suspend (String) -> Unit,
) {
    private val mutex = Mutex()
    @Volatile private var tokens: AuthTokens = tokens

    /**
     * Di-set PALING AWAL waktu logout: request yang masih jalan di
     * background bisa me-refresh token SETELAH logout; hasil rotasinya
     * tidak boleh dipersist (kalau dipersist, start berikutnya malah
     * masuk otomatis padahal user sudah logout).
     */
    private val loggedOut = AtomicBoolean(false)

    val currentRefreshToken: String get() = tokens.refreshToken

    fun markLoggedOut() = loggedOut.set(true)

    /** URL endpoint di dalam vault ini, mis. `vaultUrl("/hosts")`. */
    fun vaultUrl(path: String): String = "$baseUrl/api/v1/vaults/$vaultId$path"

    fun authUrl(path: String): String = "$baseUrl/api/v1/auth$path"

    suspend fun send(method: String, url: String, bodyJson: String? = null): HttpResult {
        val usedAccess = tokens.accessToken
        val first = client.send(method, url, bodyJson, usedAccess)
        if (first.status != 401) return first
        refreshIfStillUsing(usedAccess)
        return client.send(method, url, bodyJson, tokens.accessToken)
    }

    /**
     * Beberapa request bisa kena 401 bersamaan — cuma yang PERTAMA yang
     * benar-benar refresh; sisanya lihat access token sudah berganti dan
     * langsung pakai yang baru (refresh token lama sudah dicabut server,
     * refresh kedua dengan token lama pasti ditolak).
     */
    private suspend fun refreshIfStillUsing(usedAccess: String) = mutex.withLock {
        if (tokens.accessToken != usedAccess) return@withLock
        val fresh = try {
            authApi.refresh(baseUrl, tokens.refreshToken)
        } catch (e: ApiError.SessionExpired) {
            onSessionExpired(e.message ?: "Sesi berakhir")
            throw e
        }
        installTokens(fresh)
    }

    /** Pasang token baru (hasil refresh ATAU ganti password) & persist — kecuali sudah logout. */
    suspend fun installTokens(fresh: AuthTokens) {
        tokens = fresh
        if (!loggedOut.get()) persistRefreshToken(fresh.refreshToken)
    }

    suspend fun me(): Account = client.decode(send("GET", authUrl("/me")))

    /** Cabut refresh token TERBARU di server. Best-effort — backend selalu 204. */
    suspend fun logout() {
        markLoggedOut()
        client.expectSuccess(
            client.send("POST", authUrl("/logout"), client.json.encodeToString(RefreshRequest(tokens.refreshToken))),
        )
    }
}
