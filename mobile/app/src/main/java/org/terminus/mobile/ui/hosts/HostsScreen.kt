package org.terminus.mobile.ui.hosts

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FloatingActionButton
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
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
import org.terminus.mobile.api.Host
import org.terminus.mobile.api.HostGroup
import org.terminus.mobile.data.VaultRepository
import org.terminus.mobile.data.address
import org.terminus.mobile.data.hostListing
import org.terminus.mobile.data.searchHosts
import org.terminus.mobile.data.validateGroupName
import org.terminus.mobile.ui.common.CenterMessage
import org.terminus.mobile.ui.common.ConfirmDialog
import org.terminus.mobile.ui.common.ErrorStrip
import org.terminus.mobile.ui.common.RowAction
import org.terminus.mobile.ui.common.RowActionsMenu
import org.terminus.mobile.ui.common.SubScreenTopBar
import org.terminus.mobile.ui.common.TextInputDialog

/** Target dialog nama grup "grup baru" (selain itu = id grup yang diganti namanya). */
private const val NEW_GROUP = "__new__"

/**
 * Tab Hosts (mobile/DESIGN.md bagian 7): grup + host tanpa grup di akar,
 * masuk grup (drill-down satu tingkat), pencarian lintas grup, FAB tambah,
 * menu baris (⋮ atau tahan lama) edit/duplikat/hapus.
 *
 * @param onConnect ketuk host — terminal SSH (Milestone 4).
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun HostsScreen(repository: VaultRepository, snackbar: SnackbarHostState, onConnect: (Host) -> Unit) {
    val content by repository.content.collectAsStateWithLifecycle()
    val load by repository.load.collectAsStateWithLifecycle()
    val scope = rememberCoroutineScope()

    var openGroupId by rememberSaveable { mutableStateOf<String?>(null) }
    var query by rememberSaveable { mutableStateOf("") }
    var editorOpen by rememberSaveable { mutableStateOf(false) }
    var editorHostId by rememberSaveable { mutableStateOf<String?>(null) }
    var groupDialog by rememberSaveable { mutableStateOf<String?>(null) }
    var groupDialogBusy by rememberSaveable { mutableStateOf(false) }
    var groupDialogError by rememberSaveable { mutableStateOf<String?>(null) }
    var deleteHostId by rememberSaveable { mutableStateOf<String?>(null) }
    var deleteGroupId by rememberSaveable { mutableStateOf<String?>(null) }
    var pulling by remember { mutableStateOf(false) }

    val openGroup = content.groups.find { it.id == openGroupId }
    // Grup yang sedang dibuka dihapus (di sini atau perangkat lain) -> kembali ke akar.
    LaunchedEffect(openGroupId, openGroup, load.loaded) {
        if (openGroupId != null && openGroup == null && load.loaded) openGroupId = null
    }

    /** Jalankan perubahan; sukses/gagal diumumkan lewat snackbar. */
    fun perform(success: String, block: suspend () -> Unit) {
        scope.launch {
            val message = try {
                block()
                success
            } catch (e: ApiError) {
                e.message ?: "Gagal"
            }
            snackbar.showSnackbar(message)
        }
    }

    if (editorOpen) {
        val existing = editorHostId?.let { id -> content.hosts.find { it.id == id } }
        if (editorHostId != null && existing == null) {
            // Host yang sedang diedit sudah hilang dari server.
            LaunchedEffect(Unit) { editorOpen = false }
            return
        }
        HostEditor(
            existing = existing,
            presetGroupId = openGroupId,
            groups = content.groups,
            identities = content.identities,
            repository = repository,
            onClose = { editorOpen = false },
            onSaved = { message ->
                editorOpen = false
                scope.launch { snackbar.showSnackbar(message) }
            },
            onSavedAsExisting = { editorHostId = it.id },
        )
        return
    }

    BackHandler(enabled = query.isNotEmpty() || openGroupId != null) {
        if (query.isNotEmpty()) query = "" else openGroupId = null
    }

    fun hostActions(host: Host) = listOf(
        RowAction("Edit") { editorHostId = host.id; editorOpen = true },
        RowAction("Duplikat") { perform("Host diduplikat.") { repository.duplicateHost(host) } },
        RowAction("Hapus", destructive = true) { deleteHostId = host.id },
    )

    fun groupActions(group: HostGroup) = listOf(
        RowAction("Ganti nama") { groupDialogError = null; groupDialog = group.id },
        RowAction("Hapus", destructive = true) { deleteGroupId = group.id },
    )

    Box(Modifier.fillMaxSize()) {
        Column(Modifier.fillMaxSize()) {
            if (openGroup != null) {
                var menu by remember { mutableStateOf(false) }
                SubScreenTopBar(title = openGroup.name, onBack = { query = ""; openGroupId = null }) {
                    RowActionsMenu(menu, { menu = it }, groupActions(openGroup))
                }
            }
            SearchField(query, onQueryChange = { query = it })
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
                        "Gagal memuat host",
                        load.error,
                        "Coba lagi" to { scope.launch { repository.refresh() } },
                    )
                    !load.loaded -> Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                        CircularProgressIndicator()
                    }
                    query.isNotBlank() -> {
                        val results = searchHosts(content.groups, content.hosts, query)
                        if (results.isEmpty()) {
                            CenterMessage("Tidak ada host yang cocok")
                        } else {
                            val groupNames = content.groups.associate { it.id to it.name }
                            LazyColumn(Modifier.fillMaxSize()) {
                                items(results, key = { it.id }) { host ->
                                    HostRow(host, groupNames[host.groupId], { onConnect(host) }, hostActions(host))
                                }
                            }
                        }
                    }
                    else -> {
                        val listing = hostListing(content.groups, content.hosts, openGroupId)
                        if (listing.groups.isEmpty() && listing.hosts.isEmpty()) {
                            CenterMessage(
                                if (openGroupId == null) "Belum ada host" else "Grup ini kosong",
                                if (openGroupId == null) "Ketuk + untuk menambah host atau grup." else "Ketuk + untuk menambah host.",
                            )
                        } else {
                            LazyColumn(Modifier.fillMaxSize()) {
                                items(listing.groups, key = { "g:" + it.group.id }) { entry ->
                                    GroupRow(
                                        entry.group,
                                        entry.hostCount,
                                        onOpen = { query = ""; openGroupId = entry.group.id },
                                        actions = groupActions(entry.group),
                                    )
                                }
                                if (listing.groups.isNotEmpty() && listing.hosts.isNotEmpty()) {
                                    item { HorizontalDivider(Modifier.padding(vertical = 4.dp)) }
                                }
                                items(listing.hosts, key = { "h:" + it.id }) { host ->
                                    HostRow(host, null, { onConnect(host) }, hostActions(host))
                                }
                            }
                        }
                    }
                }
            }
        }

        AddButton(
            insideGroup = openGroupId != null,
            onNewHost = { editorHostId = null; editorOpen = true },
            onNewGroup = { groupDialogError = null; groupDialog = NEW_GROUP },
            modifier = Modifier.align(Alignment.BottomEnd).padding(16.dp),
        )
    }

    groupDialog?.let { target ->
        val renaming = content.groups.find { it.id == target }
        TextInputDialog(
            title = if (renaming == null) "Grup baru" else "Ganti nama grup",
            label = "Nama grup",
            initial = renaming?.name.orEmpty(),
            confirmLabel = "Simpan",
            busy = groupDialogBusy,
            error = groupDialogError,
            onConfirm = { raw ->
                validateGroupName(raw).fold(
                    onSuccess = { name ->
                        groupDialogBusy = true
                        groupDialogError = null
                        scope.launch {
                            try {
                                if (renaming == null) repository.createGroup(name) else repository.renameGroup(renaming.id, name)
                                groupDialog = null
                                snackbar.showSnackbar(if (renaming == null) "Grup dibuat." else "Nama grup diubah.")
                            } catch (e: ApiError) {
                                groupDialogError = e.message
                            } finally {
                                groupDialogBusy = false
                            }
                        }
                    },
                    onFailure = { groupDialogError = it.message },
                )
            },
            onDismiss = { groupDialog = null },
        )
    }

    deleteHostId?.let { id ->
        content.hosts.find { it.id == id }?.let { host ->
            ConfirmDialog(
                title = "Hapus host?",
                text = "\"${host.label}\" beserta password tersimpannya akan dihapus dari server. Tidak bisa dibatalkan.",
                confirmLabel = "Hapus",
                onConfirm = {
                    deleteHostId = null
                    perform("Host dihapus.") { repository.deleteHost(id) }
                },
                onDismiss = { deleteHostId = null },
            )
        }
    }

    deleteGroupId?.let { id ->
        content.groups.find { it.id == id }?.let { group ->
            val count = content.hosts.count { it.groupId == id }
            ConfirmDialog(
                title = "Hapus grup?",
                // Backend (= desktop) ikut menghapus host di dalam grup — wajib jelas di sini.
                text = if (count == 0) {
                    "Grup \"${group.name}\" akan dihapus."
                } else {
                    "Grup \"${group.name}\" BESERTA $count host di dalamnya (termasuk password tersimpan) " +
                        "akan dihapus dari server. Tidak bisa dibatalkan."
                },
                confirmLabel = "Hapus",
                onConfirm = {
                    deleteGroupId = null
                    perform("Grup dihapus.") { repository.deleteGroup(id) }
                },
                onDismiss = { deleteGroupId = null },
            )
        }
    }
}

@Composable
private fun SearchField(query: String, onQueryChange: (String) -> Unit) {
    OutlinedTextField(
        value = query,
        onValueChange = onQueryChange,
        placeholder = { Text("Cari host, IP, username, grup") },
        leadingIcon = { Icon(painterResource(R.drawable.ic_search), contentDescription = null) },
        trailingIcon = {
            if (query.isNotEmpty()) {
                IconButton(onClick = { onQueryChange("") }) {
                    Icon(painterResource(R.drawable.ic_close), contentDescription = "Kosongkan pencarian")
                }
            }
        },
        singleLine = true,
        modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 8.dp),
    )
}

@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun GroupRow(group: HostGroup, hostCount: Int, onOpen: () -> Unit, actions: List<RowAction>) {
    var menu by remember { mutableStateOf(false) }
    ListItem(
        headlineContent = { Text(group.name) },
        supportingContent = { Text("$hostCount host") },
        leadingContent = { Icon(painterResource(R.drawable.ic_folder), contentDescription = null) },
        trailingContent = { RowActionsMenu(menu, { menu = it }, actions) },
        modifier = Modifier.combinedClickable(onClick = onOpen, onLongClick = { menu = true }),
    )
}

@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun HostRow(host: Host, groupName: String?, onClick: () -> Unit, actions: List<RowAction>) {
    var menu by remember { mutableStateOf(false) }
    val details = listOfNotNull(host.address(), groupName, "tanpa password".takeUnless { host.hasPassword })
    ListItem(
        headlineContent = { Text(host.label) },
        supportingContent = { Text(details.joinToString(" · "), color = MaterialTheme.colorScheme.onSurfaceVariant) },
        leadingContent = { Icon(painterResource(R.drawable.ic_server), contentDescription = null) },
        trailingContent = { RowActionsMenu(menu, { menu = it }, actions) },
        modifier = Modifier.combinedClickable(onClick = onClick, onLongClick = { menu = true }),
    )
}

/** FAB: di akar pilih "Host baru"/"Grup baru"; di dalam grup langsung host baru di grup itu. */
@Composable
private fun AddButton(insideGroup: Boolean, onNewHost: () -> Unit, onNewGroup: () -> Unit, modifier: Modifier) {
    var menu by remember { mutableStateOf(false) }
    Box(modifier) {
        FloatingActionButton(onClick = { if (insideGroup) onNewHost() else menu = true }) {
            Icon(painterResource(R.drawable.ic_add), contentDescription = "Tambah")
        }
        DropdownMenu(expanded = menu, onDismissRequest = { menu = false }) {
            DropdownMenuItem(text = { Text("Host baru") }, onClick = { menu = false; onNewHost() })
            DropdownMenuItem(text = { Text("Grup baru") }, onClick = { menu = false; onNewGroup() })
        }
    }
}
