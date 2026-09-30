package org.terminus.mobile.auth

import android.content.Context
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.stringPreferencesKey
import androidx.datastore.preferences.preferencesDataStore
import kotlinx.coroutines.flow.first

private val Context.configStore by preferencesDataStore(name = "config")

class DataStoreConfigStore(private val context: Context) : ConfigStore {
    private val serverUrlKey = stringPreferencesKey("server_url")
    private val vaultIdKey = stringPreferencesKey("vault_id")

    override suspend fun load(): AppConfig {
        val prefs = context.configStore.data.first()
        return AppConfig(serverUrl = prefs[serverUrlKey], vaultId = prefs[vaultIdKey])
    }

    override suspend fun save(config: AppConfig) {
        context.configStore.edit { prefs ->
            config.serverUrl?.let { prefs[serverUrlKey] = it } ?: prefs.remove(serverUrlKey)
            config.vaultId?.let { prefs[vaultIdKey] = it } ?: prefs.remove(vaultIdKey)
        }
    }
}
