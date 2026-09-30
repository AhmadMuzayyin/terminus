package org.terminus.mobile.ssh

import java.io.IOException
import java.io.InputStream
import java.net.ConnectException
import java.net.NoRouteToHostException
import java.net.SocketTimeoutException
import java.net.UnknownHostException
import java.security.PublicKey
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ensureActive
import kotlinx.coroutines.withContext
import net.schmizz.sshj.DefaultConfig
import net.schmizz.sshj.SSHClient
import net.schmizz.sshj.connection.channel.direct.Session
import net.schmizz.sshj.transport.verification.HostKeyVerifier
import net.schmizz.sshj.userauth.UserAuthException

/** Tujuan koneksi. Password HANYA hidup selama [SshConnector.open] (mobile/DESIGN.md bagian 6). */
data class SshTarget(val host: String, val port: Int, val username: String, val password: String) {
    override fun toString() = "SshTarget($username@$host:$port)" // password tidak pernah ikut di log
}

/** Kegagalan connect yang pesannya siap tampil. */
sealed class SshFailure(message: String) : Exception(message) {
    /** Host belum ada di known_hosts — UI tanya "Percayai?", lalu [SshConnector.trust] + connect ulang. */
    class UnknownHostKey(val info: HostKeyInfo) : SshFailure("Host ini belum pernah dipercaya.")

    /** Host key berbeda dari yang dipercaya — DIBLOKIR, tidak ada tombol "lanjut saja". */
    class HostKeyChanged(val expected: HostKeyInfo, val actual: HostKeyInfo) : SshFailure(
        "HOST KEY SERVER BERUBAH! Bisa jadi server diinstal ulang, atau ada yang menyadap koneksi (MITM). " +
            "Koneksi diblokir.",
    )

    class AuthFailed : SshFailure("Username atau password SSH salah.")

    class Connect(message: String) : SshFailure(message)
}

/**
 * Buka shell interaktif lewat sshj. Host key dicek dengan known_hosts di
 * HP DUA TAHAP: host baru -> koneksi pertama ditolak & sidik jarinya
 * dilaporkan ([SshFailure.UnknownHostKey]); UI bertanya; kalau dipercaya
 * -> [trust] lalu connect ulang. (Menunggu jawaban user DI DALAM verifier
 * sshj tidak bisa: itu thread transport dengan timeout key exchange.)
 */
class SshConnector(private val knownHosts: KnownHostsStore) {

    /** Shell interaktif (terminal) dengan PTY [columns]×[rows]. */
    suspend fun open(target: SshTarget, columns: Int, rows: Int): ShellChannel = withContext(Dispatchers.IO) {
        val client = connectAuthenticated(target)
        try {
            val session = client.startSession()
            session.allocatePTY(TERM, columns, rows, 0, 0, emptyMap())
            val shell = session.startShell()
            ensureActive() // dibatalkan waktu connect -> jangan tinggalkan koneksi menggantung
            SshShell(client, session, shell)
        } catch (e: Throwable) {
            runCatching { client.disconnect() }
            throw if (e is IOException) SshFailure.Connect(describe(e, target)) else e
        }
    }

    /** Subsistem SFTP — aturan host key & autentikasi SAMA PERSIS dengan [open]. */
    suspend fun openSftp(target: SshTarget): SftpChannel = withContext(Dispatchers.IO) {
        val client = connectAuthenticated(target)
        try {
            val sftp = client.newSFTPClient()
            ensureActive()
            SftpChannel(client, sftp)
        } catch (e: Throwable) {
            runCatching { client.disconnect() }
            throw if (e is IOException) SshFailure.Connect(describe(e, target)) else e
        }
    }

    /** Connect + cek host key (known_hosts) + login password. BLOKING — panggil di Dispatchers.IO. */
    private suspend fun connectAuthenticated(target: SshTarget): SSHClient {
        val id = knownHostId(target.host, target.port)
        val stored = knownHosts.get(id)
        var check: HostKeyCheck? = null

        val client = SSHClient(DefaultConfig())
        client.addHostKeyVerifier(object : HostKeyVerifier {
            override fun verify(hostname: String, port: Int, key: PublicKey): Boolean {
                val result = checkHostKey(stored, hostKeyInfo(key))
                check = result
                return result == HostKeyCheck.Trusted
            }

            // Minta server menyodorkan jenis kunci yang sudah dipercaya.
            override fun findExistingAlgorithms(hostname: String, port: Int): List<String> = listOfNotNull(stored?.algorithm)
        })
        client.connectTimeout = CONNECT_TIMEOUT_MS

        try {
            try {
                client.connect(target.host, target.port)
            } catch (e: IOException) {
                throw when (val c = check) {
                    is HostKeyCheck.Unknown -> SshFailure.UnknownHostKey(c.actual)
                    is HostKeyCheck.Changed -> SshFailure.HostKeyChanged(c.expected, c.actual)
                    else -> SshFailure.Connect(describe(e, target))
                }
            }
            try {
                client.authPassword(target.username, target.password)
            } catch (e: UserAuthException) {
                throw SshFailure.AuthFailed()
            }
            // Tanpa keepalive, NAT/router rumahan memutus sesi yang diam beberapa menit.
            client.connection.keepAlive.keepAliveInterval = KEEPALIVE_SECONDS
            return client
        } catch (e: Throwable) {
            runCatching { client.disconnect() }
            throw if (e is IOException) SshFailure.Connect(describe(e, target)) else e
        }
    }

    /** User memilih "Percayai" di dialog sidik jari. */
    suspend fun trust(host: String, port: Int, info: HostKeyInfo) = knownHosts.put(knownHostId(host, port), info)

    /** User menghapus host key lama secara eksplisit (setelah [SshFailure.HostKeyChanged]). */
    suspend fun forget(host: String, port: Int) = knownHosts.remove(knownHostId(host, port))

    private fun describe(e: IOException, target: SshTarget): String = when (e) {
        is UnknownHostException -> "Host \"${target.host}\" tidak ditemukan."
        is ConnectException -> "Koneksi ke ${target.host}:${target.port} ditolak (server SSH mati / port salah?)."
        is NoRouteToHostException -> "Tidak ada rute ke ${target.host}."
        is SocketTimeoutException -> "Waktu habis menghubungi ${target.host}:${target.port}."
        else -> "Gagal terhubung: ${e.message ?: e.javaClass.simpleName}"
    }

    companion object {
        const val TERM = "xterm-256color"
        private const val CONNECT_TIMEOUT_MS = 15_000
        private const val KEEPALIVE_SECONDS = 30
    }
}

private class SshShell(
    private val client: SSHClient,
    private val session: Session,
    private val shell: Session.Shell,
) : ShellChannel {
    override val output: InputStream = shell.inputStream

    override fun write(data: ByteArray) {
        shell.outputStream.write(data)
        shell.outputStream.flush()
    }

    override fun resize(columns: Int, rows: Int) = shell.changeWindowDimensions(columns, rows, 0, 0)

    override fun close() {
        runCatching { shell.close() }
        runCatching { session.close() }
        runCatching { client.disconnect() }
    }
}
