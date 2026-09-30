package org.terminus.mobile.ui.account

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.launch
import org.terminus.mobile.api.Account
import org.terminus.mobile.api.ApiError
import org.terminus.mobile.api.ApiSession
import org.terminus.mobile.data.planProfileUpdate
import org.terminus.mobile.data.validatePasswordChange
import org.terminus.mobile.ui.common.ConfirmDialog
import org.terminus.mobile.ui.common.PasswordField
import org.terminus.mobile.ui.common.SubScreenTopBar

/**
 * Profil & keamanan (mobile/DESIGN.md bagian 7): ubah nama, email (wajib
 * password saat ini), ganti password (semua perangkat LAIN logout, perangkat
 * ini dapat token baru). Password saat ini salah = 403 -> pesan biasa,
 * BUKAN logout.
 */
@Composable
fun ProfileScreen(account: Account, session: ApiSession, onAccountUpdated: (Account) -> Unit, onClose: () -> Unit) {
    val scope = rememberCoroutineScope()
    BackHandler(onBack = onClose)

    // --- Profil ---
    var name by rememberSaveable { mutableStateOf(account.fullName.orEmpty()) }
    var email by rememberSaveable { mutableStateOf(account.email) }
    var emailPassword by rememberSaveable { mutableStateOf("") }
    var profileBusy by rememberSaveable { mutableStateOf(false) }
    var profileError by rememberSaveable { mutableStateOf<String?>(null) }
    var profileInfo by rememberSaveable { mutableStateOf<String?>(null) }

    // --- Password ---
    var currentPassword by rememberSaveable { mutableStateOf("") }
    var newPassword by rememberSaveable { mutableStateOf("") }
    var confirmPassword by rememberSaveable { mutableStateOf("") }
    var passwordBusy by rememberSaveable { mutableStateOf(false) }
    var passwordError by rememberSaveable { mutableStateOf<String?>(null) }
    var passwordInfo by rememberSaveable { mutableStateOf<String?>(null) }
    var confirmChange by rememberSaveable { mutableStateOf(false) }

    val emailChanged = email.trim() != account.email

    fun saveProfile() {
        profileInfo = null
        val update = planProfileUpdate(account.fullName.orEmpty(), account.email, name, email, emailPassword).getOrElse {
            profileError = it.message
            return
        }
        if (update == null) {
            profileError = null
            profileInfo = "Tidak ada yang berubah."
            return
        }
        profileBusy = true
        profileError = null
        scope.launch {
            try {
                val updated = session.updateProfile(update.fullName, update.email, update.currentPassword)
                onAccountUpdated(updated)
                name = updated.fullName.orEmpty()
                email = updated.email
                emailPassword = ""
                profileInfo = "Profil disimpan."
            } catch (e: ApiError) {
                profileError = e.message
            } finally {
                profileBusy = false
            }
        }
    }

    fun changePassword() {
        confirmChange = false
        passwordBusy = true
        passwordError = null
        scope.launch {
            try {
                session.changePassword(currentPassword, newPassword)
                currentPassword = ""
                newPassword = ""
                confirmPassword = ""
                passwordInfo = "Password diganti. Semua perangkat lain sudah logout."
            } catch (e: ApiError) {
                passwordError = e.message
            } finally {
                passwordBusy = false
            }
        }
    }

    Column(Modifier.fillMaxSize()) {
        SubScreenTopBar(title = "Profil & keamanan", onBack = onClose)
        Column(
            Modifier
                .fillMaxSize()
                .imePadding()
                .verticalScroll(rememberScrollState())
                .padding(horizontal = 16.dp, vertical = 8.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Text("Profil", style = MaterialTheme.typography.titleMedium)
            profileError?.let { Text(it, color = MaterialTheme.colorScheme.error) }
            profileInfo?.let { Text(it, color = MaterialTheme.colorScheme.primary) }
            OutlinedTextField(
                value = name,
                onValueChange = { name = it },
                label = { Text("Nama lengkap") },
                singleLine = true,
                enabled = !profileBusy,
                modifier = Modifier.fillMaxWidth(),
            )
            OutlinedTextField(
                value = email,
                onValueChange = { email = it },
                label = { Text("Email") },
                singleLine = true,
                enabled = !profileBusy,
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Email, autoCorrectEnabled = false),
                modifier = Modifier.fillMaxWidth(),
            )
            if (emailChanged) {
                PasswordField(
                    value = emailPassword,
                    onValueChange = { emailPassword = it },
                    label = "Password saat ini",
                    enabled = !profileBusy,
                    supportingText = "Wajib untuk mengganti email.",
                )
            }
            BusyButton("Simpan profil", profileBusy, ::saveProfile)

            HorizontalDivider(Modifier.padding(vertical = 16.dp))

            Text("Ganti password", style = MaterialTheme.typography.titleMedium)
            Text(
                "Semua perangkat LAIN (desktop, HP lain) akan logout. Perangkat ini tetap masuk.",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            passwordError?.let { Text(it, color = MaterialTheme.colorScheme.error) }
            passwordInfo?.let { Text(it, color = MaterialTheme.colorScheme.primary) }
            PasswordField(currentPassword, { currentPassword = it }, "Password saat ini", enabled = !passwordBusy)
            PasswordField(
                newPassword,
                { newPassword = it },
                "Password baru",
                enabled = !passwordBusy,
                supportingText = "Minimal 8 karakter.",
            )
            PasswordField(confirmPassword, { confirmPassword = it }, "Konfirmasi password baru", enabled = !passwordBusy)
            BusyButton("Ganti password", passwordBusy) {
                passwordInfo = null
                passwordError = validatePasswordChange(currentPassword, newPassword, confirmPassword)
                if (passwordError == null) confirmChange = true
            }
        }
    }

    if (confirmChange) {
        ConfirmDialog(
            title = "Ganti password?",
            text = "Semua perangkat lain yang login dengan akun ini (desktop, HP lain) akan logout dan perlu login ulang dengan password baru.",
            confirmLabel = "Ganti",
            onConfirm = ::changePassword,
            onDismiss = { confirmChange = false },
        )
    }
}

@Composable
private fun BusyButton(label: String, busy: Boolean, onClick: () -> Unit) {
    Button(onClick = onClick, enabled = !busy, modifier = Modifier.fillMaxWidth().padding(top = 4.dp)) {
        if (busy) CircularProgressIndicator(Modifier.padding(end = 8.dp).size(18.dp), strokeWidth = 2.dp)
        Text(label)
    }
}
