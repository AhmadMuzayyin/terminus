package org.terminus.mobile.sftp

import java.io.InputStream
import java.io.OutputStream
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import org.terminus.mobile.api.Host
import org.terminus.mobile.ssh.HostKeyInfo
import org.terminus.mobile.ssh.RemoteEntry
import org.terminus.mobile.ssh.RemoteKind
import org.terminus.mobile.ssh.SftpChannel
import org.terminus.mobile.ssh.SftpError
import org.terminus.mobile.ssh.SshConnector
import org.terminus.mobile.ssh.SshFailure
import org.terminus.mobile.ssh.SshTarget
import org.terminus.mobile.ssh.TransferCancelled

/** Status koneksi SFTP — pola sama dengan sesi terminal (host key, auth, gagal). */
sealed interface SftpStatus {
    data object Idle : SftpStatus

    data class Connecting(val host: Host) : SftpStatus

    data class UntrustedHostKey(val host: Host, val info: HostKeyInfo) : SftpStatus

    data class HostKeyChanged(val host: Host, val expected: HostKeyInfo, val actual: HostKeyInfo) : SftpStatus

    data class AuthFailed(val host: Host, val username: String) : SftpStatus

    data class Failed(val host: Host, val message: String) : SftpStatus

    data class Connected(val host: Host) : SftpStatus
}

/** Isi folder yang sedang dibuka. */
data class Listing(
    val path: String = "",
    val entries: List<RemoteEntry> = emptyList(),
    val loading: Boolean = false,
    val error: String? = null,
)

/** Transfer yang sedang jalan. [total] null = ukuran tidak diketahui. */
data class Transfer(val name: String, val upload: Boolean, val done: Long, val total: Long?)

/**
 * Satu koneksi SFTP, seumur PROSES (bukan layar): pindah tab tidak
 * memutus koneksi / transfer; dihitung foreground service seperti sesi
 * terminal. Semua fungsi publik dipanggil dari main thread.
 */
class SftpManager(
    private val connector: SshConnector,
    private val scope: CoroutineScope,
    private val onStarted: () -> Unit,
) {
    private val _status = MutableStateFlow<SftpStatus>(SftpStatus.Idle)
    val status: StateFlow<SftpStatus> = _status.asStateFlow()

    private val _listing = MutableStateFlow(Listing())
    val listing: StateFlow<Listing> = _listing.asStateFlow()

    private val _transfer = MutableStateFlow<Transfer?>(null)
    val transfer: StateFlow<Transfer?> = _transfer.asStateFlow()

    /** Pesan hasil (transfer selesai/gagal) untuk snackbar. */
    private val _events = MutableSharedFlow<String>(extraBufferCapacity = 8)
    val events: SharedFlow<String> = _events.asSharedFlow()

    private var channel: SftpChannel? = null
    private var connectJob: Job? = null
    private var listJob: Job? = null
    @Volatile private var transferCancelled = false

    /** Password HANYA selama dialog host key terbuka (sama dengan terminal). */
    private var pendingPassword: String? = null
    private var username = ""

    /** Folder terakhir per host — sambung ulang ke host yang sama kembali ke folder itu. */
    private var lastHostId: String? = null
    private var lastPath: String? = null

    val currentHost: Host? get() = hostOf(_status.value)

    fun connect(host: Host, username: String, password: String) {
        connectJob?.cancel()
        closeChannel()
        this.username = username
        pendingPassword = null
        _listing.value = Listing()
        _status.value = SftpStatus.Connecting(host)
        connectJob = scope.launch {
            _status.value = try {
                val ch = connector.openSftp(SshTarget(host.host, host.port, username, password))
                channel = ch
                onStarted()
                val start = lastPath.takeIf { lastHostId == host.id } ?: withContext(Dispatchers.IO) { ch.home() }
                lastHostId = host.id
                load(start)
                SftpStatus.Connected(host)
            } catch (e: SshFailure.UnknownHostKey) {
                pendingPassword = password
                SftpStatus.UntrustedHostKey(host, e.info)
            } catch (e: SshFailure.HostKeyChanged) {
                SftpStatus.HostKeyChanged(host, e.expected, e.actual)
            } catch (e: SshFailure.AuthFailed) {
                SftpStatus.AuthFailed(host, username)
            } catch (e: SshFailure) {
                SftpStatus.Failed(host, e.message ?: "Gagal terhubung.")
            } catch (e: SftpError) {
                closeChannel()
                SftpStatus.Failed(host, e.message ?: "Gagal membuka SFTP.")
            } catch (e: CancellationException) {
                throw e
            } catch (e: Exception) {
                SftpStatus.Failed(host, "Gagal terhubung: ${e.message ?: e.javaClass.simpleName}")
            }
        }
    }

    fun trustHostKey() {
        val s = _status.value as? SftpStatus.UntrustedHostKey ?: return
        val password = pendingPassword ?: return
        scope.launch {
            connector.trust(s.host.host, s.host.port, s.info)
            connect(s.host, username, password)
        }
    }

    fun forgetHostKey() {
        val s = _status.value as? SftpStatus.HostKeyChanged ?: return
        scope.launch {
            connector.forget(s.host.host, s.host.port)
            _status.value = SftpStatus.Failed(s.host, "Host key lama dihapus. Sambung ulang untuk memeriksa kunci yang baru.")
        }
    }

    /** Putus & kembali ke pemilih host. Logout juga memanggil ini. */
    fun disconnect() {
        connectJob?.cancel()
        closeChannel()
        pendingPassword = null
        _status.value = SftpStatus.Idle
        _listing.value = Listing()
    }

    fun open(path: String) = load(path)

    fun refresh() = load(_listing.value.path)

    fun goUp() {
        parentPath(_listing.value.path)?.let(::load)
    }

    /** Symlink: menunjuk folder? (mengikuti link di server). */
    suspend fun isDirectory(entry: RemoteEntry): Boolean = when (entry.kind) {
        RemoteKind.Directory -> true
        RemoteKind.File -> false
        RemoteKind.Link -> op { it.isDirectory(entry.path) }
    }

    suspend fun exists(name: String): Boolean = op { it.exists(joinPath(_listing.value.path, name)) }

    suspend fun mkdir(name: String) {
        op { it.mkdir(joinPath(_listing.value.path, name)) }
        refresh()
    }

    suspend fun rename(entry: RemoteEntry, newName: String) {
        op { it.rename(entry.path, joinPath(parentPath(entry.path) ?: "/", newName)) }
        refresh()
    }

    suspend fun delete(entry: RemoteEntry) {
        try {
            op { it.delete(entry) }
        } finally {
            refresh() // hapus folder bisa gagal di tengah — tampilkan isi sebenarnya
        }
    }

    /**
     * Unduh [entry] ke [openOutput] (file pilihan user lewat SAF). Gagal/
     * dibatalkan -> [onFailedCleanup] (hapus file setengah jadi di HP).
     */
    fun download(entry: RemoteEntry, openOutput: () -> OutputStream, onFailedCleanup: () -> Unit) {
        startTransfer(Transfer(entry.name, upload = false, done = 0, total = entry.size), onFailedCleanup) { ch, progress ->
            openOutput().use { out -> ch.download(entry.path, out, progress) { transferCancelled } }
        }
    }

    /** Unggah ke folder yang sedang dibuka sebagai [name] (menimpa kalau sudah ada). */
    fun upload(name: String, size: Long?, openInput: () -> InputStream) {
        val target = joinPath(_listing.value.path, name)
        val partial = RemoteEntry(name, target, RemoteKind.File, 0, 0)
        startTransfer(
            Transfer(name, upload = true, done = 0, total = size),
            // File setengah jadi di server dihapus (best-effort).
            onFailedCleanup = { runCatching { channel?.delete(partial) } },
        ) { ch, progress ->
            openInput().use { input -> ch.upload(input, target, progress) { transferCancelled } }
        }
    }

    fun cancelTransfer() {
        transferCancelled = true
    }

    private fun startTransfer(
        initial: Transfer,
        onFailedCleanup: () -> Unit,
        block: (SftpChannel, (Long) -> Unit) -> Unit,
    ) {
        val ch = channel ?: return
        if (_transfer.value != null) {
            _events.tryEmit("Tunggu transfer yang sedang jalan selesai dulu.")
            return
        }
        transferCancelled = false
        _transfer.value = initial
        scope.launch {
            val verb = if (initial.upload) "Unggah" else "Unduh"
            val message = try {
                withContext(Dispatchers.IO) {
                    var reported = 0L
                    block(ch) { done ->
                        // Jangan banjiri UI: lapor tiap ~256 KB.
                        if (done - reported >= PROGRESS_STEP) {
                            reported = done
                            _transfer.update { it?.copy(done = done) }
                        }
                    }
                }
                "$verb selesai: ${initial.name}"
            } catch (e: TransferCancelled) {
                withContext(Dispatchers.IO) { runCatching(onFailedCleanup) }
                "$verb dibatalkan: ${initial.name}"
            } catch (e: CancellationException) {
                throw e
            } catch (e: Exception) {
                withContext(Dispatchers.IO) { runCatching(onFailedCleanup) }
                checkConnection()
                "$verb gagal: ${e.message ?: e.javaClass.simpleName}"
            } finally {
                _transfer.value = null
            }
            if (initial.upload) refresh()
            _events.tryEmit(message)
        }
    }

    private fun load(path: String) {
        val ch = channel ?: return
        listJob?.cancel()
        _listing.update { it.copy(loading = true, error = null) }
        listJob = scope.launch {
            try {
                val entries = withContext(Dispatchers.IO) { ch.list(path) }
                _listing.value = Listing(path, sortEntries(entries))
                lastPath = path
            } catch (e: SftpError) {
                // Folder baru gagal dibuka -> tetap di folder lama, tampilkan pesannya.
                _listing.update { it.copy(loading = false, error = e.message) }
                checkConnection()
            }
        }
    }

    /** Jalankan operasi di IO; gagal -> cek apakah koneksinya putus, lalu lempar ulang. */
    private suspend fun <T> op(block: (SftpChannel) -> T): T {
        val ch = channel ?: throw SftpError("Tidak tersambung ke server.")
        return try {
            withContext(Dispatchers.IO) { block(ch) }
        } catch (e: SftpError) {
            checkConnection()
            throw e
        }
    }

    private fun checkConnection() {
        val ch = channel ?: return
        if (ch.isConnected) return
        val host = currentHost ?: return
        closeChannel()
        _status.value = SftpStatus.Failed(host, "Koneksi SFTP terputus.")
    }

    private fun closeChannel() {
        listJob?.cancel()
        transferCancelled = true
        val old = channel ?: return
        channel = null
        scope.launch(Dispatchers.IO) { old.close() }
    }

    private fun hostOf(status: SftpStatus): Host? = when (status) {
        SftpStatus.Idle -> null
        is SftpStatus.Connecting -> status.host
        is SftpStatus.UntrustedHostKey -> status.host
        is SftpStatus.HostKeyChanged -> status.host
        is SftpStatus.AuthFailed -> status.host
        is SftpStatus.Failed -> status.host
        is SftpStatus.Connected -> status.host
    }

    private companion object {
        const val PROGRESS_STEP = 256L * 1024
    }
}
