package org.terminus.mobile

import mockwebserver3.MockResponse
import org.terminus.mobile.auth.AppConfig
import org.terminus.mobile.auth.ConfigStore
import org.terminus.mobile.auth.RefreshTokenStore

fun json(code: Int, body: String): MockResponse =
    MockResponse.Builder().code(code).addHeader("Content-Type", "application/json").body(body).build()

fun tokens(access: String, refresh: String) = """{"accessToken":"$access","refreshToken":"$refresh"}"""

class FakeTokenStore(var token: String? = null) : RefreshTokenStore {
    val saved = mutableListOf<String>()
    override suspend fun load() = token
    override suspend fun save(token: String) {
        this.token = token
        saved += token
    }
    override suspend fun clear() {
        token = null
    }
}

class FakeConfigStore(var config: AppConfig = AppConfig()) : ConfigStore {
    override suspend fun load() = config
    override suspend fun save(config: AppConfig) {
        this.config = config
    }
}
