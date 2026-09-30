package org.terminus.mobile.ui.common

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.dp
import org.terminus.mobile.ssh.HostKeyInfo

// Tampilan status koneksi SSH yang SAMA untuk terminal & SFTP — terutama
// pesan keamanan host key (mobile/DESIGN.md bagian 6) harus identik.

/** Kotak pesan status + tombol aksi di kanan bawah. */
@Composable
fun StatusBanner(message: String, error: Boolean = false, actions: List<Pair<String, () -> Unit>> = emptyList()) {
    Surface(
        color = if (error) MaterialTheme.colorScheme.errorContainer else MaterialTheme.colorScheme.surfaceVariant,
        modifier = Modifier.fillMaxWidth().padding(8.dp),
        shape = RoundedCornerShape(8.dp),
    ) {
        Column(Modifier.padding(12.dp)) {
            Text(
                message,
                color = if (error) MaterialTheme.colorScheme.onErrorContainer else MaterialTheme.colorScheme.onSurfaceVariant,
                style = MaterialTheme.typography.bodyMedium,
            )
            if (actions.isNotEmpty()) {
                Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.End) {
                    actions.forEach { (label, onClick) -> TextButton(onClick = onClick) { Text(label) } }
                }
            }
        }
    }
}

/** Host baru: tampilkan sidik jari, "Percayai & sambungkan" atau batal. */
@Composable
fun UntrustedHostKeyDialog(target: String, info: HostKeyInfo, onTrust: () -> Unit, onCancel: () -> Unit) {
    AlertDialog(
        onDismissRequest = {},
        title = { Text("Host belum dikenal") },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text("Ini pertama kali terhubung ke $target. Pastikan sidik jari di bawah SAMA dengan milik server sebelum mempercayainya.")
                Text(info.algorithm, style = MaterialTheme.typography.labelMedium)
                Text(info.fingerprint, fontFamily = FontFamily.Monospace, style = MaterialTheme.typography.bodySmall)
            }
        },
        confirmButton = { TextButton(onClick = onTrust) { Text("Percayai & sambungkan") } },
        dismissButton = { TextButton(onClick = onCancel) { Text("Batal") } },
    )
}

/**
 * Host key BERUBAH: koneksi diblokir. Satu-satunya jalan = "Hapus kunci
 * lama" (dengan konfirmasi), lalu kunci baru diperiksa lagi.
 */
@Composable
fun HostKeyChangedBanner(
    target: String,
    expected: HostKeyInfo,
    actual: HostKeyInfo,
    closeLabel: String,
    onForget: () -> Unit,
    onClose: () -> Unit,
) {
    var confirmForget by remember { mutableStateOf(false) }
    StatusBanner(
        "HOST KEY $target BERUBAH — koneksi diblokir. Bisa jadi server diinstal ulang, atau ada yang menyadap " +
            "koneksi (MITM). Hapus kunci lama HANYA kalau kamu yakin servernya memang berganti.\n\n" +
            "Tersimpan: ${expected.algorithm} ${expected.fingerprint}\nSekarang: ${actual.algorithm} ${actual.fingerprint}",
        error = true,
        actions = listOf("Hapus kunci lama" to { confirmForget = true }, closeLabel to onClose),
    )
    if (confirmForget) {
        ConfirmDialog(
            title = "Hapus host key lama?",
            text = "Koneksi berikutnya ke $target akan menampilkan sidik jari baru untuk diperiksa. " +
                "Lakukan ini hanya kalau kamu yakin servernya memang berganti kunci.",
            confirmLabel = "Hapus",
            onConfirm = { confirmForget = false; onForget() },
            onDismiss = { confirmForget = false },
        )
    }
}
