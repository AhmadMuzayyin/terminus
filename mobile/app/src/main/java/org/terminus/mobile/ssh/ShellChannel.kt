package org.terminus.mobile.ssh

import java.io.InputStream

/**
 * Saluran byte shell interaktif di server — SATU-SATUNYA yang dilihat
 * lapisan terminal dari SSH (mobile/DESIGN.md bagian 4.4). Tidak tahu apa
 * pun soal Termux; tidak tahu apa pun soal layar.
 */
interface ShellChannel {
    /** Output server (dibaca terus di thread I/O sampai -1 = shell selesai). */
    val output: InputStream

    /** Kirim input user ke server. BLOKING (jaringan) — jangan dari main thread. */
    fun write(data: ByteArray)

    /** Ukuran layar berubah -> `window-change` ke server. BLOKING. */
    fun resize(columns: Int, rows: Int)

    /** Tutup shell & koneksinya. Aman dipanggil berkali-kali, dari thread mana pun. */
    fun close()
}
