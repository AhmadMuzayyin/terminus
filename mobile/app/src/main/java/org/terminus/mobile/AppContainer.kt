package org.terminus.mobile

import android.content.Context
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.launch
import org.terminus.mobile.api.HttpClient
import org.terminus.mobile.auth.AuthState
import org.terminus.mobile.auth.DataStoreConfigStore
import org.terminus.mobile.auth.KeystoreTokenStore
import org.terminus.mobile.auth.SessionManager
import org.terminus.mobile.sftp.SftpManager
import org.terminus.mobile.ssh.DataStoreKnownHostsStore
import org.terminus.mobile.ssh.SshConnector
import org.terminus.mobile.terminal.FontSize
import org.terminus.mobile.terminal.StickyModifiers
import org.terminus.mobile.terminal.TerminalEngine
import org.terminus.mobile.terminal.TerminalService
import org.terminus.mobile.terminal.TerminalSessions
import org.terminus.mobile.terminal.TermuxEngine

/** Wiring manual semua dependency (tanpa framework DI — mobile/DESIGN.md bagian 3). */
class AppContainer(context: Context) {
    /** Scope seumur proses: login/logout tidak boleh batal cuma karena layar berganti. */
    val appScope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)

    val httpClient = HttpClient()

    val sessionManager = SessionManager(
        client = httpClient,
        tokenStore = KeystoreTokenStore(context),
        configStore = DataStoreConfigStore(context),
        scope = appScope,
    )

    val terminalEngine: TerminalEngine = TermuxEngine(context.applicationContext)

    /** Ctrl/Alt tempel & ukuran font — satu untuk semua sesi, seumur proses. */
    val modifiers = StickyModifiers()
    val fontSize = FontSize()

    /** Satu konektor (satu known_hosts) untuk terminal & SFTP — aturan host key identik. */
    private val sshConnector = SshConnector(DataStoreKnownHostsStore(context))

    val terminalSessions = TerminalSessions(
        engine = terminalEngine,
        connector = sshConnector,
        scope = appScope,
        onSessionsStarted = { TerminalService.start(context.applicationContext) },
    )

    val sftp = SftpManager(
        connector = sshConnector,
        scope = appScope,
        onStarted = { TerminalService.start(context.applicationContext) },
    )

    init {
        // Logout / sesi login berakhir -> semua sesi SSH & SFTP ditutup (mobile/DESIGN.md bagian 6).
        appScope.launch {
            sessionManager.state.collect {
                if (it !is AuthState.LoggedIn) {
                    terminalSessions.closeAll()
                    sftp.disconnect()
                }
            }
        }
    }
}
