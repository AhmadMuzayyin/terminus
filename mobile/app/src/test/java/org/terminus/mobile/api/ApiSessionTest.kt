package org.terminus.mobile.api

import java.util.concurrent.atomic.AtomicInteger
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.async
import kotlinx.coroutines.awaitAll
import kotlinx.coroutines.runBlocking
import mockwebserver3.Dispatcher
import mockwebserver3.MockResponse
import mockwebserver3.MockWebServer
import mockwebserver3.RecordedRequest
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Assert.fail
import org.junit.Before
import org.junit.Test
import org.terminus.mobile.json
import org.terminus.mobile.tokens

/** Aturan client di mobile/DESIGN.md bagian 5 (sama dengan desktop). */
class ApiSessionTest {
    private lateinit var server: MockWebServer
    private val client = HttpClient()
    private val persisted = mutableListOf<String>()
    private var expiredMessage: String? = null

    @Before fun setUp() {
        server = MockWebServer()
        server.start()
    }

    @After fun tearDown() = server.close()

    private fun session(access: String = "access-1", refresh: String = "refresh-1") = ApiSession(
        client = client,
        authApi = AuthApi(client),
        baseUrl = server.url("/").toString().trimEnd('/'),
        vaultId = "vault-1",
        tokens = AuthTokens(access, refresh),
        persistRefreshToken = { persisted += it },
        onSessionExpired = { expiredMessage = it },
    )

    private val me = """{"id":"u1","email":"a@b.c","fullName":"Budi"}"""

    @Test fun `401 - refresh sekali lalu ulangi request, token hasil rotasi dipersist`() = runBlocking {
        server.enqueue(json(401, """{"error":"expired"}"""))
        server.enqueue(json(200, tokens("access-2", "refresh-2")))
        server.enqueue(json(200, me))

        val account = session().me()

        assertEquals("Budi", account.displayName)
        assertEquals(listOf("refresh-2"), persisted)
        assertEquals("/api/v1/auth/me", server.takeRequest().url.encodedPath)
        assertEquals("/api/v1/auth/refresh", server.takeRequest().url.encodedPath)
        assertEquals("Bearer access-2", server.takeRequest().headers["Authorization"])
    }

    @Test fun `refresh ditolak - SessionExpired dan callback dipanggil`() = runBlocking {
        server.enqueue(json(401, "{}"))
        server.enqueue(json(401, """{"error":"Refresh token tidak valid"}"""))
        try {
            session().me()
            fail("harus SessionExpired")
        } catch (e: ApiError.SessionExpired) {
            assertEquals("Refresh token tidak valid", e.message)
        }
        assertEquals("Refresh token tidak valid", expiredMessage)
        assertTrue(persisted.isEmpty())
    }

    @Test fun `setelah ditandai logout - token hasil rotasi TIDAK dipersist`() = runBlocking {
        server.enqueue(json(401, "{}"))
        server.enqueue(json(200, tokens("access-2", "refresh-2")))
        server.enqueue(json(200, me))
        val s = session()
        s.markLoggedOut()
        s.me()
        assertTrue(persisted.isEmpty())
    }

    @Test fun `403 (password saat ini salah) tidak memicu refresh`() = runBlocking {
        server.enqueue(json(403, """{"error":"Password saat ini salah"}"""))
        val result = session().send("PATCH", server.url("/api/v1/auth/me").toString(), """{"email":"x@y.z"}""")
        assertEquals(403, result.status)
        assertEquals("Password saat ini salah", client.httpError(result).message)
        assertEquals(1, server.requestCount)
    }

    @Test fun `refresh gagal jaringan - Network, bukan SessionExpired`() = runBlocking {
        val s = session()
        server.enqueue(json(401, "{}"))
        server.enqueue(MockResponse.Builder().code(200).onResponseStart(mockwebserver3.SocketEffect.ShutdownConnection).build())
        try {
            s.me()
            fail("harus gagal")
        } catch (e: ApiError.SessionExpired) {
            fail("gangguan jaringan tidak boleh dianggap sesi berakhir")
        } catch (e: ApiError) {
            // Network / Http — token tetap (tidak ada callback expired)
        }
        assertEquals(null, expiredMessage)
    }

    @Test fun `dua request 401 bersamaan - refresh cuma SEKALI`() = runBlocking {
        val refreshCount = AtomicInteger()
        server.dispatcher = object : Dispatcher() {
            override fun dispatch(request: RecordedRequest): MockResponse = when {
                request.url.encodedPath == "/api/v1/auth/refresh" -> {
                    refreshCount.incrementAndGet()
                    Thread.sleep(100) // perlebar jendela balapan
                    json(200, tokens("access-2", "refresh-2"))
                }
                request.headers["Authorization"] == "Bearer access-1" -> json(401, "{}")
                else -> json(200, me)
            }
        }
        val s = session()
        (1..2).map { async(Dispatchers.IO) { s.me() } }.awaitAll()
        assertEquals(1, refreshCount.get())
        assertEquals(listOf("refresh-2"), persisted)
    }
}
