package org.terminus.mobile.terminal

import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import org.terminus.mobile.api.Host
import org.terminus.mobile.ssh.HostKeyInfo
import org.terminus.mobile.ssh.SshConnector
import org.terminus.mobile.ssh.SshFailure
import org.terminus.mobile.ssh.SshTarget

/** Status satu sesi terminal — menentukan banner/dialog di layar Terminal. */
sealed interface SessionStatus {
    data object Connecting : SessionStatus

    data object Connected : SessionStatus

    /** Host baru: tampilkan sidik jari, tunggu "Percayai". */
    data class UntrustedHostKey(val info: HostKeyInfo) : SessionStatus

    /** Host key berubah: DIBLOKIR; satu-satunya jalan = hapus kunci lama secara eksplisit. */
    data class HostKeyChanged(val expected: HostKeyInfo, val actual: HostKeyInfo) : SessionStatus

    data object AuthFailed : SessionStatus

    /** Gagal connect (server mati, alamat salah, …). */
    data class Failed(val message: String) : SessionStatus

    /** Sempat tersambung, lalu berakhir (exit, jaringan putus). Layar & riwayat tetap. */
    data class Disconnected(val message: String) : SessionStatus
}

/** Satu tab sesi. [host] = salinan saat dibuka (dipakai sambung ulang). */
class TerminalTab internal constructor(val id: Long, val title: String, host: Host, val screen: EmulatorSession) {
    var host: Host = host
        internal set

    internal val statusFlow = MutableStateFlow<SessionStatus>(SessionStatus.Connecting)
    val status: StateFlow<SessionStatus> = statusFlow.asStateFlow()

    internal var username: String = ""

    /**
     * Password HANYA disimpan selama menunggu jawaban dialog host key (koneksi
     * pertama berhenti sebelum autentikasi). Dibuang begitu connect berikutnya dimulai.
     */
    internal var pendingPassword: String? = null
    internal var job: Job? = null
}

/**
 * Semua sesi SSH yang terbuka, seumur PROSES (bukan layar): pindah tab,
 * rotasi, app ke background tidak memutus sesi (mobile/DESIGN.md bagian 7).
 * Semua fungsi dipanggil dari main thread; [scope] = scope aplikasi (Main).
 *
 * @param onSessionsStarted dipanggil tiap sesi dibuka — menyalakan
 *   foreground service (service mematikan dirinya sendiri kalau 0 sesi).
 */
class TerminalSessions(
    private val engine: TerminalEngine,
    private val connector: SshConnector,
    private val scope: CoroutineScope,
    private val onSessionsStarted: () -> Unit,
) {
    private val _tabs = MutableStateFlow<List<TerminalTab>>(emptyList())
    val tabs: StateFlow<List<TerminalTab>> = _tabs.asStateFlow()

    private val _activeId = MutableStateFlow<Long?>(null)
    val activeId: StateFlow<Long?> = _activeId.asStateFlow()

    private var nextId = 1L

    fun select(id: Long) {
        if (_tabs.value.any { it.id == id }) _activeId.value = id
    }

    /** Buka tab baru ke [host] & langsung connect. */
    fun open(host: Host, username: String, password: String): TerminalTab {
        val sameHost = _tabs.value.count { it.host.id == host.id }
        val title = if (sameHost == 0) host.label else "${host.label} (${sameHost + 1})"
        val tab = TerminalTab(nextId++, title, host, engine.newSession())
        tab.screen.onChannelEnded = { error ->
            val message = if (error == null) "Sesi ditutup server." else "Koneksi terputus: ${error.message ?: error.javaClass.simpleName}"
            tab.screen.printNotice(message)
            tab.statusFlow.value = SessionStatus.Disconnected(message)
        }
        _tabs.value = _tabs.value + tab
        _activeId.value = tab.id
        onSessionsStarted()
        connect(tab, username, password)
        return tab
    }

    /** Sambung ulang di tab yang sama (layar & riwayat dipertahankan). [host] = data terbaru dari server. */
    fun reconnect(tab: TerminalTab, host: Host, username: String, password: String) {
        if (tab !in _tabs.value) return
        tab.host = host
        connect(tab, username, password)
    }

    /** User memilih "Percayai" di dialog sidik jari -> simpan & connect ulang. */
    fun trustHostKey(tab: TerminalTab) {
        val status = tab.status.value as? SessionStatus.UntrustedHostKey ?: return
        val password = tab.pendingPassword ?: return
        scope.launch {
            connector.trust(tab.host.host, tab.host.port, status.info)
            connect(tab, tab.username, password)
        }
    }

    /** Hapus host key lama (setelah peringatan "BERUBAH") — connect berikutnya akan bertanya lagi. */
    fun forgetHostKey(tab: TerminalTab) {
        scope.launch {
            connector.forget(tab.host.host, tab.host.port)
            tab.statusFlow.value = SessionStatus.Failed("Host key lama dihapus. Sambung ulang untuk memeriksa kunci yang baru.")
        }
    }

    fun close(tab: TerminalTab) {
        tab.job?.cancel()
        tab.pendingPassword = null
        tab.screen.dispose()
        val remaining = _tabs.value - tab
        _tabs.value = remaining
        if (_activeId.value == tab.id) _activeId.value = remaining.lastOrNull()?.id
    }

    /** Logout: semua sesi ditutup (mobile/DESIGN.md bagian 6). */
    fun closeAll() {
        _tabs.value.forEach { close(it) }
    }

    private fun connect(tab: TerminalTab, username: String, password: String) {
        tab.job?.cancel()
        tab.screen.detach()
        tab.username = username
        tab.pendingPassword = null
        tab.statusFlow.value = SessionStatus.Connecting
        tab.job = scope.launch {
            val target = SshTarget(tab.host.host, tab.host.port, username, password)
            tab.statusFlow.value = try {
                val channel = connector.open(target, tab.screen.columns, tab.screen.rows)
                tab.screen.attach(channel)
                SessionStatus.Connected
            } catch (e: SshFailure.UnknownHostKey) {
                tab.pendingPassword = password
                SessionStatus.UntrustedHostKey(e.info)
            } catch (e: SshFailure.HostKeyChanged) {
                SessionStatus.HostKeyChanged(e.expected, e.actual)
            } catch (e: SshFailure.AuthFailed) {
                SessionStatus.AuthFailed
            } catch (e: SshFailure) {
                SessionStatus.Failed(e.message ?: "Gagal terhubung.")
            } catch (e: CancellationException) {
                throw e
            } catch (e: Exception) {
                // sshj bisa melempar exception runtime (mis. algoritma tidak didukung).
                SessionStatus.Failed("Gagal terhubung: ${e.message ?: e.javaClass.simpleName}")
            }
        }
    }
}
