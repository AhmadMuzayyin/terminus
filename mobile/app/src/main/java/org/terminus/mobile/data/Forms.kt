package org.terminus.mobile.data

import org.terminus.mobile.api.HostFields

/** Isian mentah form host (semua String, persis seperti diketik user). */
data class HostForm(
    val label: String = "",
    val host: String = "",
    val port: String = "22",
    val username: String = "",
    val groupId: String? = null,
    val password: String = "",
) {
    /**
     * Validasi & rapikan jadi [HostFields], atau pesan error siap tampil.
     * Label kosong = pakai host/IP (label wajib di server, tapi mengetik
     * dua kali hal yang sama di HP itu menyebalkan). Username BOLEH kosong
     * (sama dengan backend & desktop — ditanya waktu connect).
     */
    fun toFields(): Result<HostFields> {
        val host = host.trim()
        val portNumber = port.trim().toIntOrNull()
        val error = when {
            host.isEmpty() -> "Host/IP wajib diisi."
            host.any { it.isWhitespace() } -> "Host/IP tidak boleh berisi spasi."
            portNumber == null || portNumber !in 1..65535 -> "Port harus angka 1–65535."
            else -> null
        }
        if (error != null) return Result.failure(IllegalArgumentException(error))
        return Result.success(
            HostFields(
                label = label.trim().ifEmpty { host },
                host = host,
                port = portNumber!!,
                username = username.trim(),
                groupId = groupId,
            ),
        )
    }

    /** Password baru yang harus dikirim ke server; null = tidak diubah. Spasi TIDAK dipotong. */
    val newPassword: String? get() = password.takeIf { it.isNotEmpty() }
}

/** Isian mentah form identity. */
data class IdentityForm(val label: String = "", val username: String = "", val password: String = "") {
    /** @param isNew identity baru wajib punya password; edit: kosong = tidak diubah. */
    fun validate(isNew: Boolean): String? = when {
        label.isBlank() -> "Label wajib diisi."
        username.isBlank() -> "Username wajib diisi."
        isNew && password.isEmpty() -> "Password wajib diisi."
        else -> null
    }
}

/** Nama grup: dipotong spasinya, tidak boleh kosong. */
fun validateGroupName(name: String): Result<String> =
    name.trim().takeIf { it.isNotEmpty() }?.let { Result.success(it) }
        ?: Result.failure(IllegalArgumentException("Nama grup wajib diisi."))
