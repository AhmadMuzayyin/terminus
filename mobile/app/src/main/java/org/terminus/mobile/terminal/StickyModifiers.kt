package org.terminus.mobile.terminal

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue

/**
 * Ctrl/Alt "tempel" di baris tombol ekstra (mobile/DESIGN.md bagian 7):
 * tekan sekali -> aktif untuk SATU tombol berikutnya (dari keyboard HP
 * atau baris ekstra), lalu mati sendiri. State Compose supaya tombolnya
 * ikut menyala/padam. Semua akses di main thread.
 */
class StickyModifiers {
    var ctrl by mutableStateOf(false)
    var alt by mutableStateOf(false)

    /** Baca & matikan (dipanggil view tepat saat satu tombol diproses). */
    fun consumeCtrl(): Boolean = ctrl.also { ctrl = false }

    fun consumeAlt(): Boolean = alt.also { alt = false }
}

/**
 * Ukuran font terminal (sp), dipakai semua sesi. Diubah lewat pinch. Belum
 * disimpan permanen — pengaturan font menyusul di Milestone 6.
 */
class FontSize(initialSp: Float = DEFAULT_SP) {
    var sp by mutableFloatStateOf(initialSp)
        private set

    fun set(value: Float) {
        sp = value.coerceIn(MIN_SP, MAX_SP)
    }

    companion object {
        const val DEFAULT_SP = 13f
        const val MIN_SP = 7f
        const val MAX_SP = 32f
    }
}
