package org.terminus.mobile.ui.hosts

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
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
import org.terminus.mobile.api.ApiError
import org.terminus.mobile.api.Host
import org.terminus.mobile.api.HostGroup
import org.terminus.mobile.api.Identity
import org.terminus.mobile.data.HostForm
import org.terminus.mobile.data.HostPasswordNotSaved
import org.terminus.mobile.data.VaultRepository
import org.terminus.mobile.ui.common.PasswordField
import org.terminus.mobile.ui.common.SaveAction
import org.terminus.mobile.ui.common.SelectField
import org.terminus.mobile.ui.common.SubScreenTopBar

/**
 * Form tambah/edit host (mobile/DESIGN.md bagian 7): label, host, port,
 * username, grup, password — atau isi username+password dari Identity.
 *
 * Password lama TIDAK PERNAH diambil dari server buat form ini; kosong =
 * tidak diubah.
 *
 * @param existing null = host baru.
 * @param onSavedAsExisting host baru SUDAH tersimpan tapi password-nya
 *   gagal — pemanggil harus mengganti target form ke host itu (isian form
 *   tetap), supaya Simpan berikutnya tidak membuat host dobel.
 */
@Composable
fun HostEditor(
    existing: Host?,
    presetGroupId: String?,
    groups: List<HostGroup>,
    identities: List<Identity>,
    repository: VaultRepository,
    onClose: () -> Unit,
    onSaved: (message: String) -> Unit,
    onSavedAsExisting: (Host) -> Unit,
) {
    val scope = rememberCoroutineScope()
    var label by rememberSaveable { mutableStateOf(existing?.label.orEmpty()) }
    var host by rememberSaveable { mutableStateOf(existing?.host.orEmpty()) }
    var port by rememberSaveable { mutableStateOf((existing?.port ?: 22).toString()) }
    var username by rememberSaveable { mutableStateOf(existing?.username.orEmpty()) }
    var groupId by rememberSaveable { mutableStateOf(existing?.groupId ?: presetGroupId) }
    var password by rememberSaveable { mutableStateOf("") }
    var busy by rememberSaveable { mutableStateOf(false) }
    var error by rememberSaveable { mutableStateOf<String?>(null) }

    BackHandler(onBack = onClose)

    fun save() {
        val form = HostForm(label, host, port, username, groupId, password)
        val fields = form.toFields().getOrElse {
            error = it.message
            return
        }
        busy = true
        error = null
        scope.launch {
            try {
                repository.saveHost(existing, fields, form.newPassword)
                onSaved(if (existing == null) "Host ditambahkan." else "Host disimpan.")
            } catch (e: HostPasswordNotSaved) {
                onSavedAsExisting(e.host)
                error = e.message
            } catch (e: ApiError) {
                error = e.message
            } finally {
                busy = false
            }
        }
    }

    fun fillFromIdentity(identity: Identity) {
        busy = true
        error = null
        scope.launch {
            try {
                password = repository.identityPassword(identity.id)
                username = identity.username
            } catch (e: ApiError) {
                error = "Gagal mengambil password identity: ${e.message}"
            } finally {
                busy = false
            }
        }
    }

    Column(Modifier.fillMaxSize()) {
        SubScreenTopBar(
            title = if (existing == null) "Host baru" else "Edit host",
            onBack = onClose,
            actions = { SaveAction(busy, ::save) },
        )
        Column(
            Modifier
                .fillMaxSize()
                .imePadding()
                .verticalScroll(rememberScrollState())
                .padding(horizontal = 16.dp, vertical = 8.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            error?.let { Text(it, color = MaterialTheme.colorScheme.error) }

            if (identities.isNotEmpty()) {
                SelectField(
                    label = "Isi dari identity",
                    options = identities.sortedBy { it.label.lowercase() }.map { it to "${it.label} (${it.username})" },
                    selected = null,
                    onSelect = { it?.let(::fillFromIdentity) },
                    enabled = !busy,
                    placeholder = "Pilih identity…",
                )
            }
            OutlinedTextField(
                value = label,
                onValueChange = { label = it },
                label = { Text("Label") },
                supportingText = { Text("Kosong = pakai host/IP") },
                singleLine = true,
                enabled = !busy,
                modifier = Modifier.fillMaxWidth(),
            )
            OutlinedTextField(
                value = host,
                onValueChange = { host = it },
                label = { Text("Host / IP") },
                singleLine = true,
                enabled = !busy,
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Uri, autoCorrectEnabled = false),
                modifier = Modifier.fillMaxWidth(),
            )
            OutlinedTextField(
                value = port,
                onValueChange = { value -> port = value.filter(Char::isDigit).take(5) },
                label = { Text("Port") },
                singleLine = true,
                enabled = !busy,
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
                modifier = Modifier.fillMaxWidth(),
            )
            OutlinedTextField(
                value = username,
                onValueChange = { username = it },
                label = { Text("Username") },
                supportingText = { Text("Boleh kosong — ditanyakan waktu connect") },
                singleLine = true,
                enabled = !busy,
                keyboardOptions = KeyboardOptions(autoCorrectEnabled = false),
                modifier = Modifier.fillMaxWidth(),
            )
            SelectField(
                label = "Grup",
                options = listOf<Pair<String?, String>>(null to "Tanpa grup") +
                    groups.sortedBy { it.name.lowercase() }.map { it.id to it.name },
                selected = groupId,
                onSelect = { groupId = it },
                enabled = !busy,
                // Grup terpilih sudah dihapus di tempat lain -> tampilkan jujur.
                placeholder = "Grup tidak ditemukan",
            )
            PasswordField(
                value = password,
                onValueChange = { password = it },
                label = "Password",
                enabled = !busy,
                supportingText = when {
                    existing?.hasPassword == true -> "Tersimpan di server. Kosongkan kalau tidak diubah."
                    existing != null -> "Belum ada password. Boleh diisi nanti."
                    else -> "Opsional — bisa diisi nanti."
                },
            )
        }
    }
}
