package org.terminus.mobile

import android.app.Application
import java.security.Security
import org.bouncycastle.jce.provider.BouncyCastleProvider

class TerminusApplication : Application() {
    lateinit var container: AppContainer
        private set

    override fun onCreate() {
        super.onCreate()
        // Android membawa provider "BC" versi PANGKASAN (banyak algoritma
        // dibuang); sshj butuh BouncyCastle lengkap (Ed25519, curve25519, …).
        // Ganti dengan versi dari dependency, prioritas pertama.
        Security.removeProvider(BouncyCastleProvider.PROVIDER_NAME)
        Security.insertProviderAt(BouncyCastleProvider(), 1)

        container = AppContainer(this)
        // Login otomatis dimulai SEKALI per proses (bukan per Activity —
        // rotasi layar tidak boleh mengulang refresh token).
        container.sessionManager.start()
    }
}
