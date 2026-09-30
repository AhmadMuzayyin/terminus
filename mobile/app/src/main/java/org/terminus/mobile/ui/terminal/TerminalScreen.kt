package org.terminus.mobile.ui.terminal

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.isImeVisible
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import org.terminus.mobile.R
import org.terminus.mobile.data.address
import org.terminus.mobile.terminal.FontSize
import org.terminus.mobile.terminal.SessionStatus
import org.terminus.mobile.terminal.SpecialKey
import org.terminus.mobile.terminal.StickyModifiers
import org.terminus.mobile.terminal.TerminalEngine
import org.terminus.mobile.terminal.TerminalSessions
import org.terminus.mobile.terminal.TerminalSurface
import org.terminus.mobile.terminal.TerminalTab
import org.terminus.mobile.ui.common.HostKeyChangedBanner
import org.terminus.mobile.ui.common.ProgressBar
import org.terminus.mobile.ui.common.StatusBanner
import org.terminus.mobile.ui.common.UntrustedHostKeyDialog

/**
 * Layar Terminal layar penuh (mobile/DESIGN.md bagian 7): pengalih sesi di
 * atas, terminal, baris tombol ekstra tepat di atas keyboard. Kembali =
 * ke Hosts, sesi TETAP jalan.
 */
private const val SHORT_SCREEN_DP = 500

@OptIn(ExperimentalLayoutApi::class)
@Composable
fun TerminalScreen(
    sessions: TerminalSessions,
    engine: TerminalEngine,
    modifiers: StickyModifiers,
    fontSize: FontSize,
    connect: ConnectFlow,
    onBack: () -> Unit,
) {
    val tabs by sessions.tabs.collectAsStateWithLifecycle()
    val activeId by sessions.activeId.collectAsStateWithLifecycle()
    val active = tabs.find { it.id == activeId } ?: tabs.lastOrNull()
    val context = LocalContext.current
    val surface = remember { engine.newSurface(context, modifiers, fontSize) }

    DisposableEffect(surface, active) {
        surface.show(active?.screen)
        onDispose { }
    }
    DisposableEffect(surface) { onDispose { surface.show(null) } }
    BackHandler(onBack = onBack)

    Column(
        Modifier
            .fillMaxSize()
            .background(Color.Black)
            .windowInsetsPadding(WindowInsets.safeDrawing),
    ) {
        // Landscape + keyboard: keyboard, baris ekstra & bar sesi bisa memakan
        // seluruh tinggi layar (terminal tinggal 0–4 baris). Bar sesi disembunyikan
        // selama keyboard terbuka di layar pendek.
        val cramped = LocalConfiguration.current.screenHeightDp < SHORT_SCREEN_DP && WindowInsets.isImeVisible
        if (!cramped) {
            SessionBar(tabs, active, onSelect = { sessions.select(it.id) }, onClose = sessions::close, onBack = onBack) {
                surface.pasteClipboard()
            }
        }
        Box(Modifier.weight(1f).fillMaxWidth()) {
            AndroidView(factory = { surface.view }, modifier = Modifier.fillMaxSize())
            active?.let { StatusOverlay(it, sessions, connect) }
        }
        ExtraKeys(surface, modifiers, compact = cramped)
    }
}

@Composable
private fun SessionBar(
    tabs: List<TerminalTab>,
    active: TerminalTab?,
    onSelect: (TerminalTab) -> Unit,
    onClose: (TerminalTab) -> Unit,
    onBack: () -> Unit,
    onPaste: () -> Unit,
) {
    var menu by remember { mutableStateOf(false) }
    Row(
        Modifier.fillMaxWidth().background(MaterialTheme.colorScheme.surface).height(48.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        IconButton(onClick = onBack) { Icon(painterResource(R.drawable.ic_back), contentDescription = "Kembali ke Hosts") }
        Row(Modifier.weight(1f).horizontalScroll(rememberScrollState()), verticalAlignment = Alignment.CenterVertically) {
            tabs.forEach { tab ->
                val status by tab.status.collectAsStateWithLifecycle()
                val selected = tab.id == active?.id
                Row(
                    Modifier
                        .padding(horizontal = 2.dp)
                        .clip(RoundedCornerShape(8.dp))
                        .background(if (selected) MaterialTheme.colorScheme.surfaceVariant else Color.Transparent)
                        .clickable { onSelect(tab) }
                        .padding(start = 10.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Box(Modifier.size(8.dp).clip(CircleShape).background(statusColor(status)))
                    Text(
                        tab.title,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                        style = MaterialTheme.typography.labelLarge,
                        modifier = Modifier.padding(start = 6.dp).widthIn(max = 140.dp),
                    )
                    IconButton(onClick = { onClose(tab) }, modifier = Modifier.size(36.dp)) {
                        Icon(painterResource(R.drawable.ic_close), contentDescription = "Tutup sesi ${tab.title}", modifier = Modifier.size(16.dp))
                    }
                }
            }
        }
        Box {
            IconButton(onClick = { menu = true }) { Icon(painterResource(R.drawable.ic_more), contentDescription = "Menu terminal") }
            DropdownMenu(expanded = menu, onDismissRequest = { menu = false }) {
                DropdownMenuItem(text = { Text("Tempel") }, onClick = { menu = false; onPaste() })
                active?.let { tab ->
                    DropdownMenuItem(
                        text = { Text("Tutup sesi ini", color = MaterialTheme.colorScheme.error) },
                        onClick = { menu = false; onClose(tab) },
                    )
                }
            }
        }
    }
}

@Composable
private fun statusColor(status: SessionStatus): Color = when (status) {
    // Hijau "success" desktop (ui/theme/Theme.kt) — `secondary` tidak diset di tema.
    SessionStatus.Connected -> MaterialTheme.colorScheme.secondaryContainer
    SessionStatus.Connecting, is SessionStatus.UntrustedHostKey -> MaterialTheme.colorScheme.outline
    else -> MaterialTheme.colorScheme.error
}

/** Banner/dialog status sesi aktif di atas terminal. */
@Composable
private fun StatusOverlay(tab: TerminalTab, sessions: TerminalSessions, connect: ConnectFlow) {
    val status by tab.status.collectAsStateWithLifecycle()
    val target = "${tab.host.host}:${tab.host.port}"
    val reconnect = { connect.start(tab.host) { h, u, p -> sessions.reconnect(tab, h, u, p) } }

    when (val s = status) {
        SessionStatus.Connected -> Unit
        SessionStatus.Connecting -> Column(Modifier.fillMaxWidth()) {
            ProgressBar()
            StatusBanner("Menghubungkan ke ${tab.host.address()}…")
        }
        is SessionStatus.UntrustedHostKey -> UntrustedHostKeyDialog(
            target,
            s.info,
            onTrust = { sessions.trustHostKey(tab) },
            onCancel = { sessions.close(tab) },
        )
        is SessionStatus.HostKeyChanged -> HostKeyChangedBanner(
            target,
            s.expected,
            s.actual,
            closeLabel = "Tutup sesi",
            onForget = { sessions.forgetHostKey(tab) },
            onClose = { sessions.close(tab) },
        )
        SessionStatus.AuthFailed -> StatusBanner(
            "Username atau password SSH salah.",
            error = true,
            actions = listOf(
                "Masukkan password" to {
                    connect.start(tab.host, authFailed = true, lastUsername = tab.username) { h, u, p -> sessions.reconnect(tab, h, u, p) }
                },
                "Tutup sesi" to { sessions.close(tab) },
            ),
        )
        is SessionStatus.Failed -> StatusBanner(
            s.message,
            error = true,
            actions = listOf("Sambung ulang" to reconnect, "Tutup sesi" to { sessions.close(tab) }),
        )
        is SessionStatus.Disconnected -> Box(Modifier.fillMaxSize(), contentAlignment = Alignment.BottomCenter) {
            StatusBanner(s.message, actions = listOf("Sambung ulang" to reconnect, "Tutup sesi" to { sessions.close(tab) }))
        }
    }
}

/**
 * Baris tombol ekstra (mobile/DESIGN.md bagian 7). Ctrl/Alt "tempel": menyala
 * sampai tombol berikutnya. Tombol di sini tidak mengambil fokus, jadi
 * keyboard HP tetap terbuka.
 */
@Composable
private fun ExtraKeys(surface: TerminalSurface, modifiers: StickyModifiers, compact: Boolean) {
    val keyPadding = if (compact) 4.dp else 8.dp
    Row(
        Modifier
            .fillMaxWidth()
            .background(MaterialTheme.colorScheme.surface)
            .horizontalScroll(rememberScrollState())
            .padding(horizontal = 4.dp, vertical = if (compact) 2.dp else 4.dp),
        horizontalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        ExtraKey("ESC", keyPadding) { surface.press(SpecialKey.Escape) }
        ExtraKey("CTRL", keyPadding, active = modifiers.ctrl) { modifiers.ctrl = !modifiers.ctrl }
        ExtraKey("ALT", keyPadding, active = modifiers.alt) { modifiers.alt = !modifiers.alt }
        ExtraKey("TAB", keyPadding) { surface.press(SpecialKey.Tab) }
        ExtraKey("←", keyPadding) { surface.press(SpecialKey.Left) }
        ExtraKey("↑", keyPadding) { surface.press(SpecialKey.Up) }
        ExtraKey("↓", keyPadding) { surface.press(SpecialKey.Down) }
        ExtraKey("→", keyPadding) { surface.press(SpecialKey.Right) }
        "-/|~".forEach { c -> ExtraKey(c.toString(), keyPadding) { surface.type(c) } }
    }
}

@Composable
private fun ExtraKey(label: String, verticalPadding: Dp, active: Boolean = false, onClick: () -> Unit) {
    Box(
        Modifier
            .clip(RoundedCornerShape(6.dp))
            .background(if (active) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.surfaceVariant)
            .clickable(onClick = onClick)
            .padding(horizontal = 12.dp, vertical = verticalPadding),
        contentAlignment = Alignment.Center,
    ) {
        Text(
            label,
            fontFamily = FontFamily.Monospace,
            fontWeight = FontWeight.Medium,
            color = if (active) MaterialTheme.colorScheme.onPrimary else MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}
