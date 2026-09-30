package org.terminus.mobile.ui.terminal

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.selection.toggleable
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Checkbox
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.launch
import org.terminus.mobile.api.ApiError
import org.terminus.mobile.api.Host
import org.terminus.mobile.data.VaultRepository
import org.terminus.mobile.data.address
import org.terminus.mobile.ui.common.PasswordField

/** Kredensial yang perlu diketik user sebelum connect. [onReady] = lanjutan setelah kredensial lengkap. */
class CredentialRequest(
    val host: Host,
    val askUsername: Boolean,
    val askPassword: Boolean,
    val initialUsername: String,
    val error: String? = null,
    val onReady: (host: Host, username: String, password: String) -> Unit,
) {
    fun withError(message: String) =
        CredentialRequest(host, askUsername, askPassword, initialUsername, message, onReady)
}

/**
 * Alur "pilih host -> kredensial lengkap" (mobile/DESIGN.md bagian 7),
 * dipakai terminal & SFTP: username & password lengkap -> password
 * diambil dari server; ada yang kosong (atau autentikasi gagal) -> tanya
 * dulu, opsi simpan ke server. Password tidak disimpan di HP — cuma
 * diteruskan ke [CredentialRequest.onReady].
 */
class ConnectFlow(
    private val repository: VaultRepository,
    private val scope: CoroutineScope,
    private val snackbar: SnackbarHostState,
) {
    var request by mutableStateOf<CredentialRequest?>(null)
        private set
    var busy by mutableStateOf(false)
        private set

    /**
     * @param authFailed percobaan sebelumnya ditolak server -> selalu tanya lagi.
     * @param lastUsername username percobaan sebelumnya (diisikan ulang ke dialog).
     */
    fun start(
        host: Host,
        authFailed: Boolean = false,
        lastUsername: String? = null,
        onReady: (host: Host, username: String, password: String) -> Unit,
    ) {
        if (busy) return
        // Pakai data TERBARU (host bisa sudah diedit sejak sesi dibuka).
        val latest = repository.content.value.hosts.find { it.id == host.id } ?: host
        val needUsername = latest.username.isBlank()
        val needPassword = !latest.hasPassword || authFailed
        if (needUsername || needPassword) {
            request = CredentialRequest(
                host = latest,
                askUsername = needUsername || authFailed,
                askPassword = needPassword,
                // Gagal login: isi dengan username yang BARUSAN dicoba (bisa
                // hasil ketikan, host-nya sendiri tidak punya username).
                initialUsername = lastUsername?.takeIf { authFailed && it.isNotEmpty() } ?: latest.username,
                error = if (authFailed) "Username atau password SSH salah." else null,
                onReady = onReady,
            )
            return
        }
        run {
            onReady(latest, latest.username, repository.hostPassword(latest.id))
        }
    }

    fun submit(username: String, password: String, saveToServer: Boolean) {
        val req = request ?: return
        val user = username.trim()
        if (user.isEmpty()) {
            request = req.withError("Username wajib diisi.")
            return
        }
        if (req.askPassword && password.isEmpty()) {
            request = req.withError("Password wajib diisi.")
            return
        }
        request = null
        run {
            val pw = if (req.askPassword) password else repository.hostPassword(req.host.id)
            if (saveToServer) {
                // Gagal simpan TIDAK membatalkan connect — cukup diberi tahu.
                try {
                    repository.saveCredentials(req.host, user, if (req.askPassword) password else null)
                } catch (e: ApiError) {
                    snackbar.showSnackbar("Gagal menyimpan ke server: ${e.message}")
                }
            }
            req.onReady(req.host.copy(username = user), user, pw)
        }
    }

    fun dismiss() {
        request = null
    }

    private fun run(block: suspend () -> Unit) {
        busy = true
        scope.launch {
            try {
                block()
            } catch (e: ApiError) {
                snackbar.showSnackbar("Gagal mengambil password dari server: ${e.message}")
            } finally {
                busy = false
            }
        }
    }
}

/** Dialog username/password sebelum connect. */
@Composable
fun CredentialDialog(request: CredentialRequest, onSubmit: (String, String, Boolean) -> Unit, onDismiss: () -> Unit) {
    var username by rememberSaveable(request.host.id) { mutableStateOf(request.initialUsername) }
    var password by rememberSaveable(request.host.id) { mutableStateOf("") }
    var save by rememberSaveable(request.host.id) { mutableStateOf(false) }

    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(request.host.label) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text(request.host.address(), color = MaterialTheme.colorScheme.onSurfaceVariant)
                request.error?.let { Text(it, color = MaterialTheme.colorScheme.error) }
                if (request.askUsername) {
                    OutlinedTextField(
                        value = username,
                        onValueChange = { username = it },
                        label = { Text("Username") },
                        singleLine = true,
                        keyboardOptions = KeyboardOptions(autoCorrectEnabled = false),
                        modifier = Modifier.fillMaxWidth(),
                    )
                }
                if (request.askPassword) {
                    PasswordField(value = password, onValueChange = { password = it }, label = "Password")
                }
                // Seluruh baris bisa diketuk (bukan cuma kotaknya).
                Row(
                    Modifier.fillMaxWidth().toggleable(value = save, role = Role.Checkbox, onValueChange = { save = it }),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Checkbox(checked = save, onCheckedChange = null)
                    Text("Simpan ke server")
                }
            }
        },
        confirmButton = { TextButton(onClick = { onSubmit(username, password, save) }) { Text("Sambungkan") } },
        dismissButton = { TextButton(onClick = onDismiss) { Text("Batal") } },
    )
}
