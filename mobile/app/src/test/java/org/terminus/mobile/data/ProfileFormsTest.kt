package org.terminus.mobile.data

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/** Aturan sama dengan `plan_account_update` desktop. */
class ProfileFormsTest {
    private fun plan(name: String, email: String, password: String = "") =
        planProfileUpdate("Budi", "budi@contoh.id", name, email, password)

    @Test fun `tidak ada perubahan - null`() {
        assertNull(plan(" Budi ", " budi@contoh.id ").getOrThrow())
    }

    @Test fun `ganti nama saja - tanpa password, email tidak dikirim`() {
        assertEquals(ProfileUpdate("Budi Santoso", null, null), plan("Budi Santoso", "budi@contoh.id").getOrThrow())
    }

    @Test fun `ganti email wajib password saat ini`() {
        assertEquals(
            "Masukkan password saat ini untuk mengganti email.",
            plan("Budi", "baru@contoh.id").exceptionOrNull()?.message,
        )
        assertEquals(
            ProfileUpdate(null, "baru@contoh.id", "rahasia"),
            plan("Budi", "baru@contoh.id", "rahasia").getOrThrow(),
        )
    }

    @Test fun `nama & email wajib`() {
        assertEquals("Nama lengkap wajib diisi.", plan("  ", "budi@contoh.id").exceptionOrNull()?.message)
        assertEquals("Email wajib diisi.", plan("Budi", " ").exceptionOrNull()?.message)
    }

    @Test fun `ganti password - saat ini wajib, minimal 8, konfirmasi sama`() {
        assertEquals("Password saat ini wajib diisi.", validatePasswordChange("", "password-baru", "password-baru"))
        assertEquals("Password baru minimal 8 karakter.", validatePasswordChange("lama", "pendek", "pendek"))
        assertEquals("Konfirmasi password baru tidak sama.", validatePasswordChange("lama", "password-baru", "password-beda"))
        assertNull(validatePasswordChange("lama", "password-baru", "password-baru"))
    }
}
