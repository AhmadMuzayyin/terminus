package org.terminus.mobile.ssh

import android.content.Context
import androidx.datastore.core.DataStore
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.stringPreferencesKey
import androidx.datastore.preferences.preferencesDataStore
import kotlinx.coroutines.flow.first

private val Context.knownHostsDataStore: DataStore<Preferences> by preferencesDataStore(name = "known_hosts")

/**
 * known_hosts di DataStore: satu entri per `host`/`[host]:port`, nilai
 * `algoritma SHA256:…`. Ikut `allowBackup="false"` (tidak ke cloud).
 */
class DataStoreKnownHostsStore(context: Context) : KnownHostsStore {
    private val store = context.applicationContext.knownHostsDataStore

    override suspend fun get(id: String): HostKeyInfo? =
        store.data.first()[stringPreferencesKey(id)]?.let(HostKeyInfo::parse)

    override suspend fun put(id: String, info: HostKeyInfo) {
        store.edit { it[stringPreferencesKey(id)] = info.serialize() }
    }

    override suspend fun remove(id: String) {
        store.edit { it.remove(stringPreferencesKey(id)) }
    }
}
