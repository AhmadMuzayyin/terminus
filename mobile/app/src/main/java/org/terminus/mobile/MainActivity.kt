package org.terminus.mobile

import android.os.Bundle
import android.graphics.Color
import androidx.activity.ComponentActivity
import androidx.activity.SystemBarStyle
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import org.terminus.mobile.ui.TerminusApp
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
        setContent {
            TerminusTheme {
                TerminusApp()
            }
        }
    }
}
