package org.terminus.mobile.api

import kotlinx.serialization.Serializable

// Bentuk JSON persis seperti backend/src/modules/{groups,hosts,identities}.
// Field yang tidak dipakai mobile (tags, terminalTheme, parentId, kind)
// tetap dibaca, tapi TIDAK PERNAH dikirim ulang — PUT backend bersifat
// parsial, jadi field yang tidak dikirim tetap apa adanya di server.

/** Grup host. Desktop cuma memakai grup satu tingkat (`parentId` selalu null). */
@Serializable
data class HostGroup(
    val id: String,
    val name: String,
    val subtitle: String? = null,
    val parentId: String? = null,
)

/** Host SSH. Password TIDAK PERNAH ikut — cuma indikator [hasPassword]. */
@Serializable
data class Host(
    val id: String,
    val label: String,
    val host: String,
    val port: Int = 22,
    val username: String = "",
    val kind: String = "ssh",
    val groupId: String? = null,
    val tags: List<String> = emptyList(),
    val terminalTheme: String? = null,
    val hasPassword: Boolean = false,
)

/** Identity = pasangan username + password yang bisa dipakai mengisi form host. */
@Serializable
data class Identity(val id: String, val label: String, val username: String)

/**
 * Isian form host yang dikirim ke server (create & update). `groupId`
 * null WAJIB terkirim sebagai `null` ("pindah ke Tanpa grup") — lihat
 * encoder khusus di [VaultApi].
 */
@Serializable
data class HostFields(
    val label: String,
    val host: String,
    val port: Int,
    val username: String,
    val groupId: String?,
)

@Serializable
internal data class GroupFields(val name: String)

@Serializable
internal data class SecretBody(val password: String)

@Serializable
internal data class IdentityCreate(val label: String, val username: String, val password: String)

/** `password` null = tidak diubah — field-nya DIBUANG dari JSON (backend menolak `null`). */
@Serializable
internal data class IdentityUpdate(val label: String, val username: String, val password: String? = null)
