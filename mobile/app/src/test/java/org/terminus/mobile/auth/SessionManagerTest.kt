package org.terminus.mobile.auth

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import mockwebserver3.Dispatcher
import mockwebserver3.MockResponse
import mockwebserver3.MockWebServer
import mockwebserver3.RecordedRequest
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.terminus.mobile.FakeConfigStore
import org.terminus.mobile.FakeTokenStore
import org.terminus.mobile.api.HttpClient
import org.terminus.mobile.json
import org.terminus.mobile.tokens

class SessionManagerTest {
    private lateinit var server: MockWebServer
    private lateinit var baseUrl: String
    private val tokenStore = FakeTokenStore()
    private val configStore = FakeConfigStore()
    private var vaults = "[]"
    private val requests = mutableListOf<String>()

    @Before fun setUp() {
        server = MockWebServer()
        server.dispatcher = object : Dispatcher() {
            override fun dispatch(request: RecordedRequest): MockResponse {
                val route = "${request.method} ${request.url.encodedPath}"
                synchronized(requests) { requests += route }
                return when (route) {
                    "POST /api/v1/auth/login" -> json(200, tokens("a1", "r1"))
                    "POST /api/v1/auth/refresh" ->
                        if (request.body?.utf8()?.contains("revoked") == true) json(401, """{"error":"dicabut"}""")
                        else json(200, tokens("a2", "r2"))
                    "POST /api/v1/auth/logout" -> MockResponse.Builder().code(204).build()
                    "GET /api/v1/vaults" -> json(200, vaults)
                    "POST /api/v1/vaults" -> json(201, """{"id":"v-baru","name":"My Vault","role":"owner"}""")
                    "GET /api/v1/auth/me" -> json(200, """{"id":"u1","email":"a@b.c","fullName":null}""")
                    else -> json(404, """{"error":"tidak ada"}""")
                }
            }
        }
        server.start()
        baseUrl = server.url("/").toString().trimEnd('/')
    }

    @After fun tearDown() = server.close()

    private fun manager() = SessionManager(HttpClient(), tokenStore, configStore, CoroutineScope(Dispatchers.Unconfined))

    private suspend fun SessionManager.awaitLoggedIn() =
        withTimeout(5_000) { state.first { it is AuthState.LoggedIn } } as AuthState.LoggedIn

    private suspend fun SessionManager.awaitLoggedOut() =
        withTimeout(5_000) { state.first { it is AuthState.LoggedOut } } as AuthState.LoggedOut

    @Test fun `login tanpa vault - buat My Vault, simpan token & config`() = runBlocking {
        val m = manager()
        m.login("$baseUrl/", "a@b.c", "pw")
        val s = m.awaitLoggedIn()
        assertEquals("v-baru", s.session.vaultId)
        assertEquals("a@b.c", s.account.displayName) // fullName null -> email
        assertEquals("r1", tokenStore.token)
        assertEquals(AppConfig(baseUrl, "v-baru"), configStore.config)
        assertTrue("POST /api/v1/vaults" in requests)
    }

    @Test fun `vault tersimpan dipakai lagi selama masih anggota & server sama`() = runBlocking {
        vaults = """[{"id":"v1","name":"A"},{"id":"v2","name":"B"}]"""
        configStore.config = AppConfig(baseUrl, "v2")
        val m = manager()
        m.login(baseUrl, "a@b.c", "pw")
        assertEquals("v2", m.awaitLoggedIn().session.vaultId)
    }

    @Test fun `vault tersimpan dari server LAIN diabaikan - pakai vault pertama`() = runBlocking {
        vaults = """[{"id":"v1","name":"A"},{"id":"v2","name":"B"}]"""
        configStore.config = AppConfig("http://server-lain", "v2")
        val m = manager()
        m.login(baseUrl, "a@b.c", "pw")
        assertEquals("v1", m.awaitLoggedIn().session.vaultId)
    }

    @Test fun `start dengan token tersimpan - masuk otomatis`() = runBlocking {
        vaults = """[{"id":"v1","name":"A"}]"""
        configStore.config = AppConfig(baseUrl, "v1")
        tokenStore.token = "r-lama"
        val m = manager()
        m.start()
        m.awaitLoggedIn()
        assertEquals("token hasil refresh harus menggantikan yang lama", "r2", tokenStore.token)
    }

    @Test fun `start - token dicabut server - token dibuang, kembali ke Login`() = runBlocking {
        configStore.config = AppConfig(baseUrl, "v1")
        tokenStore.token = "revoked"
        val m = manager()
        m.start()
        val out = m.awaitLoggedOut()
        assertEquals(baseUrl, out.serverUrl)
        assertNull(tokenStore.token)
        assertEquals("Sesi berakhir, silakan masuk lagi.", m.loginUi.value.error)
    }

    @Test fun `start - server mati - token DIPERTAHANKAN`() = runBlocking {
        configStore.config = AppConfig("http://127.0.0.1:1", "v1")
        tokenStore.token = "r-lama"
        val m = manager()
        m.start()
        m.awaitLoggedOut()
        assertEquals("r-lama", tokenStore.token)
    }

    @Test fun `daftar - konfirmasi beda ditolak lokal tanpa request`() = runBlocking {
        val m = manager()
        m.register(baseUrl, "Budi", "a@b.c", "password-panjang", "beda-sekali")
        assertEquals("Konfirmasi password tidak sama.", m.loginUi.value.error)
        assertTrue(requests.isEmpty())
    }

    @Test fun `URL tanpa skema ditolak lokal`() = runBlocking {
        val m = manager()
        m.login("vault.contoh.com", "a@b.c", "pw")
        assertEquals("Server URL harus diawali http:// atau https://", m.loginUi.value.error)
        assertTrue(requests.isEmpty())
    }

    @Test fun `logout - token dihapus, cabut di server, URL tetap`() = runBlocking {
        val m = manager()
        m.login(baseUrl, "a@b.c", "pw")
        m.awaitLoggedIn()
        m.logout()
        val out = m.awaitLoggedOut()
        assertNull(tokenStore.token)
        assertEquals(baseUrl, out.serverUrl)
        withTimeout(5_000) { while (synchronized(requests) { "POST /api/v1/auth/logout" !in requests }) kotlinx.coroutines.delay(20) }
    }

    @Test fun `normalisasi URL`() {
        assertEquals("https://vault.contoh.com", normalizeServerUrl("  https://vault.contoh.com/ ").getOrThrow())
        assertTrue(normalizeServerUrl("   ").isFailure)
    }
}
