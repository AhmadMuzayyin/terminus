package org.terminus.mobile.auth

import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

enum class LockState {
    /** Pengaturan belum terbaca — jangan tampilkan isi app dulu. */
    Unknown,
    Locked,
    Unlocked,
}

/**
 * Kunci app opsional (mobile/DESIGN.md bagian 6): kalau aktif, minta
 * biometrik / kunci layar waktu app dibuka (proses baru) & setelah
 * [timeoutMillis] di background. Logika murni — waktu dari [clock]
 * (monotonic, `SystemClock.elapsedRealtime` di app) supaya bisa dites.
 * Sesi SSH/SFTP TETAP jalan selama terkunci; yang dikunci cuma tampilan.
 */
class AppLock(
    private val clock: () -> Long,
    private val timeoutMillis: Long = DEFAULT_TIMEOUT_MILLIS,
) {
    private val _state = MutableStateFlow(LockState.Unknown)
    val state: StateFlow<LockState> = _state.asStateFlow()

    private var enabled = false
    private var backgroundedAt: Long? = null

    /** Pengaturan terbaca/berubah. Pembacaan PERTAMA menentukan kunci awal proses. */
    fun onSettings(enabled: Boolean) {
        this.enabled = enabled
        when {
            _state.value == LockState.Unknown -> _state.value = if (enabled) LockState.Locked else LockState.Unlocked
            !enabled -> _state.value = LockState.Unlocked
        }
    }

    fun onBackground() {
        backgroundedAt = clock()
    }

    fun onForeground() {
        val since = backgroundedAt ?: return
        backgroundedAt = null
        if (enabled && _state.value == LockState.Unlocked && clock() - since >= timeoutMillis) {
            _state.value = LockState.Locked
        }
    }

    /** Autentikasi biometrik / kunci layar berhasil. */
    fun unlock() {
        if (_state.value == LockState.Locked) _state.value = LockState.Unlocked
    }

    companion object {
        const val DEFAULT_TIMEOUT_MILLIS = 5 * 60 * 1000L
    }
}
