package org.terminus.mobile.api

import kotlinx.serialization.Serializable

/** Pasangan token hasil login/register/refresh/ganti password. */
@Serializable
data class AuthTokens(val accessToken: String, val refreshToken: String)

/** `GET/PATCH /auth/me`. `fullName` null = akun lama (backend/DESIGN.md Milestone 6). */
@Serializable
data class Account(val id: String, val email: String, val fullName: String? = null) {
    /** Yang tampil di UI: nama, atau email kalau nama belum diisi. */
    val displayName: String get() = fullName?.takeIf { it.isNotBlank() } ?: email
}

/** Satu baris `GET /vaults` (role tidak dipakai di v1). */
@Serializable
data class VaultSummary(val id: String, val name: String)

@Serializable
internal data class LoginRequest(val email: String, val password: String)

@Serializable
internal data class RegisterRequest(val email: String, val password: String, val fullName: String)

@Serializable
internal data class RefreshRequest(val refreshToken: String)

@Serializable
internal data class CreateVaultRequest(val name: String)

@Serializable
internal data class ErrorBody(val error: String? = null)
