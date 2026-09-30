package org.terminus.mobile.ui.account

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.toggleable
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.ListItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.RadioButton
import androidx.compose.material3.Slider
import androidx.compose.material3.SliderDefaults
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import kotlin.math.roundToInt
import kotlinx.coroutines.launch
import org.terminus.mobile.AppContainer
import org.terminus.mobile.R
import org.terminus.mobile.api.Account
import org.terminus.mobile.api.ApiSession
import org.terminus.mobile.terminal.FontSize
import org.terminus.mobile.terminal.TerminalTheme
import org.terminus.mobile.terminal.TerminalThemes
import org.terminus.mobile.ui.lock.authenticate
import org.terminus.mobile.ui.lock.deviceAuthUnavailableReason

/**
 * Tab Akun (mobile/DESIGN.md bagian 7): identitas, Profil & keamanan,
 * Pengaturan (kunci app, ukuran font, tema terminal), Logout.
 */
@Composable
fun AccountScreen(
    account: Account,
    serverUrl: String,
    session: ApiSession,
    container: AppContainer,
    snackbar: SnackbarHostState,
    onLogout: () -> Unit,
) {
    var profileOpen by rememberSaveable { mutableStateOf(false) }
    var confirmLogout by rememberSaveable { mutableStateOf(false) }
    val settings by container.settings.collectAsStateWithLifecycle()
    val context = LocalContext.current
    val scope = rememberCoroutineScope()

    if (profileOpen) {
        ProfileScreen(
            account = account,
            session = session,
            onAccountUpdated = container.sessionManager::updateAccount,
            onClose = { profileOpen = false },
        )
        return
    }

    /** Menyalakan ATAU mematikan kunci wajib lolos autentikasi dulu. */
    fun toggleLock(enable: Boolean) {
        deviceAuthUnavailableReason(context)?.let { reason ->
            scope.launch { snackbar.showSnackbar(reason) }
            return
        }
        authenticate(
            context,
            title = if (enable) "Aktifkan kunci aplikasi" else "Matikan kunci aplikasi",
            subtitle = "Konfirmasi dengan sidik jari/wajah atau kunci layar HP",
            onSuccess = { container.setAppLock(enable) },
            onError = { message -> scope.launch { snackbar.showSnackbar(message) } },
        )
    }

    Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState())) {
        Column(Modifier.padding(start = 24.dp, end = 24.dp, top = 24.dp, bottom = 8.dp)) {
            Text(account.displayName, style = MaterialTheme.typography.headlineSmall)
            if (account.displayName != account.email) {
                Text(account.email, color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
            Text(serverUrl, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.outline)
        }
        ListItem(
            headlineContent = { Text("Profil & keamanan") },
            supportingContent = { Text("Nama, email, ganti password") },
            leadingContent = { Icon(painterResource(R.drawable.ic_user), contentDescription = null) },
            trailingContent = { Icon(painterResource(R.drawable.ic_chevron_right), contentDescription = null) },
            modifier = Modifier.clickable { profileOpen = true },
        )

        HorizontalDivider(Modifier.padding(vertical = 8.dp))
        SectionTitle("Pengaturan")

        val current = settings
        if (current != null) {
            ListItem(
                headlineContent = { Text("Kunci aplikasi") },
                supportingContent = { Text("Minta sidik jari/wajah atau kunci layar HP waktu dibuka & setelah 5 menit di background.") },
                trailingContent = { Switch(checked = current.appLock, onCheckedChange = null) },
                modifier = Modifier.toggleable(value = current.appLock, role = Role.Switch, onValueChange = ::toggleLock),
            )
            FontSizeSetting(container.fontSize, TerminalThemes.resolve(null, current.terminalTheme))
            ThemeSetting(current.terminalTheme, onSelect = container::setTerminalTheme)
        }

        OutlinedButton(
            onClick = { confirmLogout = true },
            colors = ButtonDefaults.outlinedButtonColors(contentColor = MaterialTheme.colorScheme.error),
            modifier = Modifier.fillMaxWidth().padding(24.dp),
        ) { Text("Logout") }
    }

    if (confirmLogout) {
        AlertDialog(
            onDismissRequest = { confirmLogout = false },
            title = { Text("Logout dari server?") },
            text = { Text("Semua sesi SSH & SFTP akan ditutup, dan membuka aplikasi berikutnya perlu login lagi.") },
            confirmButton = {
                TextButton(onClick = { confirmLogout = false; onLogout() }) {
                    Text("Logout", color = MaterialTheme.colorScheme.error)
                }
            },
            dismissButton = { TextButton(onClick = { confirmLogout = false }) { Text("Batal") } },
        )
    }
}

@Composable
private fun SectionTitle(text: String) {
    Text(
        text,
        style = MaterialTheme.typography.titleSmall,
        color = MaterialTheme.colorScheme.primary,
        modifier = Modifier.padding(horizontal = 24.dp, vertical = 8.dp),
    )
}

/** Slider ukuran font + pratinjau. Disimpan waktu slider dilepas (bukan tiap geser). */
@Composable
private fun FontSizeSetting(fontSize: FontSize, theme: TerminalTheme) {
    var dragging by remember { mutableFloatStateOf(fontSize.sp) }
    var active by remember { mutableStateOf(false) }
    val shown = if (active) dragging else fontSize.sp

    Column(Modifier.padding(horizontal = 24.dp, vertical = 8.dp)) {
        Text("Ukuran font terminal: ${shown.roundToInt()} sp (juga bisa dicubit di layar terminal)")
        Slider(
            value = shown,
            onValueChange = { active = true; dragging = it.roundToInt().toFloat() },
            onValueChangeFinished = { fontSize.set(dragging); active = false },
            valueRange = FontSize.MIN_SP..FontSize.MAX_SP,
            steps = (FontSize.MAX_SP - FontSize.MIN_SP).toInt() - 1,
            // Track kosong bawaan = secondaryContainer = hijau di tema ini
            // (sama masalahnya dengan ProgressBar) -> netral.
            colors = SliderDefaults.colors(
                inactiveTrackColor = MaterialTheme.colorScheme.surfaceContainerHighest,
                inactiveTickColor = MaterialTheme.colorScheme.outline,
                activeTickColor = MaterialTheme.colorScheme.onPrimary,
            ),
        )
        TerminalPreview(theme, shown)
    }
}

/** Contoh tampilan terminal dengan tema & ukuran font tertentu. */
@Composable
private fun TerminalPreview(theme: TerminalTheme, fontSp: Float) {
    val fg = Color(theme.foreground)
    val text = buildAnnotatedString {
        withStyle(SpanStyle(color = Color(theme.ansi[10]))) { append("tester@server") }
        withStyle(SpanStyle(color = fg)) { append(":") }
        withStyle(SpanStyle(color = Color(theme.ansi[12]))) { append("~") }
        withStyle(SpanStyle(color = fg)) { append("$ ls\n") }
        withStyle(SpanStyle(color = Color(theme.ansi[12]))) { append("dokumen  ") }
        withStyle(SpanStyle(color = Color(theme.ansi[10]))) { append("skrip.sh  ") }
        withStyle(SpanStyle(color = Color(theme.ansi[9]))) { append("error.log") }
    }
    Text(
        text,
        fontFamily = FontFamily.Monospace,
        fontSize = fontSp.sp,
        lineHeight = (fontSp * 1.3f).sp,
        maxLines = 2,
        modifier = Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(8.dp))
            .background(Color(theme.background))
            .padding(12.dp),
    )
}

/** Pilihan tema default (dipakai host yang tidak punya tema sendiri dari desktop). */
@Composable
private fun ThemeSetting(selected: String, onSelect: (String) -> Unit) {
    Column(Modifier.padding(vertical = 8.dp)) {
        Text("Tema terminal", modifier = Modifier.padding(horizontal = 24.dp))
        Text(
            "Dipakai untuk host yang belum punya tema sendiri (tema per host diatur dari desktop).",
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.padding(horizontal = 24.dp),
        )
        TerminalThemes.all.forEach { theme ->
            Row(
                Modifier
                    .fillMaxWidth()
                    .selectable(selected = theme.name == selected, role = Role.RadioButton, onClick = { onSelect(theme.name) })
                    .padding(horizontal = 16.dp, vertical = 4.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                RadioButton(selected = theme.name == selected, onClick = null)
                Text(theme.name, modifier = Modifier.weight(1f))
                ThemeSwatch(theme)
            }
        }
    }
}

/** Latar + warna teks + 6 warna ANSI terang dalam lingkaran kecil. */
@Composable
private fun ThemeSwatch(theme: TerminalTheme) {
    Row(
        Modifier
            .clip(RoundedCornerShape(6.dp))
            .background(Color(theme.background))
            .border(1.dp, MaterialTheme.colorScheme.outlineVariant, RoundedCornerShape(6.dp))
            .padding(horizontal = 8.dp, vertical = 6.dp),
        horizontalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        (listOf(theme.foreground) + (9..14).map { theme.ansi[it] }).forEach { color ->
            Box(Modifier.size(10.dp).clip(CircleShape).background(Color(color)))
        }
    }
}
