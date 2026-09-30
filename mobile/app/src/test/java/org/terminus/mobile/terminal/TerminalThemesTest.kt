package org.terminus.mobile.terminal

import org.junit.Assert.assertEquals
import org.junit.Test

class TerminalThemesTest {
    @Test fun `nama & urutan sama dengan built_in_themes desktop`() {
        assertEquals(
            listOf("Terminus Dark", "Terminus Light", "Midnight Blue", "Solar Flare", "Mono Green", "Mono Amber"),
            TerminalThemes.all.map { it.name },
        )
    }

    @Test fun `warna disalin persis dari palette rs (contoh Terminus Dark)`() {
        val dark = TerminalThemes.byName("Terminus Dark")!!
        assertEquals(0xFF060E20.toInt(), dark.background)
        assertEquals(0xFFDAE2FD.toInt(), dark.foreground)
        assertEquals(0xFF0DBC79.toInt(), dark.ansi[2])
    }

    @Test fun `tema host menang, lalu default, lalu tema pertama`() {
        assertEquals("Mono Amber", TerminalThemes.resolve("Mono Amber", "Midnight Blue").name)
        assertEquals("Midnight Blue", TerminalThemes.resolve(null, "Midnight Blue").name)
        assertEquals("Midnight Blue", TerminalThemes.resolve("Tema Tak Dikenal", "Midnight Blue").name)
        assertEquals("Terminus Dark", TerminalThemes.resolve(null, "rusak").name)
    }
}
