package org.terminus.mobile.data

/** Perubahan profil yang dikirim ke `PATCH /auth/me` (null = field tidak diubah). */
data class ProfileUpdate(val fullName: String?, val email: String?, val currentPassword: String?)

/**
 * Rencana perubahan profil — aturan & pesan SAMA PERSIS dengan
 * `plan_account_update` desktop (`crates/app/src/auth_flow.rs`):
 * nama & email wajib; cuma field yang berubah yang dikirim; ganti email
 * wajib password saat ini. `Result.success(null)` = tidak ada yang berubah.
 */
fun planProfileUpdate(
    currentName: String,
    currentEmail: String,
    nameInput: String,
    emailInput: String,
    passwordInput: String,
): Result<ProfileUpdate?> {
    val name = nameInput.trim()
    val email = emailInput.trim()
    fun fail(message: String) = Result.failure<ProfileUpdate?>(IllegalArgumentException(message))
    if (name.isEmpty()) return fail("Nama lengkap wajib diisi.")
    if (email.isEmpty()) return fail("Email wajib diisi.")
    val fullName = name.takeIf { it != currentName }
    val emailChanged = email != currentEmail
    if (emailChanged && passwordInput.isEmpty()) return fail("Masukkan password saat ini untuk mengganti email.")
    if (fullName == null && !emailChanged) return Result.success(null)
    return Result.success(
        ProfileUpdate(
            fullName = fullName,
            email = email.takeIf { emailChanged },
            currentPassword = passwordInput.takeIf { emailChanged },
        ),
    )
}

/** Ganti password: pesan sama dengan desktop. null = valid. */
fun validatePasswordChange(current: String, new: String, confirm: String): String? = when {
    current.isEmpty() -> "Password saat ini wajib diisi."
    new.length < 8 -> "Password baru minimal 8 karakter."
    new != confirm -> "Konfirmasi password baru tidak sama."
    else -> null
}
