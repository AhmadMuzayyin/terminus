package org.terminus.mobile.sftp

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import org.terminus.mobile.ssh.RemoteEntry
import org.terminus.mobile.ssh.RemoteKind

class RemotePathsTest {
    @Test fun `gabung path dan folder induk`() {
        assertEquals("/home/a/b.txt", joinPath("/home/a", "b.txt"))
        assertEquals("/b.txt", joinPath("/", "b.txt"))
        assertEquals("/home", parentPath("/home/a"))
        assertEquals("/home", parentPath("/home/a/"))
        assertEquals("/", parentPath("/home"))
        assertNull(parentPath("/"))
    }

    @Test fun `validasi nama baru`() {
        assertEquals("laporan.txt", validateRemoteName("  laporan.txt ").getOrThrow())
        assertEquals("Nama wajib diisi.", validateRemoteName("  ").exceptionOrNull()?.message)
        assertEquals("Nama tidak boleh \".\" atau \"..\".", validateRemoteName("..").exceptionOrNull()?.message)
        assertEquals("Nama tidak boleh berisi \"/\".", validateRemoteName("a/b").exceptionOrNull()?.message)
    }

    @Test fun `folder dan symlink dulu, lalu file, urut nama`() {
        fun e(name: String, kind: RemoteKind) = RemoteEntry(name, "/$name", kind, 0, 0)
        val sorted = sortEntries(
            listOf(
                e("b.txt", RemoteKind.File),
                e("Zeta", RemoteKind.Directory),
                e("a.txt", RemoteKind.File),
                e("link", RemoteKind.Link),
                e("alpha", RemoteKind.Directory),
            ),
        )
        assertEquals(listOf("alpha", "link", "Zeta", "a.txt", "b.txt"), sorted.map { it.name })
    }

    @Test fun `format ukuran`() {
        assertEquals("0 B", formatSize(0))
        assertEquals("1023 B", formatSize(1023))
        assertEquals("1 KB", formatSize(1024))
        assertEquals("1,5 KB", formatSize(1536))
        assertEquals("20 MB", formatSize(20L * 1024 * 1024))
        assertEquals("150 MB", formatSize(150L * 1024 * 1024))
    }
}
