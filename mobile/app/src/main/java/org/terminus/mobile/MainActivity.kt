package org.terminus.mobile

import android.graphics.Color
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.SystemBarStyle
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import org.terminus.mobile.api.VaultApi
import org.terminus.mobile.auth.AuthState
import org.terminus.mobile.data.VaultRepository
import org.terminus.mobile.ui.TerminusApp
import org.terminus.mobile.ui.login.LoginScreen
import org.terminus.mobile.ui.theme.TerminusTheme

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        // App SELALU tema gelap (ui/theme/Theme.kt) — ikon status/navigation
        // bar dipaksa terang. Default `enableEdgeToEdge()` mengikuti tema
        // sistem: di HP bertema terang ikonnya jadi gelap di atas latar
        // gelap & tidak terbaca.
        enableEdgeToEdge(
            statusBarStyle = SystemBarStyle.dark(Color.TRANSPARENT),
            navigationBarStyle = SystemBarStyle.dark(Color.TRANSPARENT),
        )
        val container = (application as TerminusApplication).container
        setContent {
            TerminusTheme {
                Surface(Modifier.fillMaxSize()) { Root(container) }
            }
        }
    }
}

@Composable
private fun Root(container: AppContainer) {
    val sessionManager = container.sessionManager
    val state by sessionManager.state.collectAsStateWithLifecycle()
    val loginUi by sessionManager.loginUi.collectAsStateWithLifecycle()

    when (val s = state) {
        AuthState.Starting -> StartingScreen(loginUi.info)
        is AuthState.LoggedOut -> LoginScreen(
            initialServerUrl = s.serverUrl,
            ui = loginUi,
            onLogin = sessionManager::login,
            onRegister = sessionManager::register,
            onSwitchForm = sessionManager::clearLoginMessages,
        )
        is AuthState.LoggedIn -> {
            // Satu repository per sesi login: logout -> login lagi (mungkin
            // akun/server lain) mulai dari isi kosong, bukan isi akun lama.
            val repository = remember(s.session) { VaultRepository(VaultApi(container.httpClient, s.session)) }
            TerminusApp(s.account, s.session.baseUrl, repository, container, onLogout = { sessionManager.logout() })
        }
    }
}

/** Selama membaca token tersimpan / login otomatis. */
@Composable
private fun StartingScreen(info: String?) {
    Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
        Column(horizontalAlignment = Alignment.CenterHorizontally) {
            CircularProgressIndicator()
            info?.let {
                Text(it, color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.padding(top = 16.dp))
            }
        }
    }
}
