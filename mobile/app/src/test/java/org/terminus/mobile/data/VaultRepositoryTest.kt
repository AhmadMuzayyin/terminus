package org.terminus.mobile.data

import java.util.concurrent.CopyOnWriteArrayList
import kotlinx.coroutines.runBlocking
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import mockwebserver3.Dispatcher
import mockwebserver3.MockResponse
import mockwebserver3.MockWebServer
import mockwebserver3.RecordedRequest
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Assert.fail
import org.junit.Before
import org.junit.Test
import org.terminus.mobile.api.ApiError
import org.terminus.mobile.api.ApiSession
import org.terminus.mobile.api.AuthApi
import org.terminus.mobile.api.AuthTokens
import org.terminus.mobile.api.Host
import org.terminus.mobile.api.HostFields
import org.terminus.mobile.api.HttpClient
import org.terminus.mobile.api.Identity
import org.terminus.mobile.api.VaultApi
import org.terminus.mobile.json

/** Alur server-authoritative mobile/DESIGN.md bagian 5 lawan server tiruan. */
class VaultRepositoryTest {
    private lateinit var server: MockWebServer
    private val requests = CopyOnWriteArrayList<Pair<String, String>>() // "METHOD /path" to body
    private val routes = HashMap<String, MockResponse>()

    private val hostJson = """{"id":"h1","label":"web","host":"10.0.0.1","port":22,"username":"root",""" +
        """"kind":"ssh","groupId":null,"tags":[],"terminalTheme":null,"hasPassword":false}"""

    @Before fun setUp() {
        server = MockWebServer()
        server.dispatcher = object : Dispatcher() {
            override fun dispatch(request: RecordedRequest): MockResponse {
                val key = "${request.method} ${request.url.encodedPath.removePrefix("/api/v1/vaults/v1")}"
                requests += key to request.body?.utf8().orEmpty()
                return routes[key] ?: json(200, "[]")
            }
        }
        server.start()
    }

    @After fun tearDown() = server.close()

    private fun repository(): VaultRepository {
        val client = HttpClient()
        val session = ApiSession(
            client = client,
            authApi = AuthApi(client),
            baseUrl = server.url("/").toString().trimEnd('/'),
            vaultId = "v1",
            tokens = AuthTokens("a", "r"),
            persistRefreshToken = {},
            onSessionExpired = {},
        )
        return VaultRepository(VaultApi(client, session))
    }

    private fun bodyOf(key: String): JsonObject =
        Json.parseToJsonElement(requests.single { it.first == key }.second).jsonObject

    private fun JsonObject.string(field: String) = getValue(field).jsonPrimitive.content

    private fun keys() = requests.map { it.first }

    @Test fun `host baru - dibuat dulu, lalu password, lalu daftar dimuat ulang`() = runBlocking {
        routes["POST /hosts"] = json(201, hostJson)
        routes["PUT /hosts/h1/secret"] = MockResponse.Builder().code(204).build()
        routes["GET /hosts"] = json(200, "[${hostJson.replace("\"hasPassword\":false", "\"hasPassword\":true")}]")
        val repo = repository()

        repo.saveHost(null, HostFields("web", "10.0.0.1", 22, "root", null), "rahasia")

        assertEquals(listOf("POST /hosts", "PUT /hosts/h1/secret"), keys().take(2))
        // "Tanpa grup" WAJIB terkirim sebagai null eksplisit (lihat VaultApi.keepNulls).
        assertEquals(JsonNull, bodyOf("POST /hosts")["groupId"])
        assertEquals("rahasia", bodyOf("PUT /hosts/h1/secret").string("password"))
        assertTrue(repo.content.value.hosts.single().hasPassword)
        assertTrue(repo.load.value.loaded)
    }

    @Test fun `password gagal setelah host dibuat - HostPasswordNotSaved membawa host itu`() = runBlocking {
        routes["POST /hosts"] = json(201, hostJson)
        routes["PUT /hosts/h1/secret"] = json(500, """{"error":"Kunci server rusak"}""")
        routes["GET /hosts"] = json(200, "[$hostJson]")
        val repo = repository()

        try {
            repo.saveHost(null, HostFields("web", "10.0.0.1", 22, "root", null), "rahasia")
            fail("harus HostPasswordNotSaved")
        } catch (e: HostPasswordNotSaved) {
            assertEquals("h1", e.host.id)
            assertEquals("Host tersimpan, tapi password gagal disimpan: Kunci server rusak", e.message)
        }
        // Daftar tetap dimuat ulang -> host yang sudah tersimpan kelihatan.
        assertEquals("h1", repo.content.value.hosts.single().id)
    }

    @Test fun `edit host tanpa password baru - tidak ada PUT secret, field lain tidak dikirim`() = runBlocking {
        routes["PUT /hosts/h1"] = json(200, hostJson)
        val repo = repository()
        val existing = Host("h1", "web", "10.0.0.1", username = "root", tags = listOf("prod"), kind = "cisco_ios")

        repo.saveHost(existing, HostFields("web", "10.0.0.2", 22, "root", "g1"), password = null)

        assertFalse(keys().any { it.endsWith("/secret") })
        // tags/kind/terminalTheme tidak dikirim -> nilainya di server tetap (PUT parsial).
        assertEquals(setOf("label", "host", "port", "username", "groupId"), bodyOf("PUT /hosts/h1").keys)
    }

    @Test fun `duplikat host menyalin password lewat secret host asal`() = runBlocking {
        routes["POST /hosts"] = json(201, hostJson.replace("\"h1\"", "\"h2\""))
        routes["GET /hosts/h1/secret"] = json(200, """{"password":"rahasia"}""")
        routes["PUT /hosts/h2/secret"] = MockResponse.Builder().code(204).build()
        val repo = repository()

        repo.duplicateHost(Host("h1", "web", "10.0.0.1", username = "root", hasPassword = true))

        assertEquals("web (salinan)", bodyOf("POST /hosts").string("label"))
        assertEquals("rahasia", bodyOf("PUT /hosts/h2/secret").string("password"))
    }

    @Test fun `edit identity tanpa password - field password tidak dikirim sama sekali`() = runBlocking {
        routes["PUT /identities/i1"] = json(200, """{"id":"i1","label":"ops","username":"admin"}""")

        repository().saveIdentity(Identity("i1", "ops", "root"), "ops", "admin", password = null)

        // Backend menolak `"password": null` (zod optional, bukan nullable).
        assertEquals(setOf("label", "username"), bodyOf("PUT /identities/i1").keys)
    }

    @Test fun `muat ulang gagal - isi lama tetap, error diisi`() = runBlocking {
        routes["GET /hosts"] = json(200, "[$hostJson]")
        val repo = repository()
        repo.refresh()
        assertNull(repo.load.value.error)

        routes["GET /hosts"] = json(500, """{"error":"Database mati"}""")
        repo.refresh()

        assertEquals("h1", repo.content.value.hosts.single().id)
        assertEquals("Database mati", repo.load.value.error)
        assertTrue(repo.load.value.loaded)
        assertFalse(repo.load.value.loading)
    }

    @Test fun `hapus grup gagal - error diteruskan ke pemanggil`() = runBlocking {
        routes["DELETE /groups/g1"] = json(404, """{"error":"Grup tidak ditemukan di vault ini"}""")
        try {
            repository().deleteGroup("g1")
            fail("harus ApiError.Http")
        } catch (e: ApiError.Http) {
            assertEquals(404, e.status)
        }
    }
}
