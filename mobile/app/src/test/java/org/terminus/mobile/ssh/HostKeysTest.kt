package org.terminus.mobile.ssh

import java.util.Base64
import net.schmizz.sshj.common.Buffer
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/** known_hosts di HP (mobile/DESIGN.md bagian 6). */
class HostKeysTest {
    private val ed = HostKeyInfo("ssh-ed25519", "SHA256:aaa")

    @Test fun `sidik jari sama persis dengan ssh-keygen -lf`() {
        // Kunci uji sekali pakai (dibuat ssh-keygen, bukan kunci server sungguhan).
        val blob = Base64.getDecoder().decode("AAAAC3NzaC1lZDI1NTE5AAAAIDDW3iItjMqnQ4/Jg91ZMMKek/9Fm4BNl2I1Rlhn61bm")
        val key = Buffer.PlainBuffer(blob).readPublicKey()
        assertEquals(
            HostKeyInfo("ssh-ed25519", "SHA256:VqrdF8Gdpb18i2wUg1NSppMXKH4u6z/Nnzqyvm8ERBY"),
            hostKeyInfo(key),
        )
    }

    @Test fun `belum tersimpan - Unknown, sama - Trusted, beda - Changed`() {
        assertEquals(HostKeyCheck.Unknown(ed), checkHostKey(null, ed))
        assertEquals(HostKeyCheck.Trusted, checkHostKey(ed, ed))
        val other = ed.copy(fingerprint = "SHA256:bbb")
        assertEquals(HostKeyCheck.Changed(ed, other), checkHostKey(ed, other))
        // Jenis kunci berbeda juga diblokir.
        val rsa = HostKeyInfo("ssh-rsa", "SHA256:aaa")
        assertEquals(HostKeyCheck.Changed(ed, rsa), checkHostKey(ed, rsa))
    }

    @Test fun `id known_hosts gaya OpenSSH, host tidak peka huruf besar`() {
        assertEquals("server.lan", knownHostId(" Server.LAN ", 22))
        assertEquals("[10.0.2.2]:2222", knownHostId("10.0.2.2", 2222))
    }

    @Test fun `format simpan bolak-balik, isian rusak ditolak`() {
        assertEquals(ed, HostKeyInfo.parse(ed.serialize()))
        assertNull(HostKeyInfo.parse("sampah"))
        assertNull(HostKeyInfo.parse("ssh-ed25519 MD5:aa"))
    }
}
