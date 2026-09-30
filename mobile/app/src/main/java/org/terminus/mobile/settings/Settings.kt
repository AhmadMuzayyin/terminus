package org.terminus.mobile.settings

import android.content.Context
import androidx.datastore.core.DataStore
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.booleanPreferencesKey
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.floatPreferencesKey
import androidx.datastore.preferences.core.stringPreferencesKey
import androidx.datastore.preferences.preferencesDataStore
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.map
import org.terminus.mobile.terminal.FontSize
import org.terminus.mobile.terminal.TerminalThemes

/** Pengaturan app di HP (mobile/DESIGN.md bagian 7 — Akun > Pengaturan). Bukan data server. */
data class AppSettings(
    /** Kunci app dengan biometrik / kunci layar HP. Default MATI (keputusan bagian 2). */
    val appLock: Boolean = false,
    val fontSizeSp: Float = FontSize.DEFAULT_SP,
    /** Tema untuk host yang tidak punya tema sendiri di server. */
    val terminalTheme: String = TerminalThemes.DEFAULT_NAME,
)

interface SettingsStore {
    val settings: Flow<AppSettings>

    suspend fun update(transform: (AppSettings) -> AppSettings)
}

private val Context.settingsDataStore: DataStore<Preferences> by preferencesDataStore(name = "settings")

class DataStoreSettingsStore(context: Context) : SettingsStore {
    private val store = context.applicationContext.settingsDataStore

    override val settings: Flow<AppSettings> = store.data.map(::read)

    override suspend fun update(transform: (AppSettings) -> AppSettings) {
        store.edit { prefs ->
            val next = transform(read(prefs))
            prefs[APP_LOCK] = next.appLock
            prefs[FONT_SIZE] = next.fontSizeSp
            prefs[THEME] = next.terminalTheme
        }
    }

    private fun read(prefs: Preferences): AppSettings {
        val defaults = AppSettings()
        return AppSettings(
            appLock = prefs[APP_LOCK] ?: defaults.appLock,
            fontSizeSp = prefs[FONT_SIZE] ?: defaults.fontSizeSp,
            terminalTheme = prefs[THEME] ?: defaults.terminalTheme,
        )
    }

    private companion object {
        val APP_LOCK = booleanPreferencesKey("app_lock")
        val FONT_SIZE = floatPreferencesKey("font_size_sp")
        val THEME = stringPreferencesKey("terminal_theme")
    }
}
