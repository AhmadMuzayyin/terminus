package org.terminus.mobile.ui.sftp

import android.content.Context
import android.net.Uri
import android.provider.DocumentsContract
import android.provider.OpenableColumns
import androidx.activity.compose.BackHandler
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
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
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.ListItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import java.io.IOException
import java.time.Instant
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import java.util.Locale
import kotlinx.coroutines.launch
import org.terminus.mobile.R
import org.terminus.mobile.api.Host
import org.terminus.mobile.data.VaultRepository
import org.terminus.mobile.data.address
import org.terminus.mobile.data.searchHosts
import org.terminus.mobile.sftp.SftpManager
import org.terminus.mobile.sftp.SftpStatus
import org.terminus.mobile.sftp.Transfer
import org.terminus.mobile.sftp.formatSize
import org.terminus.mobile.sftp.parentPath
import org.terminus.mobile.sftp.validateRemoteName
import org.terminus.mobile.ssh.RemoteEntry
import org.terminus.mobile.ssh.RemoteKind
import org.terminus.mobile.ssh.SftpError
import org.terminus.mobile.ui.common.CenterMessage
import org.terminus.mobile.ui.common.ConfirmDialog
import org.terminus.mobile.ui.common.ErrorStrip
import org.terminus.mobile.ui.common.HostKeyChangedBanner
import org.terminus.mobile.ui.common.ProgressBar
import org.terminus.mobile.ui.common.RowAction
import org.terminus.mobile.ui.common.RowActionsMenu
import org.terminus.mobile.ui.common.StatusBanner
import org.terminus.mobile.ui.common.TextInputDialog
import org.terminus.mobile.ui.common.UntrustedHostKeyDialog
import org.terminus.mobile.ui.terminal.ConnectFlow

/**
 * Tab SFTP (mobile/DESIGN.md bagian 7): pilih host -> jelajah folder;
 * unduh/unggah lewat pemilih file sistem (Storage Access Framework — tanpa
 * izin storage); buat folder, ganti nama, hapus.
 *
 * @param onPickHost pilih host di daftar (izin notifikasi + kredensial ditangani pemanggil).
 */
@Composable
fun SftpScreen(
    manager: SftpManager,
    repository: VaultRepository,
    connect: ConnectFlow,
    snackbar: SnackbarHostState,
    onPickHost: (Host) -> Unit,
) {
    val status by manager.status.collectAsStateWithLifecycle()

    when (val s = status) {
        SftpStatus.Idle -> HostPicker(repository, onPickHost)
        is SftpStatus.Connected -> Browser(manager, s.host, snackbar)
        else -> Column(Modifier.fillMaxSize()) {
            val host = manager.currentHost ?: return@Column
            val target = "${host.host}:${host.port}"
            ConnectionHeader(host.label, host.address(), onDisconnect = manager::disconnect)
            when (s) {
                is SftpStatus.Connecting -> {
                    ProgressBar()
                    StatusBanner("Menghubungkan SFTP ke ${host.address()}…", actions = listOf("Batal" to manager::disconnect))
                }
                is SftpStatus.UntrustedHostKey -> UntrustedHostKeyDialog(
                    target,
                    s.info,
                    onTrust = manager::trustHostKey,
                    onCancel = manager::disconnect,
                )
                is SftpStatus.HostKeyChanged -> HostKeyChangedBanner(
                    target,
                    s.expected,
                    s.actual,
                    closeLabel = "Tutup",
                    onForget = manager::forgetHostKey,
                    onClose = manager::disconnect,
                )
                is SftpStatus.AuthFailed -> StatusBanner(
                    "Username atau password SSH salah.",
                    error = true,
                    actions = listOf(
                        "Masukkan password" to {
                            connect.start(s.host, authFailed = true, lastUsername = s.username) { h, u, p -> manager.connect(h, u, p) }
                        },
                        "Tutup" to manager::disconnect,
                    ),
                )
                is SftpStatus.Failed -> StatusBanner(
                    s.message,
                    error = true,
                    actions = listOf(
                        "Sambung ulang" to { connect.start(s.host) { h, u, p -> manager.connect(h, u, p) } },
                        "Tutup" to manager::disconnect,
                    ),
                )
                SftpStatus.Idle, is SftpStatus.Connected -> Unit // ditangani di atas
            }
        }
    }
}

@Composable
private fun ConnectionHeader(title: String, subtitle: String, onDisconnect: () -> Unit) {
    Surface(color = MaterialTheme.colorScheme.surfaceContainer) {
        Row(
            Modifier.fillMaxWidth().padding(start = 16.dp, end = 4.dp, top = 8.dp, bottom = 8.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Column(Modifier.weight(1f)) {
                Text(title, style = MaterialTheme.typography.titleMedium, maxLines = 1, overflow = TextOverflow.Ellipsis)
                Text(
                    subtitle,
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
            IconButton(onClick = onDisconnect) { Icon(painterResource(R.drawable.ic_close), contentDescription = "Putuskan SFTP") }
        }
    }
}

/** Belum tersambung: pilih host dari vault. */
@Composable
private fun HostPicker(repository: VaultRepository, onPick: (Host) -> Unit) {
    val content by repository.content.collectAsStateWithLifecycle()
    val load by repository.load.collectAsStateWithLifecycle()
    var query by rememberSaveable { mutableStateOf("") }

    Column(Modifier.fillMaxSize()) {
        Text(
            "Pilih host untuk dijelajahi lewat SFTP",
            style = MaterialTheme.typography.titleMedium,
            modifier = Modifier.padding(start = 16.dp, end = 16.dp, top = 16.dp),
        )
        OutlinedTextField(
            value = query,
            onValueChange = { query = it },
            placeholder = { Text("Cari host") },
            leadingIcon = { Icon(painterResource(R.drawable.ic_search), contentDescription = null) },
            singleLine = true,
            modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 8.dp),
        )
        val hosts = if (query.isBlank()) {
            content.hosts.sortedWith(compareBy(String.CASE_INSENSITIVE_ORDER) { it.label })
        } else {
            searchHosts(content.groups, content.hosts, query)
        }
        when {
            !load.loaded && load.error == null -> Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                CircularProgressIndicator()
            }
            !load.loaded -> CenterMessage("Gagal memuat host", load.error)
            content.hosts.isEmpty() -> CenterMessage("Belum ada host", "Tambah host dulu di tab Hosts.")
            hosts.isEmpty() -> CenterMessage("Tidak ada host yang cocok")
            else -> LazyColumn(Modifier.fillMaxSize()) {
                items(hosts, key = { it.id }) { host ->
                    ListItem(
                        headlineContent = { Text(host.label) },
                        supportingContent = { Text(host.address(), color = MaterialTheme.colorScheme.onSurfaceVariant) },
                        leadingContent = { Icon(painterResource(R.drawable.ic_server), contentDescription = null) },
                        modifier = Modifier.tappable { onPick(host) },
                    )
                }
            }
        }
    }
}

/** Unggahan yang menunggu konfirmasi "timpa?". */
private class PendingUpload(val uri: Uri, val name: String, val size: Long?)

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun Browser(manager: SftpManager, host: Host, snackbar: SnackbarHostState) {
    val listing by manager.listing.collectAsStateWithLifecycle()
    val transfer by manager.transfer.collectAsStateWithLifecycle()
    val context = LocalContext.current
    val scope = rememberCoroutineScope()

    var mkdirOpen by rememberSaveable { mutableStateOf(false) }
    var renameTarget by remember { mutableStateOf<RemoteEntry?>(null) }
    var deleteTarget by remember { mutableStateOf<RemoteEntry?>(null) }
    var overwrite by remember { mutableStateOf<PendingUpload?>(null) }
    var dialogBusy by remember { mutableStateOf(false) }
    var dialogError by remember { mutableStateOf<String?>(null) }
    var pendingDownload by remember { mutableStateOf<RemoteEntry?>(null) }

    fun message(text: String) {
        scope.launch { snackbar.showSnackbar(text) }
    }

    // Unduh: user memilih lokasi & nama di HP ("Simpan sebagai").
    val createDocument = rememberLauncherForActivityResult(ActivityResultContracts.CreateDocument("application/octet-stream")) { uri ->
        val entry = pendingDownload
        pendingDownload = null
        if (uri == null || entry == null) return@rememberLauncherForActivityResult
        val resolver = context.contentResolver
        manager.download(
            entry,
            openOutput = { resolver.openOutputStream(uri, "wt") ?: throw IOException("Tidak bisa menulis file tujuan.") },
            onFailedCleanup = { runCatching { DocumentsContract.deleteDocument(resolver, uri) } },
        )
    }

    fun upload(pending: PendingUpload) {
        manager.upload(pending.name, pending.size) {
            context.contentResolver.openInputStream(pending.uri) ?: throw IOException("Tidak bisa membaca file.")
        }
    }

    // Unggah: user memilih file di HP.
    val openDocument = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { uri ->
        if (uri == null) return@rememberLauncherForActivityResult
        val (rawName, size) = queryNameAndSize(context, uri)
        val pending = PendingUpload(uri, validateRemoteName(rawName).getOrDefault("unggahan"), size)
        scope.launch {
            try {
                if (manager.exists(pending.name)) overwrite = pending else upload(pending)
            } catch (e: SftpError) {
                message(e.message ?: "Gagal memeriksa file tujuan.")
            }
        }
    }

    fun download(entry: RemoteEntry) {
        pendingDownload = entry
        createDocument.launch(entry.name)
    }

    fun openEntry(entry: RemoteEntry) {
        scope.launch {
            try {
                if (manager.isDirectory(entry)) manager.open(entry.path) else download(entry)
            } catch (e: SftpError) {
                message(e.message ?: "Gagal membuka.")
            }
        }
    }

    /** Dialog nama (folder baru / ganti nama): validasi lokal, lalu ke server. */
    fun submitName(raw: String, action: suspend (String) -> Unit, onDone: () -> Unit) {
        val name = validateRemoteName(raw).getOrElse {
            dialogError = it.message
            return
        }
        dialogBusy = true
        dialogError = null
        scope.launch {
            try {
                action(name)
                onDone()
            } catch (e: SftpError) {
                dialogError = e.message
            } finally {
                dialogBusy = false
            }
        }
    }

    val parent = parentPath(listing.path)
    BackHandler(enabled = parent != null) { manager.goUp() }

    Box(Modifier.fillMaxSize()) {
        Column(Modifier.fillMaxSize()) {
            ConnectionHeader(host.label, listing.path.ifEmpty { "…" }, onDisconnect = manager::disconnect)
            listing.error?.let { ErrorStrip(it, manager::refresh) }
            PullToRefreshBox(
                isRefreshing = listing.loading && listing.path.isNotEmpty(),
                onRefresh = manager::refresh,
                modifier = Modifier.weight(1f).fillMaxWidth(),
            ) {
                if (listing.path.isEmpty()) {
                    Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { CircularProgressIndicator() }
                } else {
                    LazyColumn(Modifier.fillMaxSize()) {
                        if (parent != null) {
                            item(key = "..") {
                                ListItem(
                                    headlineContent = { Text("..") },
                                    supportingContent = { Text("Folder induk", color = MaterialTheme.colorScheme.onSurfaceVariant) },
                                    leadingContent = { Icon(painterResource(R.drawable.ic_back), contentDescription = null) },
                                    modifier = Modifier.tappable { manager.goUp() },
                                )
                            }
                        }
                        if (listing.entries.isEmpty() && !listing.loading) {
                            item { CenterMessage("Folder kosong", "Ketuk + untuk mengunggah file atau membuat folder.") }
                        }
                        items(listing.entries, key = { it.path }) { entry ->
                            EntryRow(
                                entry,
                                onOpen = { openEntry(entry) },
                                actions = buildList {
                                    if (entry.kind != RemoteKind.Directory) add(RowAction("Unduh") { download(entry) })
                                    add(RowAction("Ganti nama") { dialogError = null; renameTarget = entry })
                                    add(RowAction("Hapus", destructive = true) { deleteTarget = entry })
                                },
                            )
                        }
                    }
                }
            }
            transfer?.let { TransferBar(it, onCancel = manager::cancelTransfer) }
        }

        AddMenu(
            onUpload = { openDocument.launch(arrayOf("*/*")) },
            onNewFolder = { dialogError = null; mkdirOpen = true },
            modifier = Modifier.align(Alignment.BottomEnd).padding(end = 16.dp, bottom = if (transfer != null) 88.dp else 16.dp),
        )
    }

    if (mkdirOpen) {
        TextInputDialog(
            title = "Folder baru",
            label = "Nama folder",
            initial = "",
            confirmLabel = "Buat",
            busy = dialogBusy,
            error = dialogError,
            onConfirm = { raw -> submitName(raw, { manager.mkdir(it) }) { mkdirOpen = false } },
            onDismiss = { mkdirOpen = false },
        )
    }

    renameTarget?.let { entry ->
        TextInputDialog(
            title = "Ganti nama",
            label = "Nama baru",
            initial = entry.name,
            confirmLabel = "Simpan",
            busy = dialogBusy,
            error = dialogError,
            onConfirm = { raw -> submitName(raw, { manager.rename(entry, it) }) { renameTarget = null } },
            onDismiss = { renameTarget = null },
        )
    }

    deleteTarget?.let { entry ->
        ConfirmDialog(
            title = if (entry.kind == RemoteKind.Directory) "Hapus folder?" else "Hapus file?",
            text = when (entry.kind) {
                RemoteKind.Directory -> "Folder \"${entry.name}\" BESERTA SELURUH ISINYA akan dihapus dari server. Tidak bisa dibatalkan."
                RemoteKind.Link -> "Tautan \"${entry.name}\" akan dihapus (file/folder tujuannya tidak ikut terhapus)."
                RemoteKind.File -> "\"${entry.name}\" akan dihapus dari server. Tidak bisa dibatalkan."
            },
            confirmLabel = "Hapus",
            onConfirm = {
                deleteTarget = null
                scope.launch {
                    try {
                        manager.delete(entry)
                        message("Dihapus: ${entry.name}")
                    } catch (e: SftpError) {
                        message(e.message ?: "Gagal menghapus.")
                    }
                }
            },
            onDismiss = { deleteTarget = null },
        )
    }

    overwrite?.let { pending ->
        ConfirmDialog(
            title = "Timpa file?",
            text = "\"${pending.name}\" sudah ada di folder ini. File di server akan diganti dengan file dari HP.",
            confirmLabel = "Timpa",
            onConfirm = {
                overwrite = null
                upload(pending)
            },
            onDismiss = { overwrite = null },
        )
    }
}

@Composable
private fun EntryRow(entry: RemoteEntry, onOpen: () -> Unit, actions: List<RowAction>) {
    var menu by remember { mutableStateOf(false) }
    val details = buildList {
        when (entry.kind) {
            RemoteKind.Directory -> Unit
            RemoteKind.Link -> add("tautan")
            RemoteKind.File -> add(formatSize(entry.size))
        }
        formatModified(entry.modifiedEpochSeconds)?.let(::add)
    }
    ListItem(
        headlineContent = { Text(entry.name, maxLines = 1, overflow = TextOverflow.Ellipsis) },
        supportingContent = { Text(details.joinToString(" · "), color = MaterialTheme.colorScheme.onSurfaceVariant) },
        leadingContent = {
            Icon(
                painterResource(if (entry.kind == RemoteKind.File) R.drawable.ic_file else R.drawable.ic_folder),
                contentDescription = null,
            )
        },
        trailingContent = { RowActionsMenu(menu, { menu = it }, actions) },
        modifier = Modifier.tappable(onLongClick = { menu = true }, onClick = onOpen),
    )
}

@Composable
private fun TransferBar(transfer: Transfer, onCancel: () -> Unit) {
    Surface(color = MaterialTheme.colorScheme.surfaceContainerHigh) {
        Column(Modifier.fillMaxWidth().padding(start = 16.dp, end = 8.dp, top = 8.dp)) {
            val total = transfer.total?.takeIf { it > 0 }
            if (total != null) {
                ProgressBar(progress = { (transfer.done.toFloat() / total).coerceIn(0f, 1f) })
            } else {
                ProgressBar()
            }
            Row(verticalAlignment = Alignment.CenterVertically) {
                val verb = if (transfer.upload) "Mengunggah" else "Mengunduh"
                val amount = formatSize(transfer.done) + (total?.let { " / " + formatSize(it) } ?: "")
                Text(
                    "$verb ${transfer.name} — $amount",
                    style = MaterialTheme.typography.bodySmall,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                    modifier = Modifier.weight(1f),
                )
                TextButton(onClick = onCancel) { Text("Batal") }
            }
        }
    }
}

@Composable
private fun AddMenu(onUpload: () -> Unit, onNewFolder: () -> Unit, modifier: Modifier) {
    var menu by remember { mutableStateOf(false) }
    Box(modifier) {
        FloatingActionButton(onClick = { menu = true }) { Icon(painterResource(R.drawable.ic_add), contentDescription = "Tambah") }
        DropdownMenu(expanded = menu, onDismissRequest = { menu = false }) {
            DropdownMenuItem(text = { Text("Unggah file") }, onClick = { menu = false; onUpload() })
            DropdownMenuItem(text = { Text("Folder baru") }, onClick = { menu = false; onNewFolder() })
        }
    }
}

@OptIn(ExperimentalFoundationApi::class)
private fun Modifier.tappable(onLongClick: (() -> Unit)? = null, onClick: () -> Unit): Modifier =
    combinedClickable(onClick = onClick, onLongClick = onLongClick)

private val modifiedFormat = DateTimeFormatter.ofPattern("d MMM yyyy HH:mm", Locale.forLanguageTag("id"))

private fun formatModified(epochSeconds: Long): String? =
    if (epochSeconds <= 0) null else Instant.ofEpochSecond(epochSeconds).atZone(ZoneId.systemDefault()).format(modifiedFormat)

/** Nama & ukuran file pilihan user (dari penyedia dokumen). */
private fun queryNameAndSize(context: Context, uri: Uri): Pair<String, Long?> {
    context.contentResolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME, OpenableColumns.SIZE), null, null, null)?.use { cursor ->
        if (cursor.moveToFirst()) {
            val name = cursor.getString(0) ?: "unggahan"
            val size = if (cursor.isNull(1)) null else cursor.getLong(1)
            return name to size
        }
    }
    return (uri.lastPathSegment ?: "unggahan") to null
}
