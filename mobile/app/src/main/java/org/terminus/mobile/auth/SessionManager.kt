package org.terminus.mobile.auth

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import org.terminus.mobile.api.Account
import org.terminus.mobile.api.ApiError
import org.terminus.mobile.api.ApiSession
import org.terminus.mobile.api.AuthApi
import org.terminus.mobile.api.AuthTokens
import org.terminus.mobile.api.HttpClient

/** Status login app secara keseluruhan — menentukan layar mana yang tampil. */
sealed interface AuthState {
    /** Baru start: membaca token tersimpan / login otomatis. */
    data object Starting : AuthState

    data class LoggedOut(val serverUrl: String) : AuthState

    data class LoggedIn(val account: Account, val session: ApiSession) : AuthState
}

/** Status form login/daftar (terpisah supaya pindah form tidak menghapus pesan). */
data class LoginUiState(val busy: Boolean = false, val error: String? = null, val info: String? = null)

/**
 * Alur login mode Self-hosted (mobile/DESIGN.md bagian 5–6). SATU
 * instance per proses (lihat `AppContainer`). Semua pemanggilan dari UI
 * dijalankan di [scope] milik aplikasi, bukan scope layar — logout/login
 * tidak boleh terputus cuma karena layar berganti.
 */
class SessionManager(
    private val client: HttpClient,
    private val tokenStore: RefreshTokenStore,
    private val configStore: ConfigStore,
    private val scope: CoroutineScope,
) {
    private val authApi = AuthApi(client)

    private val _state = MutableStateFlow<AuthState>(AuthState.Starting)
    val state: StateFlow<AuthState> = _state.asStateFlow()

    private val _loginUi = MutableStateFlow(LoginUiState())
    val loginUi: StateFlow<LoginUiState> = _loginUi.asStateFlow()

    /** Dipanggil sekali waktu app start: login otomatis kalau ada token tersimpan. */
    fun start() = scope.launch {
        val config = configStore.load()
        val serverUrl = config.serverUrl
        val token = tokenStore.load()
        if (serverUrl == null || token == null) {
            _state.value = AuthState.LoggedOut(serverUrl.orEmpty())
            return@launch
        }
        _loginUi.value = LoginUiState(busy = true, info = "Masuk otomatis…")
        try {
            finishLogin(serverUrl, authApi.refresh(serverUrl, token))
        } catch (e: ApiError.SessionExpired) {
            tokenStore.clear()
            showLoggedOut(serverUrl, error = "Sesi berakhir, silakan masuk lagi.")
        } catch (e: ApiError) {
            // Server mati / jaringan putus: token SENGAJA tidak dibuang —
            // coba lagi cukup dengan membuka ulang app atau tekan Masuk.
            showLoggedOut(serverUrl, error = e.message)
        }
    }

    fun login(rawUrl: String, email: String, password: String) = runAuth(rawUrl) { url ->
        authApi.login(url, email.trim(), password)
    }

    fun register(rawUrl: String, fullName: String, email: String, password: String, confirm: String) {
        val error = when {
            fullName.isBlank() -> "Nama lengkap wajib diisi."
            password.length < 8 -> "Password minimal 8 karakter."
            password != confirm -> "Konfirmasi password tidak sama."
            else -> null
        }
        if (error != null) {
            _loginUi.value = LoginUiState(error = error)
            return
        }
        runAuth(rawUrl) { url -> authApi.register(url, email.trim(), password, fullName.trim()) }
    }

    /**
     * Logout: tandai sesi (rotasi token di background berhenti dipersist)
     * -> hapus token tersimpan -> cabut di server (best-effort) -> layar
     * Login dengan URL tetap terisi.
     */
    fun logout() = scope.launch {
        val loggedIn = _state.value as? AuthState.LoggedIn ?: return@launch
        loggedIn.session.markLoggedOut()
        tokenStore.clear()
        showLoggedOut(loggedIn.session.baseUrl, info = "Kamu sudah logout.")
        runCatching { loggedIn.session.logout() }
    }

    fun clearLoginMessages() = _loginUi.update { it.copy(error = null, info = null) }

    private fun runAuth(rawUrl: String, call: suspend (String) -> AuthTokens) {
        if (_loginUi.value.busy) return
        val url = normalizeServerUrl(rawUrl).getOrElse {
            _loginUi.value = LoginUiState(error = it.message)
            return
        }
        _loginUi.value = LoginUiState(busy = true)
        scope.launch {
            try {
                finishLogin(url, call(url))
            } catch (e: ApiError) {
                _loginUi.value = LoginUiState(error = e.message)
            }
        }
    }

    private suspend fun finishLogin(serverUrl: String, tokens: AuthTokens) {
        tokenStore.save(tokens.refreshToken)
        val previous = configStore.load()
        val preferred = previous.vaultId.takeIf { previous.serverUrl == serverUrl }
        val vaultId = resolveVaultId(serverUrl, tokens.accessToken, preferred)
        configStore.save(AppConfig(serverUrl = serverUrl, vaultId = vaultId))

        val session = ApiSession(
            client = client,
            authApi = authApi,
            baseUrl = serverUrl,
            vaultId = vaultId,
            tokens = tokens,
            persistRefreshToken = { tokenStore.save(it) },
            onSessionExpired = { onSessionExpired(serverUrl) },
        )
        val account = authApi.me(serverUrl, tokens.accessToken)
        _loginUi.value = LoginUiState()
        _state.value = AuthState.LoggedIn(account, session)
    }

    /**
     * `vault_id` tersimpan dipakai lagi SELAMA user masih anggotanya; kalau
     * tidak, vault pertama; 0 vault -> buat "My Vault" (sama dengan desktop).
     */
    private suspend fun resolveVaultId(serverUrl: String, accessToken: String, preferred: String?): String {
        val vaults = authApi.listVaults(serverUrl, accessToken)
        if (preferred != null && vaults.any { it.id == preferred }) return preferred
        return vaults.firstOrNull()?.id ?: authApi.createVault(serverUrl, accessToken, DEFAULT_VAULT_NAME).id
    }

    private suspend fun onSessionExpired(serverUrl: String) {
        tokenStore.clear()
        showLoggedOut(serverUrl, error = "Sesi berakhir, silakan masuk lagi.")
    }

    private fun showLoggedOut(serverUrl: String, error: String? = null, info: String? = null) {
        _loginUi.value = LoginUiState(error = error, info = info)
        _state.value = AuthState.LoggedOut(serverUrl)
    }

    companion object {
        const val DEFAULT_VAULT_NAME = "My Vault"
    }
}
