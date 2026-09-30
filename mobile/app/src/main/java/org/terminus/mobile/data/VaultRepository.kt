package org.terminus.mobile.data

import kotlinx.coroutines.NonCancellable
import kotlinx.coroutines.async
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.withContext
import org.terminus.mobile.api.ApiError
import org.terminus.mobile.api.Host
import org.terminus.mobile.api.HostFields
import org.terminus.mobile.api.HostGroup
import org.terminus.mobile.api.Identity
import org.terminus.mobile.api.VaultApi

/** Salinan isi vault yang sedang tampil (BUKAN cache offline — selalu dari server). */
data class VaultContent(
    val groups: List<HostGroup> = emptyList(),
    val hosts: List<Host> = emptyList(),
    val identities: List<Identity> = emptyList(),
)

/** Status muat daftar: [loaded] false = belum pernah berhasil sekali pun. */
data class LoadState(val loading: Boolean = false, val loaded: Boolean = false, val error: String? = null)

/**
 * Host sudah TERSIMPAN di server, tapi password-nya gagal dikirim. Form
 * harus pindah ke mode edit host ini — kalau user menekan Simpan lagi
 * dalam mode "host baru", host-nya jadi dobel.
 */
class HostPasswordNotSaved(val host: Host, cause: ApiError) :
    Exception("Host tersimpan, tapi password gagal disimpan: ${cause.message}", cause)

/**
 * Satu-satunya sumber isi vault untuk layar Hosts & Identities. Server
 * tetap sumber kebenaran: tiap perubahan dikirim dulu, lalu daftar dimuat
 * ulang dari server (mobile/DESIGN.md bagian 5 — server-authoritative).
 *
 * Mutasi dijalankan [NonCancellable]: pindah tab/tutup form di tengah
 * simpan tidak boleh memotong langkah "buat host -> kirim password".
 */
class VaultRepository(private val api: VaultApi) {
    private val _content = MutableStateFlow(VaultContent())
    val content: StateFlow<VaultContent> = _content.asStateFlow()

    private val _load = MutableStateFlow(LoadState())
    val load: StateFlow<LoadState> = _load.asStateFlow()

    /** Muat ulang grup, host, identity sekaligus. Gagal -> isi lama TETAP tampil + pesan error. */
    suspend fun refresh() {
        _load.update { it.copy(loading = true) }
        try {
            val fresh = coroutineScope {
                val groups = async { api.listGroups() }
                val hosts = async { api.listHosts() }
                val identities = async { api.listIdentities() }
                VaultContent(groups.await(), hosts.await(), identities.await())
            }
            _content.value = fresh
            _load.value = LoadState(loaded = true)
        } catch (e: ApiError) {
            _load.update { it.copy(loading = false, error = e.message) }
        }
    }

    // --- Host ---

    /**
     * Buat host baru ([existing] null) atau ubah host. [password] null =
     * tidak diubah. Host baru dibuat DULU, baru password-nya dikirim.
     */
    suspend fun saveHost(existing: Host?, fields: HostFields, password: String?): Host = mutate {
        val saved = if (existing == null) api.createHost(fields) else api.updateHost(existing.id, fields)
        if (password != null) {
            try {
                api.setHostPassword(saved.id, password)
            } catch (e: ApiError) {
                throw HostPasswordNotSaved(saved, e)
            }
        }
        saved
    }

    /** Salinan host + password-nya (kalau ada), label diberi akhiran "(salinan)". */
    suspend fun duplicateHost(host: Host): Host = mutate {
        val copy = api.createHost(
            HostFields("${host.label} (salinan)", host.host, host.port, host.username, host.groupId),
        )
        if (host.hasPassword) api.setHostPassword(copy.id, api.hostPassword(host.id))
        copy
    }

    suspend fun deleteHost(id: String) = mutate { api.deleteHost(id) }

    /** Password host dari server — dipanggil CUMA waktu connect, dibuang setelah autentikasi. */
    suspend fun hostPassword(id: String): String = api.hostPassword(id)

    /**
     * Opsi "Simpan ke server" di dialog connect: username (kalau tadinya
     * kosong) &/atau password yang baru diketik. Field lain tetap.
     */
    suspend fun saveCredentials(host: Host, username: String, password: String?): Host =
        saveHost(host, HostFields(host.label, host.host, host.port, username, host.groupId), password)

    // --- Grup ---

    suspend fun createGroup(name: String): HostGroup = mutate { api.createGroup(name) }

    suspend fun renameGroup(id: String, name: String): HostGroup = mutate { api.renameGroup(id, name) }

    /** Ikut menghapus host di dalamnya (perilaku backend = desktop). */
    suspend fun deleteGroup(id: String) = mutate { api.deleteGroup(id) }

    // --- Identity ---

    /** [password] null waktu edit = tidak diubah. */
    suspend fun saveIdentity(existing: Identity?, label: String, username: String, password: String?): Identity =
        mutate {
            if (existing == null) {
                api.createIdentity(label, username, requireNotNull(password) { "Identity baru wajib punya password" })
            } else {
                api.updateIdentity(existing.id, label, username, password)
            }
        }

    suspend fun deleteIdentity(id: String) = mutate { api.deleteIdentity(id) }

    /** Buat mengisi form host dari identity ("Pakai identity"). */
    suspend fun identityPassword(id: String): String = api.identityPassword(id)

    /**
     * Jalankan perubahan lalu muat ulang daftar. Daftar tetap dimuat ulang
     * walau perubahannya gagal di tengah (mis. host tersimpan tapi password
     * gagal) supaya yang tampil = isi server sebenarnya.
     */
    private suspend fun <T> mutate(block: suspend () -> T): T = withContext(NonCancellable) {
        try {
            block()
        } finally {
            refresh()
        }
    }
}
