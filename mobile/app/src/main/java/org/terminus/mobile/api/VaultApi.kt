package org.terminus.mobile.api

import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json

/**
 * Endpoint isi vault (grup, host, identity) — mobile/DESIGN.md bagian 5.
 * Semua lewat [ApiSession.send], jadi refresh-on-401 berlaku otomatis.
 * Server-authoritative: id dibuat server, password host dikirim SESUDAH
 * host-nya ada (`PUT /hosts/:id/secret`).
 */
class VaultApi(private val client: HttpClient, private val session: ApiSession) {
    private val json = client.json

    // `HttpClient.defaultJson` pakai `explicitNulls = false` (null dibuang).
    // Untuk host, `groupId: null` justru bermakna ("keluarkan dari grup"),
    // jadi encoder-nya harus mempertahankan null.
    private val keepNulls = Json(json) { explicitNulls = true }

    // --- Grup ---

    suspend fun listGroups(): List<HostGroup> = client.decode(session.send("GET", session.vaultUrl("/groups")))

    suspend fun createGroup(name: String): HostGroup =
        client.decode(session.send("POST", session.vaultUrl("/groups"), json.encodeToString(GroupFields(name))))

    suspend fun renameGroup(id: String, name: String): HostGroup =
        client.decode(session.send("PUT", session.vaultUrl("/groups/$id"), json.encodeToString(GroupFields(name))))

    /** Ikut menghapus SEMUA host di dalam grup (+ password-nya) — UI wajib konfirmasi. */
    suspend fun deleteGroup(id: String) = client.expectSuccess(session.send("DELETE", session.vaultUrl("/groups/$id")))

    // --- Host ---

    suspend fun listHosts(): List<Host> = client.decode(session.send("GET", session.vaultUrl("/hosts")))

    suspend fun createHost(fields: HostFields): Host =
        client.decode(session.send("POST", session.vaultUrl("/hosts"), keepNulls.encodeToString(fields)))

    suspend fun updateHost(id: String, fields: HostFields): Host =
        client.decode(session.send("PUT", session.vaultUrl("/hosts/$id"), keepNulls.encodeToString(fields)))

    suspend fun deleteHost(id: String) = client.expectSuccess(session.send("DELETE", session.vaultUrl("/hosts/$id")))

    suspend fun setHostPassword(id: String, password: String) = client.expectSuccess(
        session.send("PUT", session.vaultUrl("/hosts/$id/secret"), json.encodeToString(SecretBody(password))),
    )

    /** Dipanggil CUMA saat benar-benar butuh (connect, duplikat) — bukan buat render daftar. */
    suspend fun hostPassword(id: String): String =
        client.decode<SecretBody>(session.send("GET", session.vaultUrl("/hosts/$id/secret"))).password

    // --- Identity ---

    suspend fun listIdentities(): List<Identity> = client.decode(session.send("GET", session.vaultUrl("/identities")))

    suspend fun createIdentity(label: String, username: String, password: String): Identity = client.decode(
        session.send("POST", session.vaultUrl("/identities"), json.encodeToString(IdentityCreate(label, username, password))),
    )

    /** `password` null = password lama dipertahankan. */
    suspend fun updateIdentity(id: String, label: String, username: String, password: String?): Identity = client.decode(
        session.send(
            "PUT",
            session.vaultUrl("/identities/$id"),
            json.encodeToString(IdentityUpdate(label, username, password)),
        ),
    )

    suspend fun deleteIdentity(id: String) =
        client.expectSuccess(session.send("DELETE", session.vaultUrl("/identities/$id")))

    suspend fun identityPassword(id: String): String =
        client.decode<SecretBody>(session.send("GET", session.vaultUrl("/identities/$id/secret"))).password
}
