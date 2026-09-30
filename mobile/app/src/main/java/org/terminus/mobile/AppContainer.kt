package org.terminus.mobile

import android.content.Context
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import org.terminus.mobile.api.HttpClient
import org.terminus.mobile.auth.DataStoreConfigStore
import org.terminus.mobile.auth.KeystoreTokenStore
import org.terminus.mobile.auth.SessionManager

/** Wiring manual semua dependency (tanpa framework DI — mobile/DESIGN.md bagian 3). */
class AppContainer(context: Context) {
    /** Scope seumur proses: login/logout tidak boleh batal cuma karena layar berganti. */
    val appScope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)

    val httpClient = HttpClient()

    val sessionManager = SessionManager(
        client = httpClient,
        tokenStore = KeystoreTokenStore(context),
        configStore = DataStoreConfigStore(context),
        scope = appScope,
    )
}
