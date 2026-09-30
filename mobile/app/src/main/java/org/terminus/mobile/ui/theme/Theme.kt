package org.terminus.mobile.ui.theme

import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color

// Nilai SAMA PERSIS dengan `Tokens` desktop (ui/tokens.slint) — desktop
// sudah memakai skema Material 3 gelap, jadi bisa dipetakan 1:1 supaya
// mobile & desktop terasa satu aplikasi. Sengaja cuma tema gelap di v1.
private val TerminusColors = darkColorScheme(
    primary = Color(0xFFADC6FF),
    onPrimary = Color(0xFF002E6A),
    primaryContainer = Color(0xFF4D8EFF),
    onPrimaryContainer = Color(0xFF00285D),
    secondaryContainer = Color(0xFF00A572), // success-container desktop: indikator tab aktif hijau seperti NavRail
    onSecondaryContainer = Color(0xFF00311F),
    surface = Color(0xFF0B1326),
    onSurface = Color(0xFFDAE2FD),
    onSurfaceVariant = Color(0xFFC2C6D6),
    surfaceContainerLowest = Color(0xFF060E20),
    surfaceContainerLow = Color(0xFF131B2E),
    surfaceContainer = Color(0xFF171F33),
    surfaceContainerHigh = Color(0xFF222A3D),
    surfaceContainerHighest = Color(0xFF2D3449),
    background = Color(0xFF0B1326),
    onBackground = Color(0xFFDAE2FD),
    outline = Color(0xFF8C909F),
    outlineVariant = Color(0xFF424754),
    error = Color(0xFFFFB4AB),
    onError = Color(0xFF690005),
    errorContainer = Color(0xFF93000A),
    onErrorContainer = Color(0xFFFFDAD6),
)

@Composable
fun TerminusTheme(content: @Composable () -> Unit) {
    MaterialTheme(colorScheme = TerminusColors, content = content)
}
