package org.terminus.mobile.terminal

/**
 * Warna terminal: latar, teks, 16 warna ANSI (ARGB). Dipasang ke emulator
 * lewat [EmulatorSession.applyTheme].
 */
class TerminalTheme(val name: String, val background: Int, val foreground: Int, val ansi: IntArray) {
    init {
        require(ansi.size == 16) { "Tema butuh tepat 16 warna ANSI" }
    }
}

/**
 * Tema bawaan — SALINAN PERSIS `built_in_themes()` desktop
 * (`crates/term-emulator/src/palette.rs`), nama & urutan sama, supaya
 * `terminalTheme` host yang dipilih di desktop tampil sama di HP.
 */
object TerminalThemes {
    const val DEFAULT_NAME = "Terminus Dark"

    val all: List<TerminalTheme> = listOf(
        TerminalTheme(
            "Terminus Dark", rgb(0x060e20), rgb(0xdae2fd),
            ansi(
                0x000000, 0xcd3131, 0x0dbc79, 0xe5e510, 0x2472c8, 0xbc3fbc, 0x11a8cd, 0xe5e5e5,
                0x666666, 0xf14c4c, 0x23d18b, 0xf5f543, 0x3b8eea, 0xd670d6, 0x29b8db, 0xffffff,
            ),
        ),
        TerminalTheme(
            "Terminus Light", rgb(0xfafafa), rgb(0x1a1a1a),
            ansi(
                0x000000, 0xc41a1a, 0x0a8f5b, 0xa68600, 0x1a56a6, 0x9a2c9a, 0x0e8299, 0x4d4d4d,
                0x767676, 0xe03c3c, 0x1cb373, 0xc7a10a, 0x3b82e0, 0xc24fc2, 0x2ca6c2, 0x1a1a1a,
            ),
        ),
        TerminalTheme(
            "Midnight Blue", rgb(0x0a1228), rgb(0xd6e6ff),
            ansi(
                0x0a1228, 0xff5c5c, 0x4cd68c, 0xffd166, 0x4f9cff, 0xc47aff, 0x4fd6e0, 0xc9d8f0,
                0x505f82, 0xff8a8a, 0x7ce8ab, 0xffe099, 0x87bbff, 0xd9a8ff, 0x87e8f0, 0xffffff,
            ),
        ),
        TerminalTheme(
            "Solar Flare", rgb(0x1b0f0a), rgb(0xffe4c4),
            ansi(
                0x1b0f0a, 0xe04b2c, 0x8fb33d, 0xe0a62c, 0xd96a2c, 0xc23c6e, 0xe08a2c, 0xe8c9a8,
                0x6b4a3a, 0xff6e4d, 0xb5e05c, 0xffc64d, 0xff8f4d, 0xe07a9c, 0xffad4d, 0xfff3e0,
            ),
        ),
        mono("Mono Green", bg = 0x030a03, dim = 0x1f7a1f, normal = 0x33d633, bright = 0x6bff6b),
        mono("Mono Amber", bg = 0x0a0702, dim = 0x8a5a10, normal = 0xdb9a1f, bright = 0xffc45c),
    )

    fun byName(name: String?): TerminalTheme? = all.firstOrNull { it.name == name }

    /**
     * Tema satu sesi: tema host (diatur di desktop, `terminalTheme` di
     * server) kalau namanya dikenal; kalau tidak, tema default Pengaturan.
     */
    fun resolve(hostTheme: String?, defaultName: String): TerminalTheme =
        byName(hostTheme) ?: byName(defaultName) ?: all.first()

    /** Sama dengan `mono_green`/`mono_amber` desktop. */
    private fun mono(name: String, bg: Int, dim: Int, normal: Int, bright: Int) = TerminalTheme(
        name, rgb(bg), rgb(normal),
        ansi(bg, dim, normal, normal, dim, dim, normal, normal, dim, bright, bright, bright, bright, bright, bright, bright),
    )

    private fun rgb(value: Int): Int = 0xFF000000.toInt() or value

    private fun ansi(vararg values: Int): IntArray = IntArray(values.size) { rgb(values[it]) }
}
