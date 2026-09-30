package org.terminus.mobile.ui.common

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import org.terminus.mobile.R

/**
 * Bar atas layar form/drill-down. `windowInsets` nol karena Scaffold
 * induk (`TerminusApp`) sudah memberi padding status bar.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SubScreenTopBar(title: String, onBack: () -> Unit, actions: @Composable () -> Unit = {}) {
    TopAppBar(
        title = { Text(title, maxLines = 1) },
        navigationIcon = {
            IconButton(onClick = onBack) { Icon(painterResource(R.drawable.ic_back), contentDescription = "Kembali") }
        },
        actions = { actions() },
        windowInsets = WindowInsets(0),
    )
}

/** Tombol Simpan di bar atas form — berubah jadi spinner selama menyimpan. */
@Composable
fun SaveAction(busy: Boolean, onSave: () -> Unit) {
    if (busy) {
        CircularProgressIndicator(Modifier.padding(horizontal = 16.dp).size(24.dp), strokeWidth = 2.dp)
    } else {
        TextButton(onClick = onSave) { Text("Simpan") }
    }
}

/** Dialog konfirmasi; [destructive] = tombol konfirmasi berwarna error. */
@Composable
fun ConfirmDialog(
    title: String,
    text: String,
    confirmLabel: String,
    onConfirm: () -> Unit,
    onDismiss: () -> Unit,
    destructive: Boolean = true,
) {
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(title) },
        text = { Text(text) },
        confirmButton = {
            TextButton(onClick = onConfirm) {
                Text(confirmLabel, color = if (destructive) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.primary)
            }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text("Batal") } },
    )
}

/**
 * Dialog satu isian teks (nama grup). [error] & [busy] dikelola pemanggil
 * karena simpannya ke server (async).
 */
@Composable
fun TextInputDialog(
    title: String,
    label: String,
    initial: String,
    confirmLabel: String,
    busy: Boolean,
    error: String?,
    onConfirm: (String) -> Unit,
    onDismiss: () -> Unit,
) {
    var value by rememberSaveable { mutableStateOf(initial) }
    AlertDialog(
        onDismissRequest = { if (!busy) onDismiss() },
        title = { Text(title) },
        text = {
            OutlinedTextField(
                value = value,
                onValueChange = { value = it },
                label = { Text(label) },
                singleLine = true,
                enabled = !busy,
                isError = error != null,
                supportingText = error?.let { { Text(it) } },
                keyboardOptions = KeyboardOptions(imeAction = ImeAction.Done),
                modifier = Modifier.fillMaxWidth(),
            )
        },
        confirmButton = {
            if (busy) {
                CircularProgressIndicator(Modifier.size(24.dp), strokeWidth = 2.dp)
            } else {
                TextButton(onClick = { onConfirm(value) }) { Text(confirmLabel) }
            }
        },
        dismissButton = { TextButton(onClick = onDismiss, enabled = !busy) { Text("Batal") } },
    )
}

/** Field password dengan tombol Lihat/Sembunyikan. */
@Composable
fun PasswordField(
    value: String,
    onValueChange: (String) -> Unit,
    label: String,
    modifier: Modifier = Modifier,
    supportingText: String? = null,
    enabled: Boolean = true,
) {
    var visible by remember { mutableStateOf(false) }
    OutlinedTextField(
        value = value,
        onValueChange = onValueChange,
        label = { Text(label) },
        singleLine = true,
        enabled = enabled,
        visualTransformation = if (visible) VisualTransformation.None else PasswordVisualTransformation(),
        keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Password, autoCorrectEnabled = false),
        trailingIcon = {
            TextButton(onClick = { visible = !visible }) { Text(if (visible) "Sembunyikan" else "Lihat") }
        },
        supportingText = supportingText?.let { { Text(it) } },
        modifier = modifier.fillMaxWidth(),
    )
}

/**
 * Pilihan satu dari daftar (grup host, identity) dalam bentuk field
 * read-only + menu. Sengaja tidak memakai `ExposedDropdownMenuBox` —
 * API anchor-nya masih berganti antar versi Material 3.
 */
@Composable
fun <T> SelectField(
    label: String,
    options: List<Pair<T, String>>,
    selected: T,
    onSelect: (T) -> Unit,
    modifier: Modifier = Modifier,
    enabled: Boolean = true,
    placeholder: String = "",
) {
    var expanded by remember { mutableStateOf(false) }
    val selectedLabel = options.firstOrNull { it.first == selected }?.second ?: placeholder
    Box(modifier.fillMaxWidth()) {
        OutlinedTextField(
            value = selectedLabel,
            onValueChange = {},
            readOnly = true,
            enabled = enabled,
            label = { Text(label) },
            trailingIcon = { Text("▾", style = MaterialTheme.typography.titleMedium) },
            modifier = Modifier.fillMaxWidth(),
        )
        // Lapisan transparan: TextField read-only tetap "memakan" ketukan
        // untuk fokus, jadi ketukan ditangkap di atasnya.
        Box(Modifier.matchParentSize().clickable(enabled = enabled) { expanded = true })
        DropdownMenu(expanded = expanded, onDismissRequest = { expanded = false }) {
            options.forEach { (value, text) ->
                DropdownMenuItem(text = { Text(text) }, onClick = { expanded = false; onSelect(value) })
            }
        }
    }
}

/** Satu aksi di menu baris (⋮ / tahan lama). */
data class RowAction(val label: String, val destructive: Boolean = false, val onClick: () -> Unit)

/** Menu aksi baris. [expanded] dikelola pemanggil supaya tahan-lama bisa ikut membukanya. */
@Composable
fun RowActionsMenu(expanded: Boolean, onExpandedChange: (Boolean) -> Unit, actions: List<RowAction>) {
    Box {
        IconButton(onClick = { onExpandedChange(true) }) {
            Icon(painterResource(R.drawable.ic_more), contentDescription = "Aksi lain")
        }
        DropdownMenu(expanded = expanded, onDismissRequest = { onExpandedChange(false) }) {
            actions.forEach { action ->
                DropdownMenuItem(
                    text = {
                        Text(
                            action.label,
                            color = if (action.destructive) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurface,
                        )
                    },
                    onClick = { onExpandedChange(false); action.onClick() },
                )
            }
        }
    }
}

/** Isi kosong / gagal muat di tengah layar. */
@Composable
fun CenterMessage(title: String, body: String? = null, action: Pair<String, () -> Unit>? = null) {
    Column(
        Modifier.fillMaxSize().padding(32.dp),
        verticalArrangement = Arrangement.Center,
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Text(title, style = MaterialTheme.typography.titleMedium, textAlign = TextAlign.Center)
        body?.let {
            Text(
                it,
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                textAlign = TextAlign.Center,
                modifier = Modifier.padding(top = 8.dp),
            )
        }
        action?.let { (label, onClick) ->
            TextButton(onClick = onClick, modifier = Modifier.padding(top = 8.dp)) { Text(label) }
        }
    }
}

/** Pesan error kecil di atas daftar (isi lama tetap tampil di bawahnya). */
@Composable
fun ErrorStrip(message: String, onRetry: () -> Unit) {
    Row(
        Modifier
            .fillMaxWidth()
            .padding(horizontal = 16.dp, vertical = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            message,
            color = MaterialTheme.colorScheme.error,
            style = MaterialTheme.typography.bodySmall,
            modifier = Modifier.weight(1f),
        )
        TextButton(onClick = onRetry) { Text("Coba lagi") }
    }
}

/**
 * Bar progres dengan track NETRAL. Track bawaan Material memakai
 * `secondaryContainer` = hijau "sukses" di tema ini — bar 6% jadi terlihat
 * hampir penuh. [progress] null = tak tentu (animasi).
 */
@Composable
fun ProgressBar(modifier: Modifier = Modifier, progress: (() -> Float)? = null) {
    val track = MaterialTheme.colorScheme.surfaceContainerHighest
    if (progress == null) {
        LinearProgressIndicator(modifier.fillMaxWidth(), trackColor = track)
    } else {
        LinearProgressIndicator(progress = progress, modifier = modifier.fillMaxWidth(), trackColor = track)
    }
}
