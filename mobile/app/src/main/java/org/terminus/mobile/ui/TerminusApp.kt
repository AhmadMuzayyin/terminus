package org.terminus.mobile.ui

import android.Manifest
import android.content.pm.PackageManager
import android.os.Build
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.consumeWindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.saveable.rememberSaveableStateHolder
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import org.terminus.mobile.AppContainer
import org.terminus.mobile.api.Account
import org.terminus.mobile.api.Host
import org.terminus.mobile.data.VaultRepository
import org.terminus.mobile.ui.account.AccountScreen
import org.terminus.mobile.ui.hosts.HostsScreen
import org.terminus.mobile.ui.identities.IdentitiesScreen
import org.terminus.mobile.ui.terminal.ConnectFlow
import org.terminus.mobile.ui.terminal.CredentialDialog
import org.terminus.mobile.ui.terminal.TerminalScreen

// Layar utama SETELAH login: bottom navigation + layar Terminal penuh di
// atasnya. Pakai state biasa (bukan navigation-compose): cuma dua tingkat.
@Composable
fun TerminusApp(
    account: Account,
    serverUrl: String,
    repository: VaultRepository,
    container: AppContainer,
    onLogout: () -> Unit,
) {
    var current by rememberSaveable { mutableStateOf(Tab.Hosts) }
    var terminalOpen by rememberSaveable { mutableStateOf(false) }
    // State tiap tab (grup yang dibuka, form setengah terisi) tetap ada
    // waktu pindah tab lalu kembali.
    val tabStates = rememberSaveableStateHolder()
    val snackbar = remember { SnackbarHostState() }
    val scope = rememberCoroutineScope()
    val sessions = container.terminalSessions
    val terminalTabs by sessions.tabs.collectAsStateWithLifecycle()
    val connect = remember(repository) {
        ConnectFlow(repository, sessions, scope, snackbar, onOpened = { terminalOpen = true })
    }
    val requestNotifications = rememberNotificationPermission()

    LaunchedEffect(repository) { repository.refresh() }
    // Sesi terakhir ditutup -> kembali ke Hosts.
    LaunchedEffect(terminalTabs.isEmpty()) { if (terminalTabs.isEmpty()) terminalOpen = false }

    val onConnect: (Host) -> Unit = { host ->
        requestNotifications()
        connect.start(host)
    }

    if (terminalOpen && terminalTabs.isNotEmpty()) {
        TerminalScreen(
            sessions = sessions,
            engine = container.terminalEngine,
            modifiers = container.modifiers,
            fontSize = container.fontSize,
            connect = connect,
            onBack = { terminalOpen = false },
        )
    } else {
        Scaffold(
            snackbarHost = { SnackbarHost(snackbar) },
            bottomBar = {
                Column {
                    if (terminalTabs.isNotEmpty()) {
                        ActiveSessionsBar(terminalTabs.size) { terminalOpen = true }
                    }
                    NavigationBar {
                        Tab.entries.forEach { tab ->
                            NavigationBarItem(
                                selected = tab == current,
                                onClick = { current = tab },
                                icon = { Icon(painterResource(tab.icon), contentDescription = null) },
                                label = { Text(stringResource(tab.label)) },
                            )
                        }
                    }
                }
            },
        ) { padding ->
            // `consumeWindowInsets`: form di dalam tab memakai `imePadding()` —
            // tanpa ini tinggi bottom bar terhitung dua kali saat keyboard muncul.
            Box(Modifier.fillMaxSize().padding(padding).consumeWindowInsets(padding)) {
                tabStates.SaveableStateProvider(current.name) {
                    when (current) {
                        Tab.Hosts -> HostsScreen(repository, snackbar, onConnect = onConnect)
                        Tab.Identities -> IdentitiesScreen(repository, snackbar)
                        Tab.Account -> AccountScreen(account, serverUrl, onLogout)
                        Tab.Sftp -> PlaceholderScreen(title = stringResource(current.label))
                    }
                }
            }
        }
    }

    connect.request?.let { request ->
        CredentialDialog(request, onSubmit = connect::submit, onDismiss = connect::dismiss)
    }
}

/** "N sesi terminal aktif" di atas bottom nav — kembali ke layar Terminal. */
@Composable
private fun ActiveSessionsBar(count: Int, onOpen: () -> Unit) {
    Surface(color = MaterialTheme.colorScheme.secondaryContainer, modifier = Modifier.fillMaxWidth().clickable(onClick = onOpen)) {
        Text(
            "$count sesi terminal aktif — ketuk untuk membuka",
            color = MaterialTheme.colorScheme.onSecondaryContainer,
            style = MaterialTheme.typography.labelLarge,
            modifier = Modifier.padding(horizontal = 16.dp, vertical = 10.dp),
        )
    }
}

/**
 * Android 13+: notifikasi foreground service ("N sesi SSH aktif") butuh izin.
 * Ditanya waktu pertama kali connect; ditolak pun sesi tetap jalan.
 */
@Composable
private fun rememberNotificationPermission(): () -> Unit {
    if (Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU) return {}
    val context = LocalContext.current
    val launcher = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { }
    var asked by rememberSaveable { mutableStateOf(false) }
    return {
        val granted = context.checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) == PackageManager.PERMISSION_GRANTED
        if (!granted && !asked) {
            asked = true
            launcher.launch(Manifest.permission.POST_NOTIFICATIONS)
        }
    }
}

@Composable
private fun PlaceholderScreen(title: String) {
    Column(
        modifier = Modifier.fillMaxSize().padding(24.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.Center,
    ) {
        Text(title, style = MaterialTheme.typography.headlineMedium)
        Text(
            "Belum diimplementasi — lihat mobile/DESIGN.md bagian 9.",
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            textAlign = TextAlign.Center,
            modifier = Modifier.padding(top = 8.dp),
        )
    }
}
