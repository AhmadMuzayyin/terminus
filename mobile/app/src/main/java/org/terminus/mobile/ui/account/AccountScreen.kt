package org.terminus.mobile.ui.account

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import org.terminus.mobile.api.Account

/**
 * Tab Akun — Milestone 2 cuma identitas akun + Logout. Profile (ubah
 * nama/email/password) & Pengaturan menyusul di Milestone 6.
 */
@Composable
fun AccountScreen(account: Account, serverUrl: String, onLogout: () -> Unit) {
    var confirmLogout by rememberSaveable { mutableStateOf(false) }

    Column(Modifier.fillMaxSize().padding(24.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Text(account.displayName, style = MaterialTheme.typography.headlineSmall)
        if (account.displayName != account.email) {
            Text(account.email, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        Text(serverUrl, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.outline)
        OutlinedButton(
            onClick = { confirmLogout = true },
            colors = ButtonDefaults.outlinedButtonColors(contentColor = MaterialTheme.colorScheme.error),
            modifier = Modifier.fillMaxWidth().padding(top = 16.dp),
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
