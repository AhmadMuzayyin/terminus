package org.terminus.mobile.sftp

import java.util.Locale
import org.terminus.mobile.ssh.RemoteEntry
import org.terminus.mobile.ssh.RemoteKind

// Fungsi murni untuk path & tampilan SFTP (path server selalu gaya POSIX).

fun joinPath(dir: String, name: String): String = if (dir.endsWith("/")) dir + name else "$dir/$name"

/** Folder induk; `/` untuk isi akar; null kalau sudah di akar. */
fun parentPath(path: String): String? {
    val trimmed = path.trimEnd('/')
    if (trimmed.isEmpty()) return null
    val cut = trimmed.lastIndexOf('/')
    return if (cut <= 0) "/" else trimmed.substring(0, cut)
}

/** Nama file/folder baru: dipotong spasi ujungnya; tidak boleh kosong, `.`/`..`, atau berisi `/`. */
fun validateRemoteName(raw: String): Result<String> {
    val name = raw.trim()
    val error = when {
        name.isEmpty() -> "Nama wajib diisi."
        name == "." || name == ".." -> "Nama tidak boleh \".\" atau \"..\"."
        '/' in name -> "Nama tidak boleh berisi \"/\"."
        '\u0000' in name -> "Nama berisi karakter tidak valid."
        else -> null
    }
    return if (error == null) Result.success(name) else Result.failure(IllegalArgumentException(error))
}

/** Folder (dan symlink) dulu, lalu file; masing-masing urut nama tanpa peka huruf besar. */
fun sortEntries(entries: List<RemoteEntry>): List<RemoteEntry> =
    entries.sortedWith(compareBy<RemoteEntry> { if (it.kind == RemoteKind.File) 1 else 0 }.thenBy(String.CASE_INSENSITIVE_ORDER) { it.name })

/** 0 B, 512 B, 1,5 KB, 20 MB … (desimal koma, gaya Indonesia). */
fun formatSize(bytes: Long): String {
    if (bytes < 1024) return "$bytes B"
    val units = listOf("KB", "MB", "GB", "TB")
    var value = bytes / 1024.0
    var unit = 0
    while (value >= 1024 && unit < units.lastIndex) {
        value /= 1024
        unit++
    }
    val text = if (value >= 100) String.format(Locale.ROOT, "%.0f", value) else String.format(Locale.ROOT, "%.1f", value)
    return text.removeSuffix(".0").replace('.', ',') + " " + units[unit]
}
