package org.terminus.mobile.data

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import org.terminus.mobile.api.HostFields

class FormsTest {
    private fun error(form: HostForm) = form.toFields().exceptionOrNull()?.message

    @Test fun `host valid dirapikan, label kosong pakai host`() {
        val fields = HostForm(label = "  ", host = " 10.0.0.1 ", port = " 2222 ", username = " root ", groupId = "g1")
            .toFields().getOrThrow()
        assertEquals(HostFields("10.0.0.1", "10.0.0.1", 2222, "root", "g1"), fields)
    }

    @Test fun `username boleh kosong`() {
        assertEquals("", HostForm(label = "srv", host = "srv.lan").toFields().getOrThrow().username)
    }

    @Test fun `host kosong, berspasi, dan port di luar rentang ditolak`() {
        assertEquals("Host/IP wajib diisi.", error(HostForm(host = "  ")))
        assertEquals("Host/IP tidak boleh berisi spasi.", error(HostForm(host = "10.0 .0.1")))
        assertEquals("Port harus angka 1–65535.", error(HostForm(host = "h", port = "")))
        assertEquals("Port harus angka 1–65535.", error(HostForm(host = "h", port = "0")))
        assertEquals("Port harus angka 1–65535.", error(HostForm(host = "h", port = "65536")))
    }

    @Test fun `password kosong berarti tidak diubah, spasi tidak dipotong`() {
        assertNull(HostForm(password = "").newPassword)
        assertEquals(" rahasia ", HostForm(password = " rahasia ").newPassword)
    }

    @Test fun `identity baru wajib password, edit boleh kosong`() {
        assertEquals("Label wajib diisi.", IdentityForm(" ", "u", "p").validate(isNew = true))
        assertEquals("Username wajib diisi.", IdentityForm("l", " ", "p").validate(isNew = true))
        assertEquals("Password wajib diisi.", IdentityForm("l", "u", "").validate(isNew = true))
        assertNull(IdentityForm("l", "u", "").validate(isNew = false))
    }

    @Test fun `nama grup dipotong spasinya dan tidak boleh kosong`() {
        assertEquals("Prod", validateGroupName("  Prod ").getOrThrow())
        assertEquals("Nama grup wajib diisi.", validateGroupName("   ").exceptionOrNull()?.message)
    }
}
