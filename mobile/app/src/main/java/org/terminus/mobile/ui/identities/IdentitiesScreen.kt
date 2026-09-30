package org.terminus.mobile.ui.identities

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FloatingActionButton
import androidx.compose.material3.Icon
import androidx.compose.material3.ListItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Text
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import kotlinx.coroutines.launch
import org.terminus.mobile.R
import org.terminus.mobile.api.ApiError
import org.terminus.mobile.api.Identity
import org.terminus.mobile.data.IdentityForm
import org.terminus.mobile.data.VaultRepository
import org.terminus.mobile.ui.common.CenterMessage
import org.terminus.mobile.ui.common.ConfirmDialog
import org.terminus.mobile.ui.common.ErrorStrip
import org.terminus.mobile.ui.common.PasswordField
import org.terminus.mobile.ui.common.RowAction
import org.terminus.mobile.ui.common.RowActionsMenu
import org.terminus.mobile.ui.common.SaveAction
import org.terminus.mobile.ui.common.SubScreenTopBar

/** Tab Identities (mobile/DESIGN.md bagian 7): daftar, tambah, edit, hapus. */
@OptIn(ExperimentalMaterial3Api::class, ExperimentalFoundationApi::class)
@Composable
fun IdentitiesScreen(repository: VaultRepository, snackbar: SnackbarHostState) {
    val content by repository.content.collectAsStateWithLifecycle()
    val load by repository.load.collectAsStateWithLifecycle()
    val scope = rememberCoroutineScope()

    var editorOpen by rememberSaveable { mutableStateOf(false) }
    var editorId by rememberSaveable { mutableStateOf<String?>(null) }
    var deleteId by rememberSaveable { mutableStateOf<String?>(null) }
    var pulling by remember { mutableStateOf(false) }

    if (editorOpen) {
        val existing = editorId?.let { id -> content.identities.find { it.id == id } }
        if (editorId != null && existing == null) {
            LaunchedEffect(Unit) { editorOpen = false }
            return
        }
        IdentityEditor(
            existing = existing,
            repository = repository,
            onClose = { editorOpen = false },
            onSaved = { message ->
                editorOpen = false
                scope.launch { snackbar.showSnackbar(message) }
            },
        )
        return
    }

    Box(Modifier.fillMaxSize()) {
        Column(Modifier.fillMaxSize()) {
            load.error?.takeIf { load.loaded }?.let { ErrorStrip(it) { scope.launch { repository.refresh() } } }
            PullToRefreshBox(
                isRefreshing = pulling,
                onRefresh = {
                    pulling = true
                    scope.launch {
                        repository.refresh()
                        pulling = false
                    }
                },
                modifier = Modifier.fillMaxSize(),
            ) {
                when {
                    !load.loaded && load.error != null -> CenterMessage(
                        "Gagal memuat identity",
                        load.error,
                        "Coba lagi" to { scope.launch { repository.refresh() } },
                    )
                    !load.loaded -> Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                        CircularProgressIndicator()
                    }
                    content.identities.isEmpty() -> CenterMessage(
                        "Belum ada identity",
                        "Identity = username + password yang bisa dipakai mengisi form host. Ketuk + untuk menambah.",
                    )
                    else -> LazyColumn(Modifier.fillMaxSize()) {
                        items(content.identities.sortedBy { it.label.lowercase() }, key = { it.id }) { identity ->
                            var menu by remember { mutableStateOf(false) }
                            val edit = { editorId = identity.id; editorOpen = true }
                            ListItem(
                                headlineContent = { Text(identity.label) },
                                supportingContent = {
                                    Text(identity.username, color = MaterialTheme.colorScheme.onSurfaceVariant)
                                },
                                leadingContent = { Icon(painterResource(R.drawable.ic_key), contentDescription = null) },
                                trailingContent = {
                                    RowActionsMenu(
                                        menu,
                                        { menu = it },
                                        listOf(
                                            RowAction("Edit", onClick = edit),
                                            RowAction("Hapus", destructive = true) { deleteId = identity.id },
                                        ),
                                    )
                                },
                                modifier = Modifier.combinedClickable(onClick = edit, onLongClick = { menu = true }),
                            )
                        }
                    }
                }
            }
        }

        FloatingActionButton(
            onClick = { editorId = null; editorOpen = true },
            modifier = Modifier.align(Alignment.BottomEnd).padding(16.dp),
        ) { Icon(painterResource(R.drawable.ic_add), contentDescription = "Tambah identity") }
    }

    deleteId?.let { id ->
        content.identities.find { it.id == id }?.let { identity ->
            ConfirmDialog(
                title = "Hapus identity?",
                // Identity cuma dipakai MENGISI form host (disalin), jadi host yang
                // pernah memakainya tidak ikut berubah.
                text = "\"${identity.label}\" beserta password-nya akan dihapus dari server. " +
                    "Host yang pernah diisi dari identity ini tidak berubah.",
                confirmLabel = "Hapus",
                onConfirm = {
                    deleteId = null
                    scope.launch {
                        val message = try {
                            repository.deleteIdentity(id)
                            "Identity dihapus."
                        } catch (e: ApiError) {
                            e.message ?: "Gagal"
                        }
                        snackbar.showSnackbar(message)
                    }
                },
                onDismiss = { deleteId = null },
            )
        }
    }
}

/** Form identity. Edit: password kosong = tidak diubah (sama dengan backend). */
@Composable
private fun IdentityEditor(
    existing: Identity?,
    repository: VaultRepository,
    onClose: () -> Unit,
    onSaved: (message: String) -> Unit,
) {
    val scope = rememberCoroutineScope()
    var label by rememberSaveable { mutableStateOf(existing?.label.orEmpty()) }
    var username by rememberSaveable { mutableStateOf(existing?.username.orEmpty()) }
    var password by rememberSaveable { mutableStateOf("") }
    var busy by rememberSaveable { mutableStateOf(false) }
    var error by rememberSaveable { mutableStateOf<String?>(null) }

    BackHandler(onBack = onClose)

    fun save() {
        IdentityForm(label, username, password).validate(isNew = existing == null)?.let {
            error = it
            return
        }
        busy = true
        error = null
        scope.launch {
            try {
                repository.saveIdentity(existing, label.trim(), username.trim(), password.takeIf { it.isNotEmpty() })
                onSaved(if (existing == null) "Identity ditambahkan." else "Identity disimpan.")
            } catch (e: ApiError) {
                error = e.message
            } finally {
                busy = false
            }
        }
    }

    Column(Modifier.fillMaxSize()) {
        SubScreenTopBar(
            title = if (existing == null) "Identity baru" else "Edit identity",
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
            OutlinedTextField(
                value = label,
                onValueChange = { label = it },
                label = { Text("Label") },
                singleLine = true,
                enabled = !busy,
                modifier = Modifier.fillMaxWidth(),
            )
            OutlinedTextField(
                value = username,
                onValueChange = { username = it },
                label = { Text("Username") },
                singleLine = true,
                enabled = !busy,
                keyboardOptions = KeyboardOptions(autoCorrectEnabled = false),
                modifier = Modifier.fillMaxWidth(),
            )
            PasswordField(
                value = password,
                onValueChange = { password = it },
                label = "Password",
                enabled = !busy,
                supportingText = if (existing == null) null else "Kosongkan kalau tidak diubah.",
            )
        }
    }
}
