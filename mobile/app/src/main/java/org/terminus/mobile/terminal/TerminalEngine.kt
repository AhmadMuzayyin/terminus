package org.terminus.mobile.terminal

import android.content.Context
import android.view.View
import org.terminus.mobile.ssh.ShellChannel

// Titik tukar mesin terminal (mobile/DESIGN.md bagian 4.4). Layar & manajer
// sesi cuma memakai antarmuka di file ini; implementasi v1 = Termux
// (`TermuxEngine`). Pindah ke libghostty = implementasi baru dari tiga
// antarmuka ini, `ssh/` & layar lain tidak ikut berubah.

/** Pembuat sesi & permukaan terminal dari satu mesin terminal. */
interface TerminalEngine {
    fun newSession(): EmulatorSession

    fun newSurface(context: Context, modifiers: StickyModifiers, fontSize: FontSize): TerminalSurface
}

/**
 * Isi layar satu sesi (layar + riwayat scroll) yang disambungkan ke
 * [ShellChannel]. Hidup lebih lama dari view: pindah sesi / rotasi layar
 * tidak menghapus isinya; sambung ulang memakai layar yang sama.
 */
interface EmulatorSession {
    /** Ukuran terakhir (kolom × baris) — dipakai minta PTY waktu connect. */
    val columns: Int
    val rows: Int

    /** Pasang saluran shell & mulai menampilkan outputnya. Main thread. */
    fun attach(channel: ShellChannel)

    /** Dipanggil (main thread) saat shell berakhir SENDIRI: [error] null = ditutup server dengan normal. */
    var onChannelEnded: ((error: Throwable?) -> Unit)?

    /** Putus saluran tanpa memicu [onChannelEnded]; layar tetap. */
    fun detach()

    /** Tulis pesan status ke layar (bukan dari server), mis. "[koneksi terputus]". */
    fun printNotice(text: String)

    /** Lepas semua sumber daya; sesi tidak dipakai lagi. */
    fun dispose()
}

/** View yang menampilkan satu [EmulatorSession] + input keyboard/sentuh. */
interface TerminalSurface {
    val view: View

    /** Tampilkan sesi ini (null = lepas). */
    fun show(session: EmulatorSession?)

    /** Tombol ekstra non-karakter (Esc, Tab, panah) — memperhitungkan Ctrl/Alt tempel. */
    fun press(key: SpecialKey)

    /** Karakter dari baris tombol ekstra — memperhitungkan Ctrl/Alt tempel. */
    fun type(char: Char)

    /** Tempel isi clipboard sebagai input (bracketed paste kalau aplikasi server memintanya). */
    fun pasteClipboard()

    fun showKeyboard()
}

enum class SpecialKey { Escape, Tab, Left, Up, Down, Right }
