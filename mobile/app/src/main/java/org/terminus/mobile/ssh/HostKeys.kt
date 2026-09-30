package org.terminus.mobile.ssh

import java.security.MessageDigest
import java.security.PublicKey
import java.util.Base64
import net.schmizz.sshj.common.Buffer
import net.schmizz.sshj.common.KeyType

/** Host key server dalam bentuk yang disimpan & ditampilkan: algoritma + sidik jari SHA256. */
data class HostKeyInfo(val algorithm: String, val fingerprint: String) {
    /** Format simpan: `ssh-ed25519 SHA256:abc…` (satu baris). */
    fun serialize(): String = "$algorithm $fingerprint"

    companion object {
        fun parse(raw: String): HostKeyInfo? {
            val parts = raw.trim().split(' ')
            return if (parts.size == 2 && parts[1].startsWith("SHA256:")) HostKeyInfo(parts[0], parts[1]) else null
        }
    }
}

/** Hasil mencocokkan host key server dengan known_hosts (mobile/DESIGN.md bagian 6). */
sealed interface HostKeyCheck {
    data object Trusted : HostKeyCheck

    /** Host belum pernah dipercaya — tanya user dulu. */
    data class Unknown(val actual: HostKeyInfo) : HostKeyCheck

    /** Host key BERBEDA dari yang dipercaya — koneksi DIBLOKIR (kemungkinan MITM). */
    data class Changed(val expected: HostKeyInfo, val actual: HostKeyInfo) : HostKeyCheck
}

fun checkHostKey(stored: HostKeyInfo?, actual: HostKeyInfo): HostKeyCheck = when {
    stored == null -> HostKeyCheck.Unknown(actual)
    // Algoritma beda juga dianggap berubah: sshj diminta memprioritaskan
    // algoritma tersimpan (lihat `findExistingAlgorithms`), jadi server jujur
    // selalu menyodorkan jenis kunci yang sama.
    stored == actual -> HostKeyCheck.Trusted
    else -> HostKeyCheck.Changed(stored, actual)
}

/** Kunci known_hosts per host & port, format OpenSSH: `host` (port 22) atau `[host]:port`. */
fun knownHostId(host: String, port: Int): String {
    val h = host.trim().lowercase()
    return if (port == 22) h else "[$h]:$port"
}

/** Sidik jari gaya OpenSSH: `SHA256:` + base64 tanpa padding dari blob kunci. */
fun hostKeyInfo(key: PublicKey): HostKeyInfo {
    val blob = Buffer.PlainBuffer().putPublicKey(key).compactData
    val digest = MessageDigest.getInstance("SHA-256").digest(blob)
    return HostKeyInfo(KeyType.fromKey(key).toString(), "SHA256:" + Base64.getEncoder().withoutPadding().encodeToString(digest))
}

/** Penyimpanan host key yang sudah dipercaya (di HP, tidak di server). */
interface KnownHostsStore {
    suspend fun get(id: String): HostKeyInfo?

    suspend fun put(id: String, info: HostKeyInfo)

    suspend fun remove(id: String)
}
