package org.terminus.mobile.api

/**
 * Semua kegagalan client API. Pesan [message] SELALU siap tampil ke user
 * (Bahasa Indonesia atau pesan `{ "error": ... }` dari server apa adanya).
 */
sealed class ApiError(message: String) : Exception(message) {
    /** Server membalas non-2xx (selain kasus [SessionExpired]). */
    class Http(val status: Int, message: String) : ApiError(message)

    /** Tidak sampai ke server (mati, salah alamat, timeout). Token TIDAK dibuang. */
    class Network(message: String) : ApiError(message)

    /** Refresh token ditolak server (dicabut/kedaluwarsa) — user harus login ulang. */
    class SessionExpired(message: String) : ApiError(message)
}
