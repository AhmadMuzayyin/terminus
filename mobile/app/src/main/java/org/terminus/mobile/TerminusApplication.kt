package org.terminus.mobile

import android.app.Application

class TerminusApplication : Application() {
    lateinit var container: AppContainer
        private set

    override fun onCreate() {
        super.onCreate()
        container = AppContainer(this)
        // Login otomatis dimulai SEKALI per proses (bukan per Activity —
        // rotasi layar tidak boleh mengulang refresh token).
        container.sessionManager.start()
    }
}
