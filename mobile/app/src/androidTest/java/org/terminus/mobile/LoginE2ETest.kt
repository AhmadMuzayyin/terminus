package org.terminus.mobile

import androidx.compose.ui.test.hasSetTextAction
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextReplacement
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.Assume.assumeTrue
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/**
 * E2E login lawan backend SUNGGUHAN (mobile/DESIGN.md bagian 8). Server &
 * akun dikirim lewat argumen instrumentasi — JANGAN pakai server berisi
 * akun sungguhan. Tanpa argumen, test DILEWATI (bukan gagal).
 *
 * ```
 * ./gradlew connectedDebugAndroidTest \
 *   -Pandroid.testInstrumentationRunnerArguments.serverUrl=http://10.0.2.2:4100 \
 *   -Pandroid.testInstrumentationRunnerArguments.email=mobile@terminus.test \
 *   -Pandroid.testInstrumentationRunnerArguments.password=... \
 *   -Pandroid.testInstrumentationRunnerArguments.expectHost=ssh-uji
 * ```
 * Data app dibersihkan sebelum tiap test (orchestrator + clearPackageData).
 */
@RunWith(AndroidJUnit4::class)
class LoginE2ETest {
    @get:Rule
    val compose = createAndroidComposeRule<MainActivity>()

    private val args = InstrumentationRegistry.getArguments()
    private fun arg(name: String): String = args.getString(name).orEmpty()

    @Before
    fun needsServer() {
        assumeTrue("Argumen serverUrl/email/password tidak diberikan — E2E dilewati", arg("serverUrl").isNotEmpty())
    }

    private fun fill(label: String, value: String) {
        compose.onNode(hasSetTextAction() and hasText(label)).performTextReplacement(value)
    }

    private fun waitForText(text: String, timeoutMillis: Long = 15_000) {
        compose.waitUntil(timeoutMillis) {
            compose.onAllNodes(hasText(text)).fetchSemanticsNodes().isNotEmpty()
        }
    }

    private fun login(password: String) {
        waitForText("Masuk ke Server")
        fill("Server URL", arg("serverUrl"))
        fill("Email", arg("email"))
        fill("Password", password)
        compose.onNodeWithText("Masuk").performClick()
    }

    @Test
    fun loginBenar_daftarHostTampil() {
        login(arg("password"))
        waitForText("Cari host, IP, username, grup")
        arg("expectHost").takeIf { it.isNotEmpty() }?.let { waitForText(it) }
    }

    @Test
    fun passwordSalah_pesanError_tetapDiLogin() {
        login("password-yang-pasti-salah")
        waitForText("Email atau password salah")
        compose.onNodeWithText("Masuk ke Server").assertExists()
    }
}
