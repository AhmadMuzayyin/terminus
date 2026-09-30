package org.terminus.mobile.ui.login

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp
import org.terminus.mobile.auth.LoginUiState

/**
 * Login ke server Self-hosted + form "Daftar admin pertama" (mobile/DESIGN.md
 * bagian 7). URL server tetap terisi antar form & setelah logout.
 */
@Composable
fun LoginScreen(
    initialServerUrl: String,
    ui: LoginUiState,
    onLogin: (url: String, email: String, password: String) -> Unit,
    onRegister: (url: String, fullName: String, email: String, password: String, confirm: String) -> Unit,
    onSwitchForm: () -> Unit,
) {
    var registerMode by rememberSaveable { mutableStateOf(false) }
    var url by rememberSaveable { mutableStateOf(initialServerUrl) }
    var fullName by rememberSaveable { mutableStateOf("") }
    var email by rememberSaveable { mutableStateOf("") }
    var password by rememberSaveable { mutableStateOf("") }
    var confirm by rememberSaveable { mutableStateOf("") }

    val submit = {
        if (registerMode) onRegister(url, fullName, email, password, confirm) else onLogin(url, email, password)
    }

    Column(
        modifier = Modifier
            .fillMaxSize()
            .safeDrawingPadding()
            .imePadding()
            .verticalScroll(rememberScrollState())
            .padding(24.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.Center,
    ) {
        Column(Modifier.widthIn(max = 420.dp).fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(12.dp)) {
            Text(
                if (registerMode) "Daftar Admin Pertama" else "Masuk ke Server",
                style = MaterialTheme.typography.headlineSmall,
            )
            Text(
                if (registerMode) {
                    "Cuma bisa sekali, waktu server belum punya user sama sekali."
                } else {
                    "Host & kredensial disimpan di server Terminus self-hosted kamu, tidak ada salinan di HP."
                },
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )

            Field("Server URL", url, { url = it }, KeyboardType.Uri, placeholder = "https://vault.contoh.com")
            if (url.trim().startsWith("http://")) {
                Text(
                    "Koneksi http:// tidak terenkripsi — aman cuma di jaringan yang kamu percaya.",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.error,
                )
            }
            if (registerMode) Field("Nama Lengkap", fullName, { fullName = it }, KeyboardType.Text)
            Field("Email", email, { email = it }, KeyboardType.Email)
            Field(
                if (registerMode) "Password (minimal 8 karakter)" else "Password",
                password,
                { password = it },
                KeyboardType.Password,
                isPassword = true,
                last = !registerMode,
                onDone = submit,
            )
            if (registerMode) {
                Field("Konfirmasi Password", confirm, { confirm = it }, KeyboardType.Password, isPassword = true, last = true, onDone = submit)
            }

            ui.info?.let { Text(it, color = MaterialTheme.colorScheme.onSurfaceVariant, style = MaterialTheme.typography.bodySmall) }
            ui.error?.let { Text(it, color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodySmall) }

            Spacer(Modifier.height(4.dp))
            Button(onClick = submit, enabled = !ui.busy, modifier = Modifier.fillMaxWidth()) {
                if (ui.busy) {
                    CircularProgressIndicator(Modifier.size(18.dp), strokeWidth = 2.dp)
                } else {
                    Text(if (registerMode) "Daftar & Masuk" else "Masuk")
                }
            }
            TextButton(
                onClick = {
                    registerMode = !registerMode
                    onSwitchForm()
                },
                enabled = !ui.busy,
                modifier = Modifier.fillMaxWidth(),
            ) {
                Text(if (registerMode) "Sudah punya akun? Masuk" else "Server baru? Daftar admin pertama")
            }
        }
    }
}

@Composable
private fun Field(
    label: String,
    value: String,
    onChange: (String) -> Unit,
    keyboardType: KeyboardType,
    placeholder: String? = null,
    isPassword: Boolean = false,
    last: Boolean = false,
    onDone: () -> Unit = {},
) {
    OutlinedTextField(
        value = value,
        onValueChange = onChange,
        label = { Text(label) },
        placeholder = placeholder?.let { { Text(it) } },
        singleLine = true,
        visualTransformation = if (isPassword) PasswordVisualTransformation() else androidx.compose.ui.text.input.VisualTransformation.None,
        keyboardOptions = KeyboardOptions(
            keyboardType = keyboardType,
            imeAction = if (last) ImeAction.Done else ImeAction.Next,
            autoCorrectEnabled = false,
        ),
        keyboardActions = androidx.compose.foundation.text.KeyboardActions(onDone = { onDone() }),
        modifier = Modifier.fillMaxWidth(),
    )
}
