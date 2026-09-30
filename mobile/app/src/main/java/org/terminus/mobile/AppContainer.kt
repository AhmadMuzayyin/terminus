package org.terminus.mobile

import android.content.Context
import android.os.SystemClock
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import org.terminus.mobile.api.HttpClient
import org.terminus.mobile.auth.AppLock
import org.terminus.mobile.auth.AuthState
import org.terminus.mobile.auth.DataStoreConfigStore
import org.terminus.mobile.auth.KeystoreTokenStore
import org.terminus.mobile.auth.SessionManager
import org.terminus.mobile.settings.AppSettings
import org.terminus.mobile.settings.DataStoreSettingsStore
import org.terminus.mobile.settings.SettingsStore
import org.terminus.mobile.sftp.SftpManager
import org.terminus.mobile.ssh.DataStoreKnownHostsStore
import org.terminus.mobile.ssh.SshConnector
import org.terminus.mobile.terminal.FontSize
import org.terminus.mobile.terminal.StickyModifiers
import org.terminus.mobile.terminal.TerminalEngine
import org.terminus.mobile.terminal.TerminalService
import org.terminus.mobile.terminal.TerminalSessions
import org.terminus.mobile.terminal.TerminalThemes
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

    private val settingsStore: SettingsStore = DataStoreSettingsStore(context)

    /** Pengaturan terakhir yang terbaca (null = belum terbaca). */
    private val _settings = MutableStateFlow<AppSettings?>(null)
    val settings: StateFlow<AppSettings?> = _settings.asStateFlow()

    val appLock = AppLock(clock = SystemClock::elapsedRealtime)

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
        themeFor = { host -> TerminalThemes.resolve(host.terminalTheme, _settings.value?.terminalTheme ?: TerminalThemes.DEFAULT_NAME) },
    )

    val sftp = SftpManager(
        connector = sshConnector,
        scope = appScope,
        onStarted = { TerminalService.start(context.applicationContext) },
    )

    init {
        // Ukuran font dari pinch/slider -> disimpan permanen.
        fontSize.onChange = { sp -> appScope.launch { settingsStore.update { it.copy(fontSizeSp = sp) } } }
        appScope.launch {
            settingsStore.settings.collect { next ->
                val previousTheme = _settings.value?.terminalTheme
                _settings.value = next
                appLock.onSettings(next.appLock)
                fontSize.load(next.fontSizeSp)
                if (previousTheme != null && previousTheme != next.terminalTheme) terminalSessions.refreshThemes()
            }
        }
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

    /** Diaktifkan HANYA setelah user lolos autentikasi (lihat AccountScreen). */
    fun setAppLock(enabled: Boolean) {
        appScope.launch { settingsStore.update { it.copy(appLock = enabled) } }
    }

    fun setTerminalTheme(name: String) {
        appScope.launch { settingsStore.update { it.copy(terminalTheme = name) } }
    }
}
